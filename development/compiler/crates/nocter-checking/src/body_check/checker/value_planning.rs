use nocter_model::{BodyNodeId, BorrowCapability, GenericParameterId, PlaceId, TypeId};
use nocter_syntax::{Keyword, NodeId, NodeKind, TokenKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::{direct_child, direct_nodes, direct_token, is_transparent_expression};
use crate::type_relations::{TypeSubstitution, collect_generic_parameters};
use crate::{CallableInference, GenericArguments, InferenceEvidence, InferenceFailure};

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
}

#[derive(Clone, Copy)]
pub(super) struct PositionalValueContext<'a> {
    pub(super) owner: NodeId,
    pub(super) result: TypeId,
    pub(super) inference_parameters: &'a [GenericParameterId],
    pub(super) destination_types: &'a [TypeId],
    pub(super) expected: Option<TypeId>,
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
            if is_none_expression(self, syntax) {
                inference
                    .constrain_contextual(self.types, destination, InferenceEvidence::Absent)
                    .map_err(|error| self.inference_error(syntax, error, context.failure_rule))?;
                values.push(ValueDraft::Deferred { syntax });
                continue;
            }
            let generics = collect_generic_parameters(self.types, [destination])
                .map_err(InferenceFailure::from)
                .map_err(|error| self.inference_error(syntax, error, context.failure_rule))?;
            let known = !generics
                .iter()
                .any(|parameter| context.inference_parameters.contains(parameter));
            if !known && let Some(place) = self.positional_value_place(syntax)? {
                inference
                    .constrain_contextual(
                        self.types,
                        destination,
                        InferenceEvidence::Typed(place.ty),
                    )
                    .map_err(|error| self.inference_error(syntax, error, context.failure_rule))?;
                values.push(ValueDraft::Place {
                    syntax,
                    place: place.id,
                    ty: place.ty,
                });
                continue;
            }
            let value = self.check_expression(syntax, known.then_some(destination))?;
            inference
                .constrain_contextual(
                    self.types,
                    destination,
                    InferenceEvidence::Typed(self.node_type(value)?),
                )
                .map_err(|error| self.inference_error(syntax, error, context.failure_rule))?;
            values.push(ValueDraft::Checked { syntax, value });
        }
        if let Some(expected) = context.expected {
            inference
                .constrain_result_contextual(self.types, context.result, expected)
                .map_err(|error| {
                    self.inference_error(context.owner, error, context.failure_rule)
                })?;
        }
        let generic_arguments = inference
            .finish(self.types)
            .map_err(|error| self.inference_error(context.owner, error, context.failure_rule))?;
        Ok((values, generic_arguments))
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
                let destination = substitution
                    .apply_type(self.types, destination)
                    .map_err(BodyCheckInternalError::CallSubstitution)?;
                match value {
                    ValueDraft::Checked { syntax, value } => {
                        self.apply_expected(syntax, value, destination)
                    }
                    ValueDraft::Place { syntax, place, ty } => {
                        self.apply_expected_place(syntax, place, ty, destination)
                    }
                    ValueDraft::Deferred { syntax } => {
                        self.check_expression(syntax, Some(destination))
                    }
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
        match self.kind(syntax)? {
            NodeKind::ReferenceExpression => self.named_place(syntax).map(Some),
            NodeKind::PostfixExpression
                if direct_child(self.tree(), syntax, NodeKind::CallSuffix).is_none() =>
            {
                self.postfix_place(syntax, BorrowCapability::Readonly)
                    .map(Some)
            }
            _ => Ok(None),
        }
    }

    fn inference_error(
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
