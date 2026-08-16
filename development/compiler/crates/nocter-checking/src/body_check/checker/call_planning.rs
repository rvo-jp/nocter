use nocter_declarations::{CallableDeclaration, ExportedEntity, ParameterRole};
use nocter_model::{BodyNodeId, BorrowCapability, GenericParameterId, PlaceId, TypeId};
use nocter_syntax::{Keyword, NodeId, NodeKind, TokenKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::conformance::normalize_requirements;
use crate::instance_operations::InstanceOperationSelector;
use crate::syntax::{direct_child, direct_nodes, direct_token, is_transparent_expression};
use crate::type_relations::{TypeSubstitution, collect_generic_parameters};
use crate::{CallableInference, GenericArguments, InferenceEvidence, InferenceFailure, NameTarget};

enum ArgumentDraft {
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

pub(super) struct DeclaredCallPlan {
    pub(super) arguments: Vec<BodyNodeId>,
    pub(super) generic_arguments: GenericArguments,
    pub(super) result: TypeId,
}

impl BodyChecker<'_, '_> {
    pub(super) fn plan_declared_call(
        &mut self,
        node: NodeId,
        suffix: NodeId,
        callable_id: nocter_model::CallableId,
        callable: &CallableDeclaration,
        inference_parameters: &[GenericParameterId],
        expected: Option<TypeId>,
    ) -> Result<DeclaredCallPlan, BodyCheckError> {
        let argument_syntax = direct_nodes(self.tree(), suffix);
        let parameter_types =
            self.declared_parameter_types(callable_id, callable, suffix, argument_syntax.len())?;
        let (arguments, generic_arguments, substitution) = self.infer_declared_arguments(
            node,
            callable,
            inference_parameters,
            argument_syntax,
            &parameter_types,
            expected,
        )?;
        if !self.declared_requirements_hold(callable, &substitution)? {
            return Err(self.rule(BodyRule::InvalidCall, node)?);
        }
        let arguments =
            self.materialize_declared_arguments(arguments, parameter_types, &substitution)?;
        let result = substitution
            .apply_type(self.types, callable.result())
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        Ok(DeclaredCallPlan {
            arguments,
            generic_arguments,
            result,
        })
    }

    fn declared_parameter_types(
        &self,
        callable_id: nocter_model::CallableId,
        callable: &CallableDeclaration,
        suffix: NodeId,
        argument_count: usize,
    ) -> Result<Vec<TypeId>, BodyCheckError> {
        if argument_count != callable.parameters().len() {
            return Err(self.rule(BodyRule::InvalidCall, suffix)?);
        }
        callable
            .parameters()
            .iter()
            .copied()
            .map(|parameter| {
                let parameter = self
                    .graph
                    .declarations()
                    .parameters()
                    .get(parameter)
                    .copied()
                    .ok_or(BodyCheckInternalError::MissingParameterType(
                        NameTarget::Exported(ExportedEntity::Callable(callable_id)),
                    ))?;
                if !matches!(
                    parameter.role(),
                    ParameterRole::Ordinary {
                        variadic: false,
                        ..
                    }
                ) {
                    return Err(BodyCheckInternalError::UnsupportedSyntax(
                        suffix,
                        NodeKind::CallSuffix,
                    ));
                }
                Ok(parameter.ty())
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn infer_declared_arguments(
        &mut self,
        node: NodeId,
        callable: &CallableDeclaration,
        inference_parameters: &[GenericParameterId],
        argument_syntax: Vec<NodeId>,
        parameter_types: &[TypeId],
        expected: Option<TypeId>,
    ) -> Result<(Vec<ArgumentDraft>, GenericArguments, TypeSubstitution), BodyCheckError> {
        let mut inference = CallableInference::new(inference_parameters);
        let mut arguments = Vec::with_capacity(argument_syntax.len());
        for (syntax, parameter) in argument_syntax
            .into_iter()
            .zip(parameter_types.iter().copied())
        {
            if is_none_expression(self, syntax) {
                inference
                    .constrain_contextual(self.types, parameter, InferenceEvidence::Absent)
                    .map_err(|error| self.call_inference_error(syntax, error))?;
                arguments.push(ArgumentDraft::Deferred { syntax });
                continue;
            }
            let generics = collect_generic_parameters(self.types, [parameter])
                .map_err(InferenceFailure::from)
                .map_err(|error| self.call_inference_error(syntax, error))?;
            let known = !generics
                .iter()
                .any(|parameter| inference_parameters.contains(parameter));
            if !known && let Some(place) = self.declared_argument_place(syntax)? {
                inference
                    .constrain_contextual(self.types, parameter, InferenceEvidence::Typed(place.ty))
                    .map_err(|error| self.call_inference_error(syntax, error))?;
                arguments.push(ArgumentDraft::Place {
                    syntax,
                    place: place.id,
                    ty: place.ty,
                });
                continue;
            }
            let value = self.check_expression(syntax, known.then_some(parameter))?;
            inference
                .constrain_contextual(
                    self.types,
                    parameter,
                    InferenceEvidence::Typed(self.node_type(value)?),
                )
                .map_err(|error| self.call_inference_error(syntax, error))?;
            arguments.push(ArgumentDraft::Checked { syntax, value });
        }
        if let Some(expected) = expected {
            inference
                .constrain_result_contextual(self.types, callable.result(), expected)
                .map_err(|error| self.call_inference_error(node, error))?;
        }
        let generic_arguments = inference
            .finish(self.types)
            .map_err(|error| self.call_inference_error(node, error))?;
        let mut substitution = TypeSubstitution::default();
        for argument in generic_arguments.as_slice() {
            substitution.bind_generic(argument.parameter(), argument.ty());
        }
        Ok((arguments, generic_arguments, substitution))
    }

    fn declared_requirements_hold(
        &mut self,
        callable: &CallableDeclaration,
        substitution: &TypeSubstitution,
    ) -> Result<bool, BodyCheckError> {
        let requirements = normalize_requirements(
            self.graph,
            self.types,
            substitution,
            callable.requirements(),
        )
        .map_err(BodyCheckInternalError::CallSubstitution)?;
        let mut selector = InstanceOperationSelector::new(
            self.graph,
            self.types,
            self.conformances,
            self.copyabilities,
            self.instance_operations,
            &self.assumptions,
            self.source.module(),
        );
        selector
            .requirements_hold(&requirements, &TypeSubstitution::default())
            .map_err(BodyCheckInternalError::from)
            .map_err(Into::into)
    }

    fn materialize_declared_arguments(
        &mut self,
        arguments: Vec<ArgumentDraft>,
        parameter_types: Vec<TypeId>,
        substitution: &TypeSubstitution,
    ) -> Result<Vec<BodyNodeId>, BodyCheckError> {
        arguments
            .into_iter()
            .zip(parameter_types)
            .map(|(argument, parameter)| {
                let parameter = substitution
                    .apply_type(self.types, parameter)
                    .map_err(BodyCheckInternalError::CallSubstitution)?;
                match argument {
                    ArgumentDraft::Checked { syntax, value } => {
                        self.apply_expected(syntax, value, parameter)
                    }
                    ArgumentDraft::Place { syntax, place, ty } => {
                        self.apply_expected_place(syntax, place, ty, parameter)
                    }
                    ArgumentDraft::Deferred { syntax } => {
                        self.check_expression(syntax, Some(parameter))
                    }
                }
            })
            .collect()
    }

    fn declared_argument_place(
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

    fn call_inference_error(&self, node: NodeId, error: InferenceFailure) -> BodyCheckError {
        match error {
            InferenceFailure::UnknownType(ty) => BodyCheckInternalError::UnknownType(ty).into(),
            InferenceFailure::InvalidSubstitution(error) => {
                BodyCheckInternalError::CallSubstitution(error).into()
            }
            error => self
                .rule(BodyRule::InvalidCall, node)
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
