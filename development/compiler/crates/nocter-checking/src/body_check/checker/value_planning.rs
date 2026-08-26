use nocter_model::{
    BodyNodeId, BorrowCapability, CallableContract, GenericParameterId, PlaceId, TypeId,
};
use nocter_syntax::{Keyword, NodeId, NodeKind, TokenKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::{direct_child, direct_nodes, direct_token, is_transparent_expression};
use crate::type_relations::{TypeSubstitution, collect_generic_parameters};
use crate::{
    CallableInference, CheckedPredicate, CheckedRequirement, GenericArguments, InferenceEvidence,
    InferenceFailure,
};

/// A source-order value awaiting the final substitution of its destination type.
pub(super) enum ValueDraft {
    Checked {
        syntax: NodeId,
        value: BodyNodeId,
    },
    Place {
        syntax: NodeId,
        place: PlaceId,
        ty: TypeId,
    },
    Deferred {
        syntax: NodeId,
    },
    Closure {
        syntax: NodeId,
        destination: TypeId,
        contract: Option<CallableContract>,
    },
}

#[derive(Clone, Copy)]
pub(super) enum CallResultContext {
    Complete(TypeId),
    OutcomePayload(TypeId),
    Propagation(TypeId),
}

impl CallResultContext {
    pub(super) const fn complete(expected: Option<TypeId>) -> Option<Self> {
        match expected {
            Some(expected) => Some(Self::Complete(expected)),
            None => None,
        }
    }

    pub(super) const fn complete_type(self) -> Option<TypeId> {
        match self {
            Self::Complete(expected) => Some(expected),
            Self::OutcomePayload(_) | Self::Propagation(_) => None,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct PositionalValueContext<'a> {
    pub(super) owner: NodeId,
    pub(super) result: TypeId,
    pub(super) inference_parameters: &'a [GenericParameterId],
    pub(super) destination_types: &'a [TypeId],
    pub(super) requirements: &'a [CheckedRequirement],
    pub(super) result_context: Option<CallResultContext>,
    pub(super) failure_rule: BodyRule,
}

impl BodyChecker<'_, '_> {
    /// Checks source-order values and collects generic evidence without assigning construction or
    /// call semantics to them. Calls, fields, variant payloads, and typed literals can therefore
    /// share one contextual-inference boundary.
    pub(super) fn infer_positional_values(
        &mut self,
        value_syntax: Vec<NodeId>,
        context: PositionalValueContext<'_>,
    ) -> Result<(Vec<ValueDraft>, GenericArguments), BodyCheckError> {
        let mut inference = CallableInference::new(context.inference_parameters);
        let mut values = Vec::with_capacity(value_syntax.len());
        for (syntax, destination) in value_syntax
            .into_iter()
            .zip(context.destination_types.iter().copied())
        {
            values.push(self.draft_positional_value(
                syntax,
                destination,
                context.inference_parameters,
                context.requirements,
                &mut inference,
                context.failure_rule,
            )?);
        }
        let generic_arguments =
            self.finish_positional_inference(&mut values, &context, inference)?;
        Ok((values, generic_arguments))
    }

    pub(super) fn draft_positional_value(
        &mut self,
        syntax: NodeId,
        destination: TypeId,
        inference_parameters: &[GenericParameterId],
        requirements: &[CheckedRequirement],
        inference: &mut CallableInference,
        failure_rule: BodyRule,
    ) -> Result<ValueDraft, BodyCheckError> {
        if let Some(closure_syntax) = closure_expression(self, syntax) {
            let contract = contextual_callable_contract(requirements, destination)?;
            return Ok(ValueDraft::Closure {
                syntax: closure_syntax,
                destination,
                contract,
            });
        }
        if is_none_expression(self, syntax) {
            inference
                .constrain_contextual(self.types, destination, InferenceEvidence::Absent)
                .map_err(|error| self.inference_error(syntax, error, failure_rule))?;
            return Ok(ValueDraft::Deferred { syntax });
        }
        let generics = collect_generic_parameters(self.types, [destination])
            .map_err(InferenceFailure::from)
            .map_err(|error| self.inference_error(syntax, error, failure_rule))?;
        let known = !generics
            .iter()
            .any(|parameter| inference_parameters.contains(parameter));
        if let Some(place) = self.positional_value_place(syntax)? {
            if known {
                let value = self.materialize_call_place(syntax, place.id, place.ty, destination)?;
                inference
                    .constrain_contextual(
                        self.types,
                        destination,
                        InferenceEvidence::Typed(self.node_type(value)?),
                    )
                    .map_err(|error| self.inference_error(syntax, error, failure_rule))?;
                return Ok(ValueDraft::Checked { syntax, value });
            }
            inference
                .constrain_contextual(self.types, destination, InferenceEvidence::Typed(place.ty))
                .map_err(|error| self.inference_error(syntax, error, failure_rule))?;
            return Ok(ValueDraft::Place {
                syntax,
                place: place.id,
                ty: place.ty,
            });
        }
        let value = self.check_expression(syntax, known.then_some(destination))?;
        inference
            .constrain_contextual(
                self.types,
                destination,
                InferenceEvidence::Typed(self.node_type(value)?),
            )
            .map_err(|error| self.inference_error(syntax, error, failure_rule))?;
        Ok(ValueDraft::Checked { syntax, value })
    }

    pub(super) fn finish_positional_inference(
        &mut self,
        values: &mut [ValueDraft],
        context: &PositionalValueContext<'_>,
        mut inference: CallableInference,
    ) -> Result<GenericArguments, BodyCheckError> {
        if let Some(result_context) = context.result_context {
            let result = match result_context {
                CallResultContext::Complete(expected) => {
                    inference.constrain_result_contextual(self.types, context.result, expected)
                }
                CallResultContext::OutcomePayload(expected) => {
                    inference.constrain_outcome_payload(self.types, context.result, expected)
                }
                CallResultContext::Propagation(expected) => {
                    inference.constrain_propagation_result(self.types, context.result, expected)
                }
            };
            result.map_err(|error| {
                self.inference_error(context.owner, error, context.failure_rule)
            })?;
        }
        self.infer_closure_drafts(values, context, &mut inference)?;
        inference
            .finish(self.types)
            .map_err(|error| self.inference_error(context.owner, error, context.failure_rule))
    }

    fn infer_closure_drafts(
        &mut self,
        values: &mut [ValueDraft],
        context: &PositionalValueContext<'_>,
        inference: &mut CallableInference,
    ) -> Result<(), BodyCheckError> {
        for value in values.iter() {
            let ValueDraft::Closure {
                syntax,
                contract: Some(contract),
                ..
            } = value
            else {
                continue;
            };
            self.constrain_closure_annotations(*syntax, contract, inference)?;
        }
        let mut remaining = values
            .iter()
            .enumerate()
            .filter_map(|(position, value)| {
                matches!(value, ValueDraft::Closure { .. }).then_some(position)
            })
            .collect::<Vec<_>>();
        while !remaining.is_empty() {
            let partial = inference
                .partial_substitution(self.types)
                .map_err(|error| {
                    self.inference_error(context.owner, error, context.failure_rule)
                })?;
            let ready = remaining
                .iter()
                .position(|position| match &values[*position] {
                    ValueDraft::Closure { contract, .. } => {
                        contract.as_ref().is_none_or(|contract| {
                            self.closure_context_is_ready(
                                contract,
                                context.inference_parameters,
                                &partial,
                            )
                            .unwrap_or(false)
                        })
                    }
                    _ => false,
                })
                .unwrap_or(0);
            let position = remaining.remove(ready);
            let ValueDraft::Closure {
                syntax,
                destination,
                contract,
            } = &values[position]
            else {
                unreachable!("remaining positions contain only closure drafts")
            };
            let syntax = *syntax;
            let destination = *destination;
            let contract = contract.clone();
            let value = self.check_inferred_closure(
                syntax,
                contract.as_ref(),
                context.inference_parameters,
                inference,
                context.failure_rule,
            )?;
            inference.constrain_exact(destination, self.node_type(value)?);
            values[position] = ValueDraft::Checked { syntax, value };
        }
        Ok(())
    }

    pub(super) fn materialize_positional_values(
        &mut self,
        values: Vec<ValueDraft>,
        destination_types: Vec<TypeId>,
        substitution: &TypeSubstitution,
    ) -> Result<Vec<BodyNodeId>, BodyCheckError> {
        values
            .into_iter()
            .zip(destination_types)
            .map(|(value, destination)| {
                let destination = self.apply_type_substitution(substitution, destination)?;
                match value {
                    ValueDraft::Checked { syntax, value } => {
                        self.apply_expected(syntax, value, destination)
                    }
                    ValueDraft::Place { syntax, place, ty } => {
                        self.materialize_call_place(syntax, place, ty, destination)
                    }
                    ValueDraft::Deferred { syntax } => {
                        self.check_expression(syntax, Some(destination))
                    }
                    ValueDraft::Closure { .. } => Err(BodyCheckInternalError::CallInference(
                        InferenceFailure::DuplicateResultContext,
                    )
                    .into()),
                }
            })
            .collect()
    }

    fn positional_value_place(
        &mut self,
        root: NodeId,
    ) -> Result<Option<super::ResolvedPlace>, BodyCheckError> {
        let mut syntax = root;
        while self.kind(syntax).is_ok_and(is_transparent_expression) {
            let children = direct_nodes(self.tree(), syntax);
            let [child] = children.as_slice() else {
                break;
            };
            syntax = *child;
        }
        if self.is_constant_reference(syntax) {
            return Ok(None);
        }
        match self.kind(syntax)? {
            NodeKind::ReferenceExpression => self.named_place(syntax).map(Some),
            NodeKind::PostfixExpression
                if direct_child(self.tree(), syntax, NodeKind::CallSuffix).is_none() =>
            {
                if super::calls::construction_member_syntax(self, syntax)?.is_some() {
                    return Ok(None);
                }
                self.postfix_place(syntax, BorrowCapability::Readonly)
                    .map(Some)
            }
            _ => Ok(None),
        }
    }

    pub(super) fn inference_error(
        &self,
        node: NodeId,
        error: InferenceFailure,
        rule: BodyRule,
    ) -> BodyCheckError {
        match error {
            InferenceFailure::UnknownType(ty) => BodyCheckInternalError::UnknownType(ty).into(),
            InferenceFailure::InvalidSubstitution(error) => {
                BodyCheckInternalError::CallSubstitution(error).into()
            }
            error => self
                .rule(rule, node)
                .unwrap_or_else(|_| BodyCheckInternalError::CallInference(error).into()),
        }
    }
}

fn contextual_callable_contract(
    requirements: &[CheckedRequirement],
    destination: TypeId,
) -> Result<Option<CallableContract>, BodyCheckError> {
    let mut contracts = requirements.iter().filter_map(|requirement| {
        let CheckedPredicate::Callable { subject, contract } = requirement.predicate() else {
            return None;
        };
        (*subject == destination).then(|| contract.clone())
    });
    let selected = contracts.next();
    if contracts.next().is_some() {
        return Err(BodyCheckInternalError::CallContractSelection.into());
    }
    Ok(selected)
}

fn closure_expression(checker: &BodyChecker<'_, '_>, mut node: NodeId) -> Option<NodeId> {
    while checker.kind(node).is_ok_and(is_transparent_expression) {
        let children = direct_nodes(checker.tree(), node);
        let [child] = children.as_slice() else {
            return None;
        };
        node = *child;
    }
    checker
        .kind(node)
        .is_ok_and(|kind| kind == NodeKind::ClosureExpression)
        .then_some(node)
}

fn is_none_expression(checker: &BodyChecker<'_, '_>, mut node: NodeId) -> bool {
    while checker.kind(node).is_ok_and(is_transparent_expression) {
        let children = direct_nodes(checker.tree(), node);
        let [child] = children.as_slice() else {
            return false;
        };
        node = *child;
    }
    checker
        .kind(node)
        .is_ok_and(|kind| kind == NodeKind::ScalarLiteral)
        && direct_token(checker.tree(), node)
            .is_some_and(|token| token.kind() == TokenKind::Keyword(Keyword::None))
}
