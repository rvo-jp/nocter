use nocter_model::{BodyNodeId, BuiltinType, TypeId, TypeKind};
use nocter_syntax::SyntaxOrigin;
use nocter_syntax::{Keyword, NodeId, NodeKind, Punctuation, SyntaxElement, TokenKind};

use super::value_planning::CallResultContext;
use super::{BlockExpectation, BodyChecker};
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::{child_nodes, direct_identifier, direct_node};
use crate::{
    CheckedOperation, CheckedOutcome, ExpectedBase, ExpectedEvidence, LocalBindingKind,
    OutcomeLayer, TypedBodyInterruption, TypedBodyInterruptionKind, plan_expected_type,
};

impl BodyChecker<'_, '_> {
    pub(super) fn check_outcome_expression(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let punctuation = outcome_punctuation(self, node)?;
        let operand = match self.kind(node)? {
            NodeKind::MoveExpression => self.check_move_place(node)?,
            NodeKind::OutcomeExpression => {
                let children = child_nodes(self.tree(), node);
                let [operand] = children.as_slice() else {
                    return Err(BodyCheckInternalError::InvalidSyntax(node).into());
                };
                let result_context = self.outcome_operand_context(expected, punctuation);
                self.check_outcome_operand_expression(*operand, result_context)?
            }
            _ => return Err(BodyCheckInternalError::InvalidSyntax(node).into()),
        };
        let operand_type = self.node_type(operand)?;
        let Some((layer, payload)) = outcome_layer(self.types, operand_type) else {
            return Err(self.rule(BodyRule::InvalidOutcomeOperation, node)?);
        };
        let operation = match punctuation {
            Punctuation::Question => {
                let outer = if self.closure_result_inference.is_some() {
                    Box::new([])
                } else {
                    let evidence = match layer {
                        OutcomeLayer::Optional => ExpectedEvidence::Absent,
                        OutcomeLayer::Fallible => ExpectedEvidence::Failure,
                    };
                    let Ok(plan) = plan_expected_type(self.types, self.result_type, evidence)
                    else {
                        self.record_outcome_contract_interruption(node, layer)?;
                        return Err(self.rule(BodyRule::InvalidOutcomeOperation, node)?);
                    };
                    let (base, outer) = plan.into_parts();
                    if !matches!(
                        (layer, base),
                        (OutcomeLayer::Optional, ExpectedBase::Absent(_))
                            | (OutcomeLayer::Fallible, ExpectedBase::Failure(_))
                    ) {
                        return Err(BodyCheckInternalError::InvalidSyntax(node).into());
                    }
                    outer
                };
                CheckedOutcome::Propagate {
                    operand,
                    layer,
                    outer,
                }
            }
            Punctuation::Bang => CheckedOutcome::Force { operand, layer },
            _ => return Err(BodyCheckInternalError::InvalidSyntax(node).into()),
        };
        let checked = self.add_node(node, payload, CheckedOperation::Outcome(operation))?;
        if punctuation == Punctuation::Question && self.closure_result_inference.is_some() {
            self.record_inferred_closure_propagation(node, checked, layer)?;
        }
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    fn record_outcome_contract_interruption(
        &mut self,
        node: NodeId,
        layer: OutcomeLayer,
    ) -> Result<(), BodyCheckInternalError> {
        let proposed_result = match (layer, self.types.get(self.result_type).cloned()) {
            // Official mixed-outcome spelling keeps fallible outside optional success.
            (OutcomeLayer::Optional, Some(TypeKind::Fallible(payload))) => {
                let optional = self
                    .types
                    .intern(TypeKind::Optional(payload))
                    .map_err(|_| BodyCheckInternalError::UnknownType(payload))?;
                self.types
                    .intern(TypeKind::Fallible(optional))
                    .map_err(|_| BodyCheckInternalError::UnknownType(optional))?
            }
            (OutcomeLayer::Optional, _) => self
                .types
                .intern(TypeKind::Optional(self.result_type))
                .map_err(|_| BodyCheckInternalError::UnknownType(self.result_type))?,
            (OutcomeLayer::Fallible, _) => self
                .types
                .intern(TypeKind::Fallible(self.result_type))
                .map_err(|_| BodyCheckInternalError::UnknownType(self.result_type))?,
        };
        let origin = nocter_source_index::SourceOrigin::from_node(self.tree(), node)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
        self.interruption = Some(TypedBodyInterruption::new(
            self.source.body(),
            origin,
            TypedBodyInterruptionKind::OutcomeContract {
                layer,
                proposed_result,
            },
        ));
        Ok(())
    }

    fn check_outcome_operand_expression(
        &mut self,
        root: NodeId,
        result_context: Option<CallResultContext>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let mut syntax = root;
        while self
            .kind(syntax)
            .is_ok_and(crate::syntax::is_transparent_expression)
        {
            let children = child_nodes(self.tree(), syntax);
            let [child] = children.as_slice() else {
                break;
            };
            syntax = *child;
        }
        if self.kind(syntax)? == NodeKind::PostfixExpression
            && direct_node(self.tree(), syntax, NodeKind::CallSuffix).is_some()
            && let Some(result_context) = result_context
        {
            return self.check_outcome_operand_call(syntax, result_context);
        }
        self.check_expression(syntax, None)
    }

    fn outcome_operand_context(
        &self,
        payload: Option<TypeId>,
        punctuation: Punctuation,
    ) -> Option<CallResultContext> {
        let payload = payload?;
        if punctuation != Punctuation::Question || self.closure_result_inference.is_some() {
            return None;
        }
        let accepts_optional = accepts_propagation(
            self.types,
            self.result_type,
            ExpectedEvidence::Absent,
            OutcomeLayer::Optional,
        );
        let accepts_fallible = accepts_propagation(
            self.types,
            self.result_type,
            ExpectedEvidence::Failure,
            OutcomeLayer::Fallible,
        );
        let payload = if payload == self.result_type {
            match (accepts_optional, accepts_fallible) {
                (true, false) => propagated_payload(
                    self.types,
                    self.result_type,
                    ExpectedEvidence::Absent,
                    OutcomeLayer::Optional,
                ),
                (false, true) => propagated_payload(
                    self.types,
                    self.result_type,
                    ExpectedEvidence::Failure,
                    OutcomeLayer::Fallible,
                ),
                (false, false) | (true, true) => None,
            }
            .unwrap_or(payload)
        } else {
            payload
        };
        match (accepts_optional, accepts_fallible) {
            (true, false) | (false, true) => Some(CallResultContext::OutcomePayload(payload)),
            (true, true) => Some(CallResultContext::Propagation(self.result_type)),
            (false, false) => None,
        }
    }

    pub(super) fn check_recovery_expression(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let children = child_nodes(self.tree(), node);
        let [operand_syntax, clause] = children.as_slice() else {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        };
        if self.kind(*clause)? != NodeKind::RecoveryClause {
            return Err(BodyCheckInternalError::InvalidSyntax(*clause).into());
        }
        let catch = self.tree().children(*clause).iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind() == TokenKind::Keyword(Keyword::Catch)
            )
        });
        let result_context = expected.map(CallResultContext::OutcomePayload);
        let operand = self.check_outcome_operand_expression(*operand_syntax, result_context)?;
        let operand_type = self.node_type(operand)?;
        let Some((layer, payload)) = outcome_layer(self.types, operand_type) else {
            return Err(self.rule(BodyRule::InvalidOutcomeOperation, *clause)?);
        };
        if catch != (layer == OutcomeLayer::Fallible) {
            return Err(self.rule(BodyRule::InvalidOutcomeOperation, *clause)?);
        }
        let binding = if catch {
            self.define_catch_binding(*clause)?
        } else {
            None
        };
        let block = direct_node(self.tree(), *clause, NodeKind::Block)
            .ok_or(BodyCheckInternalError::InvalidSyntax(*clause))?;
        let fallback = self.check_block(block, BlockExpectation::Value(Some(payload)))?;
        let recovered = self.add_node(
            node,
            payload,
            CheckedOperation::Outcome(CheckedOutcome::Recover {
                operand,
                layer,
                binding,
                fallback,
            }),
        )?;
        expected.map_or(Ok(recovered), |expected| {
            self.apply_expected(node, recovered, expected)
        })
    }

    fn define_catch_binding(
        &mut self,
        clause: NodeId,
    ) -> Result<Option<nocter_model::LocalBindingId>, BodyCheckError> {
        let token = direct_identifier(self.tree(), clause)
            .ok_or(BodyCheckInternalError::InvalidSyntax(clause))?;
        if self.token_text(token)? == "_" {
            return Ok(None);
        }
        let local = self
            .local_declarations
            .get(&SyntaxOrigin::Token(token))
            .copied()
            .ok_or(BodyCheckInternalError::MissingLocalDeclaration(clause))?;
        if self
            .names
            .locals()
            .get(local)
            .is_none_or(|declaration| declaration.kind() != LocalBindingKind::Catch)
        {
            return Err(BodyCheckInternalError::MissingLocalDeclaration(clause).into());
        }
        self.builder
            .define_local(local, self.types.builtin(BuiltinType::Error))?;
        Ok(Some(local))
    }
}

