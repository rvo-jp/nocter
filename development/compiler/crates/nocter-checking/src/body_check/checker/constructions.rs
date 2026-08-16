use nocter_declarations::{CallableKind, CallableOwner, ExportedEntity};
use nocter_model::{BodyNodeId, GenericParameterId, TypeId, TypeKind};
use nocter_source_index::{SemanticEntity, SourceOrigin};
use nocter_syntax::{NodeId, SyntaxToken};

use super::BodyChecker;
use super::call_planning::DeclaredCallGenerics;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::type_relations::TypeSubstitution;
use crate::{
    CallTarget, CheckedCall, CheckedOperation, GenericArgument, NameTarget, StaticDispatch,
    StaticSelection,
};

enum ConstructionOwnerArguments {
    Inferred,
    Explicit(Box<[TypeId]>),
}

impl BodyChecker<'_, '_> {
    pub(super) fn check_construction_function_call(
        &mut self,
        node: NodeId,
        owner: NodeId,
        member: NodeId,
        call_suffix: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let owner = self.resolve_inferred_construction_owner(owner)?;
        let owner_reference = owner.reference;
        let owner_target = owner.target;
        let construction = match owner_target {
            NameTarget::Exported(ExportedEntity::NominalType(nominal)) => {
                let Some(construction) = self.construction_surfaces.for_nominal(nominal) else {
                    return Err(self.rule(BodyRule::InvalidCall, member)?);
                };
                construction
            }
            NameTarget::Builtin(builtin) => {
                let ty = self.types.builtin(builtin);
                let Some(construction) = self.construction_surfaces.for_type(self.types, ty) else {
                    return Err(self.rule(BodyRule::InvalidCall, member)?);
                };
                construction
            }
            _ => return Err(self.rule(BodyRule::InvalidCall, member)?),
        };
        self.consumed_uses
            .insert(super::calls::call_origin(self, owner_reference)?);

        let member_token = crate::syntax::direct_identifier(self.tree(), member)
            .ok_or(BodyCheckInternalError::InvalidSyntax(member))?;
        self.finish_construction_function_call(
            node,
            construction,
            member_token,
            call_suffix,
            ConstructionOwnerArguments::Inferred,
            expected,
        )
    }

    pub(super) fn check_explicit_construction_function_call(
        &mut self,
        node: NodeId,
        owner: NodeId,
        call_suffix: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let owner = self.resolve_explicit_construction_owner(owner)?;
        let construction = self
            .construction_surfaces
            .for_nominal(owner.definition)
            .ok_or_else(|| self.rule(BodyRule::InvalidCall, node));
        let construction = match construction {
            Ok(construction) => construction,
            Err(error) => return Err(error?),
        };
        self.finish_construction_function_call(
            node,
            construction,
            owner.member,
            call_suffix,
            ConstructionOwnerArguments::Explicit(owner.arguments),
            expected,
        )
    }

