use nocter_model::{BodyNodeId, BuiltinType, TypeId, TypeKind};
use nocter_syntax::{Keyword, NodeId, NodeKind, TokenKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::{child_nodes, first_direct_token, is_transparent_expression};
use crate::{
    CheckedControl, CheckedOperation, CheckedOutcome, ExpectedBase, ExpectedEvidence, OutcomeLayer,
    plan_expected_type,
};

/// Deferred result constraints for one closure whose result has no annotation or fixed context.
///
/// Checked expressions are constructed once. Only outcome injections on return/propagation edges
/// are deferred until every reachable result site has contributed its evidence.
#[derive(Default)]
pub(super) struct ClosureResultInference {
    returns: Vec<ReturnSite>,
    propagations: Vec<PropagationSite>,
}

struct ReturnSite {
    syntax: NodeId,
    control: BodyNodeId,
    payload: Option<BodyNodeId>,
    evidence: ExpectedEvidence,
    contributes: bool,
}

struct PropagationSite {
    syntax: NodeId,
    node: BodyNodeId,
    layer: OutcomeLayer,
    contributes: bool,
}

impl BodyChecker<'_, '_> {
    pub(super) fn record_inferred_closure_return(
        &mut self,
        syntax: NodeId,
        control: BodyNodeId,
        payload: Option<BodyNodeId>,
        evidence: ExpectedEvidence,
    ) -> Result<(), BodyCheckInternalError> {
        let inference = self
            .closure_result_inference
            .as_mut()
            .ok_or(BodyCheckInternalError::CallContractSelection)?;
        inference.returns.push(ReturnSite {
            syntax,
            control,
            payload,
            evidence,
            contributes: self.flow_reachable,
        });
        Ok(())
    }

    pub(super) fn record_inferred_closure_propagation(
        &mut self,
        syntax: NodeId,
        node: BodyNodeId,
        layer: OutcomeLayer,
    ) -> Result<(), BodyCheckInternalError> {
        let inference = self
            .closure_result_inference
            .as_mut()
            .ok_or(BodyCheckInternalError::CallContractSelection)?;
        inference.propagations.push(PropagationSite {
            syntax,
            node,
            layer,
            contributes: self.flow_reachable,
        });
        Ok(())
    }

    pub(super) fn finish_inferred_closure_result(
        &mut self,
        block_syntax: NodeId,
        block: BodyNodeId,
    ) -> Result<(BodyNodeId, TypeId), BodyCheckError> {
        let inference = self
            .closure_result_inference
            .take()
            .ok_or(BodyCheckInternalError::CallContractSelection)?;
        let block_type = self.node_type(block)?;
        let tail = (block_type != self.types.builtin(BuiltinType::Never)).then_some(block);
        let mut evidence = inference
            .returns
            .iter()
            .filter(|site| site.contributes)
            .map(|site| site.evidence)
            .collect::<Vec<_>>();
        evidence.extend(
            inference
                .propagations
                .iter()
                .filter(|site| site.contributes)
                .map(|site| match site.layer {
                    OutcomeLayer::Optional => ExpectedEvidence::Absent,
                    OutcomeLayer::Fallible => ExpectedEvidence::Failure,
                }),
        );
        if let Some(tail) = tail {
            evidence.push(ExpectedEvidence::Typed(self.node_type(tail)?));
        }
        let result = if evidence.is_empty() && block_type == self.types.builtin(BuiltinType::Never)
        {
            block_type
        } else {
            let Some(result) = infer_result_type(self.types, &evidence) else {
                return Err(self.rule(BodyRule::TypeMismatch, block_syntax)?);
            };
            result
        };
        if crate::validate_type(self.types, result, crate::TypePosition::CallableResult).is_err() {
            return Err(self.rule(BodyRule::TypeMismatch, block_syntax)?);
        }

        for site in inference.returns {
            let plan = plan_expected_type(self.types, result, site.evidence)
                .map_err(|error| self.expected_error(site.syntax, error))?;
            let value = self.materialize_plan(site.syntax, plan, site.payload)?;
            self.builder.replace_operation(
                site.control,
                CheckedOperation::Control(CheckedControl::Return(Some(value))),
            )?;
        }
        for site in inference.propagations {
            let evidence = match site.layer {
                OutcomeLayer::Optional => ExpectedEvidence::Absent,
                OutcomeLayer::Fallible => ExpectedEvidence::Failure,
            };
            let (base, outer) = plan_expected_type(self.types, result, evidence)
                .map_err(|error| self.expected_error(site.syntax, error))?
                .into_parts();
            if !matches!(
                (site.layer, base),
                (OutcomeLayer::Optional, ExpectedBase::Absent(_))
                    | (OutcomeLayer::Fallible, ExpectedBase::Failure(_))
            ) {
                return Err(BodyCheckInternalError::CallContractSelection.into());
            }
            let operation = self
                .builder
                .node(site.node)
                .ok_or(BodyCheckInternalError::MissingNode(site.node))?
                .operation();
            let CheckedOperation::Outcome(CheckedOutcome::Propagate { operand, layer, .. }) =
                operation
            else {
                return Err(BodyCheckInternalError::CallContractSelection.into());
            };
            self.builder.replace_operation(
                site.node,
                CheckedOperation::Outcome(CheckedOutcome::Propagate {
                    operand: *operand,
                    layer: *layer,
                    outer,
                }),
            )?;
        }

        let execution = match tail {
            Some(_) => self.apply_expected(block_syntax, block, result)?,
            None => block,
        };
        Ok((execution, result))
    }
}

fn infer_result_type(
    types: &mut nocter_model::TypeTransaction,
    evidence: &[ExpectedEvidence],
) -> Option<TypeId> {
    let bases = evidence
        .iter()
        .filter_map(|evidence| match evidence {
            ExpectedEvidence::Typed(ty)
                if *ty != types.builtin(BuiltinType::Never)
                    && *ty != types.builtin(BuiltinType::Error) =>
            {
                Some(*ty)
            }
            ExpectedEvidence::Typed(_) | ExpectedEvidence::Absent | ExpectedEvidence::Failure => {
                None
            }
        })
        .collect::<std::collections::BTreeSet<_>>();
    if bases.is_empty() {
        return None;
    }

    let mut candidates = Vec::<(usize, TypeId)>::new();
    for base in bases {
        let needs_optional = evidence.iter().any(|evidence| {
            *evidence == ExpectedEvidence::Absent
                && plan_expected_type(types, base, *evidence).is_err()
        });
        let needs_fallible = evidence.iter().any(|evidence| {
            (*evidence == ExpectedEvidence::Failure
                || matches!(evidence, ExpectedEvidence::Typed(ty) if *ty == types.builtin(BuiltinType::Error)))
                && plan_expected_type(types, base, *evidence).is_err()
        });
        let additions = usize::from(needs_optional) + usize::from(needs_fallible);
        match (needs_optional, needs_fallible) {
            (false, false) => candidates.push((0, base)),
            (true, false) => push_layer(
                types,
                &mut candidates,
                additions,
                base,
                OutcomeLayer::Optional,
            ),
            (false, true) => push_layer(
                types,
                &mut candidates,
                additions,
                base,
                OutcomeLayer::Fallible,
            ),
            (true, true) => {
                if let Some(optional) = layer(types, base, OutcomeLayer::Optional) {
                    push_layer(
                        types,
                        &mut candidates,
                        additions,
                        optional,
                        OutcomeLayer::Fallible,
                    );
                }
                if let Some(fallible) = layer(types, base, OutcomeLayer::Fallible) {
                    push_layer(
                        types,
                        &mut candidates,
                        additions,
                        fallible,
                        OutcomeLayer::Optional,
                    );
                }
            }
        }
    }
    candidates.retain(|(_, candidate)| {
        evidence
            .iter()
            .all(|evidence| plan_expected_type(types, *candidate, *evidence).is_ok())
    });
    let minimum = candidates.iter().map(|(added, _)| *added).min()?;
    let selected = candidates
        .into_iter()
        .filter_map(|(added, candidate)| (added == minimum).then_some(candidate))
        .collect::<std::collections::BTreeSet<_>>();
    (selected.len() == 1).then(|| *selected.first().unwrap())
}

fn push_layer(
    types: &mut nocter_model::TypeTransaction,
    candidates: &mut Vec<(usize, TypeId)>,
    additions: usize,
    base: TypeId,
    layer_: OutcomeLayer,
) {
    if let Some(candidate) = layer(types, base, layer_) {
        candidates.push((additions, candidate));
    }
}

fn layer(
    types: &mut nocter_model::TypeTransaction,
    payload: TypeId,
    layer: OutcomeLayer,
) -> Option<TypeId> {
    types
        .intern(match layer {
            OutcomeLayer::Optional => TypeKind::Optional(payload),
            OutcomeLayer::Fallible => TypeKind::Fallible(payload),
        })
        .ok()
}

pub(super) fn is_absent_expression(checker: &BodyChecker<'_, '_>, mut node: NodeId) -> bool {
    while checker.kind(node).is_ok_and(is_transparent_expression) {
        let children = child_nodes(checker.tree(), node);
        let [child] = children.as_slice() else {
            return false;
        };
        node = *child;
    }
    checker
        .kind(node)
        .is_ok_and(|kind| kind == NodeKind::ScalarLiteral)
        && first_direct_token(checker.tree(), node)
            .is_some_and(|token| token.kind() == TokenKind::Keyword(Keyword::None))
}

#[cfg(test)]
mod tests {
    use nocter_model::{BuiltinType, TypeKind, TypeStore};

    use super::infer_result_type;
    use crate::ExpectedEvidence;

    #[test]
    fn a_contextual_tag_adds_one_minimal_result_layer() {
        let mut types = TypeStore::new().transaction();
        let i32_type = types.builtin(BuiltinType::I32);
        let inferred = infer_result_type(
            &mut types,
            &[ExpectedEvidence::Absent, ExpectedEvidence::Typed(i32_type)],
        )
        .unwrap();
        assert_eq!(types.get(inferred), Some(&TypeKind::Optional(i32_type)));
    }

    #[test]
    fn unordered_distinct_outcome_requirements_remain_ambiguous() {
        let mut types = TypeStore::new().transaction();
        let i32_type = types.builtin(BuiltinType::I32);
        assert!(
            infer_result_type(
                &mut types,
                &[
                    ExpectedEvidence::Absent,
                    ExpectedEvidence::Failure,
                    ExpectedEvidence::Typed(i32_type),
                ],
            )
            .is_none()
        );
    }
}