fn propagated_payload(
    types: &nocter_model::TypeStore,
    result: TypeId,
    evidence: ExpectedEvidence,
    layer: OutcomeLayer,
) -> Option<TypeId> {
    let plan = plan_expected_type(types, result, evidence).ok()?;
    let ((OutcomeLayer::Optional, ExpectedBase::Absent(outcome))
    | (OutcomeLayer::Fallible, ExpectedBase::Failure(outcome))) = (layer, plan.base())
    else {
        return None;
    };
    match (layer, types.get(outcome)?) {
        (OutcomeLayer::Optional, TypeKind::Optional(payload))
        | (OutcomeLayer::Fallible, TypeKind::Fallible(payload)) => Some(*payload),
        _ => None,
    }
}

fn accepts_propagation(
    types: &nocter_model::TypeStore,
    result: TypeId,
    evidence: ExpectedEvidence,
    layer: OutcomeLayer,
) -> bool {
    plan_expected_type(types, result, evidence).is_ok_and(|plan| {
        matches!(
            (layer, plan.base()),
            (OutcomeLayer::Optional, ExpectedBase::Absent(_))
                | (OutcomeLayer::Fallible, ExpectedBase::Failure(_))
        )
    })
}

fn outcome_layer(types: &nocter_model::TypeStore, ty: TypeId) -> Option<(OutcomeLayer, TypeId)> {
    match types.get(ty)? {
        TypeKind::Optional(payload) => Some((OutcomeLayer::Optional, *payload)),
        TypeKind::Fallible(payload) => Some((OutcomeLayer::Fallible, *payload)),
        _ => None,
    }
}

fn outcome_punctuation(
    checker: &BodyChecker<'_, '_>,
    node: NodeId,
) -> Result<Punctuation, BodyCheckInternalError> {
    checker
        .tree()
        .children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => match token.kind() {
                TokenKind::Punctuation(Punctuation::Question | Punctuation::Bang) => {
                    let TokenKind::Punctuation(punctuation) = token.kind() else {
                        unreachable!()
                    };
                    Some(punctuation)
                }
                _ => None,
            },
            SyntaxElement::Node(_) | SyntaxElement::Missing(_) => None,
        })
        .ok_or(BodyCheckInternalError::InvalidSyntax(node))
}
