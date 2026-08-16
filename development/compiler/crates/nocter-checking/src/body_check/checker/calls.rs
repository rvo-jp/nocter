use nocter_declarations::{
    CallableDeclaration, CallableKind, CallableOwner, ExportedEntity, ParameterRole,
};
use nocter_model::{BodyNodeId, TypeId};
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{Keyword, NodeId, NodeKind, SyntaxToken, TokenKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::conformance::normalize_requirements;
use crate::instance_operations::InstanceOperationSelector;
use crate::syntax::{direct_identifier, direct_nodes, direct_token, is_transparent_expression};
use crate::type_relations::{TypeSubstitution, collect_generic_parameters};
use crate::{
    CallTarget, CallableInference, CheckedCall, CheckedOperation, GenericArguments,
    InferenceEvidence, InferenceFailure, NameTarget, StaticDispatch, StaticSelection,
};

enum ArgumentDraft {
    Checked { syntax: NodeId, value: BodyNodeId },
    Deferred { syntax: NodeId },
}

impl BodyChecker<'_, '_> {
    pub(super) fn check_static_call(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let (reference, suffix) = direct_call_syntax(self, node)?;
        let callable_id = self.consume_static_callable(reference, suffix)?;
        let callable = self
            .graph
            .declarations()
            .callables()
            .get(callable_id)
            .cloned()
            .ok_or(BodyCheckInternalError::MissingCallable(callable_id))?;
        if !matches!(callable.owner(), CallableOwner::Module(_))
            || !matches!(
                callable.kind(),
                CallableKind::Function | CallableKind::Primitive
            )
            || callable.receiver().is_some()
        {
            return Err(
                BodyCheckInternalError::UnsupportedSyntax(suffix, NodeKind::CallSuffix).into(),
            );
        }

        let argument_syntax = direct_nodes(self.tree(), suffix);
        let parameter_types =
            self.static_parameter_types(callable_id, &callable, suffix, argument_syntax.len())?;
        let (arguments, generic_arguments, substitution) = self.infer_static_arguments(
            node,
            &callable,
            argument_syntax,
            &parameter_types,
            expected,
        )?;
        if !self.static_requirements_hold(&callable, &substitution)? {
            return Err(self.rule(BodyRule::InvalidCall, node)?);
        }
        let checked_arguments =
            self.materialize_static_arguments(arguments, parameter_types, &substitution)?;
        let result = substitution
            .apply_type(self.types, callable.result())
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let call = self.add_node(
            node,
            result,
            CheckedOperation::Call(CheckedCall::new(
                CallTarget::Static(StaticSelection::new(
                    StaticDispatch::Direct(callable_id),
                    generic_arguments,
                )),
                None,
                checked_arguments,
            )),
        )?;
        expected.map_or(Ok(call), |expected| {
            self.apply_expected(node, call, expected)
        })
    }

    fn static_parameter_types(
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

    fn infer_static_arguments(
        &mut self,
        node: NodeId,
        callable: &CallableDeclaration,
        argument_syntax: Vec<NodeId>,
        parameter_types: &[TypeId],
        expected: Option<TypeId>,
    ) -> Result<(Vec<ArgumentDraft>, GenericArguments, TypeSubstitution), BodyCheckError> {
        let mut inference = CallableInference::new(callable.generic_parameters());
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
                .any(|parameter| callable.generic_parameters().contains(parameter));
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

    fn static_requirements_hold(
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

    fn materialize_static_arguments(
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
                    ArgumentDraft::Deferred { syntax } => {
                        self.check_expression(syntax, Some(parameter))
                    }
                }
            })
            .collect()
    }

    fn consume_static_callable(
        &mut self,
        reference: NodeId,
        suffix: NodeId,
    ) -> Result<nocter_model::CallableId, BodyCheckError> {
        let token = direct_identifier(self.tree(), reference)
            .or_else(|| identifier(self, reference))
            .ok_or(BodyCheckInternalError::InvalidSyntax(reference))?;
        let origin = SyntaxOrigin::Token(token);
        let target = self
            .uses
            .get(&origin)
            .copied()
            .ok_or(BodyCheckInternalError::MissingNameUse(reference))?;
        self.consumed_uses.insert(origin);
        match target {
            NameTarget::Exported(ExportedEntity::Callable(callable)) => Ok(callable),
            NameTarget::Parameter(_) | NameTarget::Local(_) | NameTarget::Capture(_) => {
                Err(BodyCheckInternalError::UnsupportedSyntax(suffix, NodeKind::CallSuffix).into())
            }
            NameTarget::Exported(_) | NameTarget::Builtin(_) => {
                Err(self.rule(BodyRule::InvalidCall, reference)?)
            }
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

fn direct_call_syntax(
    checker: &BodyChecker<'_, '_>,
    node: NodeId,
) -> Result<(NodeId, NodeId), BodyCheckInternalError> {
    let children = direct_nodes(checker.tree(), node);
    let [callee, suffix] = children.as_slice() else {
        return Err(BodyCheckInternalError::InvalidSyntax(node));
    };
    if checker.kind(*suffix)? != NodeKind::CallSuffix {
        return Err(BodyCheckInternalError::InvalidSyntax(*suffix));
    }
    let mut reference = *callee;
    while checker.kind(reference).is_ok_and(is_transparent_expression) {
        let children = direct_nodes(checker.tree(), reference);
        let [child] = children.as_slice() else {
            break;
        };
        reference = *child;
    }
    if checker.kind(reference)? != NodeKind::ReferenceExpression {
        return Err(BodyCheckInternalError::UnsupportedSyntax(
            *suffix,
            NodeKind::CallSuffix,
        ));
    }
    Ok((reference, *suffix))
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

fn identifier(checker: &BodyChecker<'_, '_>, node: NodeId) -> Option<SyntaxToken> {
    let mut found = crate::syntax::identifier_tokens(checker.tree(), node).into_iter();
    let token = found.next()?;
    found.next().is_none().then_some(token)
}
