use nocter_declarations::{
    CallableDeclaration, ExportedEntity, ParameterRole, StructuralCapability,
};
use nocter_model::{BodyNodeId, GenericParameterId, TypeId, TypeKind};
use nocter_syntax::{NodeId, NodeKind};

use super::BodyChecker;
use super::value_planning::PositionalValueContext;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::conformance::normalize_requirements;
use crate::instance_operations::InstanceOperationSelector;
use crate::syntax::direct_nodes;
use crate::type_relations::TypeSubstitution;
use crate::{CheckedPredicate, GenericArgument, GenericArguments, NameTarget};

pub(super) struct DeclaredCallPlan {
    pub(super) arguments: Vec<BodyNodeId>,
    pub(super) generic_arguments: GenericArguments,
    pub(super) result: TypeId,
}

#[derive(Clone, Copy)]
pub(super) struct DeclaredCallGenerics<'a> {
    pub(super) inference_parameters: &'a [GenericParameterId],
    pub(super) fixed_arguments: &'a [GenericArgument],
    pub(super) owner_substitution: Option<&'a TypeSubstitution>,
}

impl<'a> DeclaredCallGenerics<'a> {
    pub(super) const fn inferred(inference_parameters: &'a [GenericParameterId]) -> Self {
        Self {
            inference_parameters,
            fixed_arguments: &[],
            owner_substitution: None,
        }
    }

    pub(super) const fn with_fixed(
        inference_parameters: &'a [GenericParameterId],
        fixed_arguments: &'a [GenericArgument],
    ) -> Self {
        Self {
            inference_parameters,
            fixed_arguments,
            owner_substitution: None,
        }
    }

    pub(super) const fn specialized(
        inference_parameters: &'a [GenericParameterId],
        fixed_arguments: &'a [GenericArgument],
        owner_substitution: &'a TypeSubstitution,
    ) -> Self {
        Self {
            inference_parameters,
            fixed_arguments,
            owner_substitution: Some(owner_substitution),
        }
    }
}

impl BodyChecker<'_, '_> {
    pub(super) fn plan_declared_call(
        &mut self,
        node: NodeId,
        suffix: NodeId,
        callable_id: nocter_model::CallableId,
        callable: &CallableDeclaration,
        generics: DeclaredCallGenerics<'_>,
        expected: Option<TypeId>,
    ) -> Result<DeclaredCallPlan, BodyCheckError> {
        let argument_syntax = direct_nodes(self.tree(), suffix);
        let mut substitution = generics.owner_substitution.cloned().unwrap_or_default();
        for argument in generics.fixed_arguments {
            substitution.bind_generic(argument.parameter(), argument.ty());
        }
        let parameter_types = self
            .declared_parameter_types(callable_id, callable, suffix, argument_syntax.len())?
            .into_iter()
            .map(|parameter| {
                substitution
                    .apply_type(self.types, parameter)
                    .map_err(BodyCheckInternalError::CallSubstitution)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = substitution
            .apply_type(self.types, callable.result())
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let requirements = normalize_requirements(
            self.graph,
            self.types,
            &substitution,
            callable.requirements(),
        )
        .map_err(BodyCheckInternalError::CallSubstitution)?;
        let (arguments, inferred_arguments) = self.infer_positional_values(
            argument_syntax,
            PositionalValueContext {
                owner: node,
                result,
                inference_parameters: generics.inference_parameters,
                destination_types: &parameter_types,
                requirements: &requirements,
                expected,
                failure_rule: BodyRule::InvalidCall,
            },
        )?;
        for argument in inferred_arguments.as_slice() {
            substitution.bind_generic(argument.parameter(), argument.ty());
        }
        let generic_arguments = GenericArguments::new(
            generics
                .fixed_arguments
                .iter()
                .copied()
                .chain(inferred_arguments.as_slice().iter().copied()),
        )
        .map_err(BodyCheckInternalError::CallGenericArguments)?;
        if !self.requirements_hold(callable.requirements(), &substitution)? {
            return Err(self.rule(BodyRule::InvalidCall, node)?);
        }
        let arguments =
            self.materialize_positional_values(arguments, parameter_types, &substitution)?;
        let result = substitution
            .apply_type(self.types, result)
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

    pub(super) fn requirements_hold(
        &mut self,
        requirements: &[nocter_model::RequirementId],
        substitution: &TypeSubstitution,
    ) -> Result<bool, BodyCheckError> {
        let requirements =
            normalize_requirements(self.graph, self.types, substitution, requirements)
                .map_err(BodyCheckInternalError::CallSubstitution)?;
        let mut ordinary = Vec::with_capacity(requirements.len());
        for requirement in requirements {
            let CheckedPredicate::Capability {
                subject,
                capability: StructuralCapability::Callable(contract),
            } = requirement.predicate()
            else {
                ordinary.push(requirement);
                continue;
            };
            let Some(TypeKind::Closure(closure)) = self.types.get(*subject) else {
                ordinary.push(requirement);
                continue;
            };
            let closure = *closure;
            let signature = self
                .closures
                .get(closure)
                .ok_or(BodyCheckInternalError::MissingClosure(closure))?
                .signature()
                .clone();
            if !super::closures::concrete_closure_satisfies(contract, &signature) {
                return Ok(false);
            }
            self.closures
                .require_callable(closure, contract.clone())
                .map_err(BodyCheckInternalError::from)?;
        }
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
            .requirements_hold(&ordinary, &TypeSubstitution::default())
            .map_err(BodyCheckInternalError::from)
            .map_err(Into::into)
    }
}