    fn finish_construction_function_call(
        &mut self,
        node: NodeId,
        construction: nocter_model::ConstructionId,
        member_token: SyntaxToken,
        call_suffix: NodeId,
        owner_arguments: ConstructionOwnerArguments,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let member_name = self
            .graph
            .symbols()
            .get(self.token_text(member_token)?)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let callable_id = self
            .construction_surfaces
            .named_function(self.graph, construction, member_name, self.source.module())
            .map_err(BodyCheckInternalError::from)?;
        let Some(callable_id) = callable_id else {
            return Err(self.token_rule(BodyRule::InvalidCall, member_token)?);
        };
        let callable = self
            .graph
            .declarations()
            .callables()
            .get(callable_id)
            .cloned()
            .ok_or(BodyCheckInternalError::MissingCallable(callable_id))?;
        if callable.kind() != CallableKind::ConstructionFunction
            || callable.owner() != CallableOwner::Construction(construction)
            || callable.receiver().is_some()
        {
            return Err(BodyCheckInternalError::ConstructionSurfaceSelection(
                crate::ConstructionSurfaceSelectionError::InvalidMember(callable_id),
            )
            .into());
        }
        let construction_declaration = self
            .graph
            .declarations()
            .constructions()
            .get(construction)
            .ok_or(crate::ConstructionSurfaceSelectionError::MissingConstruction(construction))
            .map_err(BodyCheckInternalError::from)?;
        let (inference_parameters, fixed_arguments) = match owner_arguments {
            ConstructionOwnerArguments::Inferred => (
                combined_parameters(
                    construction_declaration.generic_parameters(),
                    callable.generic_parameters(),
                ),
                Vec::new(),
            ),
            ConstructionOwnerArguments::Explicit(arguments) => {
                if construction_declaration.generic_parameters().len() != arguments.len() {
                    return Err(self.rule(BodyRule::InvalidCall, node)?);
                }
                let fixed = construction_declaration
                    .generic_parameters()
                    .iter()
                    .copied()
                    .zip(arguments)
                    .map(|(parameter, ty)| GenericArgument::new(parameter, ty))
                    .collect();
                (callable.generic_parameters().to_vec(), fixed)
            }
        };
        let plan = self.plan_declared_call(
            node,
            call_suffix,
            callable_id,
            &callable,
            DeclaredCallGenerics::with_fixed(&inference_parameters, &fixed_arguments),
            expected,
        )?;
        if !self.construction_target_requirements_hold(
            construction_declaration.target(),
            &plan.generic_arguments,
        )? {
            return Err(self.rule(BodyRule::InvalidCall, node)?);
        }
        self.project_construction_member(member_token, callable_id)?;
        let call = self.add_node(
            node,
            plan.result,
            CheckedOperation::Call(CheckedCall::new(
                CallTarget::Static(StaticSelection::new(
                    StaticDispatch::Direct(callable_id),
                    plan.generic_arguments,
                )),
                None,
                plan.arguments,
            )),
        )?;
        expected.map_or(Ok(call), |expected| {
            self.apply_expected(node, call, expected)
        })
    }

    fn project_construction_member(
        &mut self,
        token: SyntaxToken,
        callable: nocter_model::CallableId,
    ) -> Result<(), BodyCheckInternalError> {
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        self.projections.push(super::NodeProjection {
            entity: SemanticEntity::Callable(callable),
            origin,
        });
        Ok(())
    }

    fn construction_target_requirements_hold(
        &mut self,
        target: TypeId,
        arguments: &crate::GenericArguments,
    ) -> Result<bool, BodyCheckError> {
        let mut construction_substitution = TypeSubstitution::default();
        for argument in arguments.as_slice() {
            construction_substitution.bind_generic(argument.parameter(), argument.ty());
        }
        let target = construction_substitution
            .apply_type(self.types, target)
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let Some(TypeKind::Nominal {
            definition,
            arguments,
        }) = self.types.get(target)
        else {
            return Ok(true);
        };
        let nominal = self
            .graph
            .declarations()
            .nominal_types()
            .get(*definition)
            .ok_or(BodyCheckInternalError::UnknownType(target))?;
        if nominal.generic_parameters().len() != arguments.len() {
            return Err(BodyCheckInternalError::UnknownType(target).into());
        }
        let requirements = nominal.requirements().to_vec();
        let mut substitution = TypeSubstitution::default();
        for (parameter, argument) in nominal
            .generic_parameters()
            .iter()
            .copied()
            .zip(arguments.iter().copied())
        {
            substitution.bind_generic(parameter, argument);
        }
        self.requirements_hold(&requirements, &substitution)
    }
}

fn combined_parameters(
    owner: &[GenericParameterId],
    callable: &[GenericParameterId],
) -> Vec<GenericParameterId> {
    owner.iter().chain(callable).copied().collect::<Vec<_>>()
}
