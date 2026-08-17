use nocter_declarations::{
    CallableKind, CallableOwner, ExportedEntity, ParameterOwner, ParameterRole,
};
use nocter_model::{BodyNodeId, GenericParameterId, TypeId, TypeKind, VariantId};
use nocter_source_index::{SemanticEntity, SourceOrigin};
use nocter_syntax::{NodeId, SyntaxToken};

use super::BodyChecker;
use super::call_planning::DeclaredCallGenerics;
use super::construction_planning::bind_inferred_arguments;
use super::type_uses::{NominalConstructionOwner, NominalOwnerArguments};
use super::value_planning::PositionalValueContext;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::type_relations::TypeSubstitution;
use crate::{
    AggregateConstruction, CallTarget, CheckedCall, CheckedOperation, GenericArgument, NameTarget,
    StaticDispatch, StaticSelection,
};

enum ConstructionOwnerArguments {
    Inferred,
    Explicit(Box<[TypeId]>),
}

enum VariantInvocation {
    Member,
    Call(Vec<NodeId>),
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
        let member_token = crate::syntax::direct_identifier(self.tree(), member)
            .ok_or(BodyCheckInternalError::InvalidSyntax(member))?;
        let member_name = self
            .graph
            .symbols()
            .get(self.token_text(member_token)?)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        self.consumed_uses
            .insert(super::calls::call_origin(self, owner_reference)?);
        let construction = match owner_target {
            NameTarget::Exported(ExportedEntity::NominalType(nominal)) => {
                if let Some(variant) = self
                    .construction_surfaces
                    .variant(nominal, member_name)
                    .map_err(BodyCheckInternalError::from)?
                {
                    let owner = self.inferred_nominal_construction_type(nominal)?;
                    return self.finish_variant_construction(
                        node,
                        owner,
                        variant,
                        member_token,
                        VariantInvocation::Call(crate::syntax::direct_nodes(
                            self.tree(),
                            call_suffix,
                        )),
                        expected,
                    );
                }
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
        let member_name = self
            .graph
            .symbols()
            .get(self.token_text(owner.member)?)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        if let Some(variant) = self
            .construction_surfaces
            .variant(owner.definition, member_name)
            .map_err(BodyCheckInternalError::from)?
        {
            return self.finish_variant_construction(
                node,
                NominalConstructionOwner {
                    definition: owner.definition,
                    arguments: NominalOwnerArguments::Fixed(owner.arguments),
                },
                variant,
                owner.member,
                VariantInvocation::Call(crate::syntax::direct_nodes(self.tree(), call_suffix)),
                expected,
            );
        }
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

    pub(super) fn check_inferred_construction_member(
        &mut self,
        node: NodeId,
        owner: NodeId,
        member: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let owner = self.resolve_inferred_construction_owner(owner)?;
        let reference = owner.reference;
        let NameTarget::Exported(ExportedEntity::NominalType(nominal)) = owner.target else {
            return Err(self.rule(BodyRule::InvalidConstruction, node)?);
        };
        self.consumed_uses
            .insert(super::calls::call_origin(self, reference)?);
        let token = crate::syntax::direct_identifier(self.tree(), member)
            .ok_or(BodyCheckInternalError::InvalidSyntax(member))?;
        let name = self.segment_symbol(token)?;
        let Some(variant) = self
            .construction_surfaces
            .variant(nominal, name)
            .map_err(BodyCheckInternalError::from)?
        else {
            return Err(self.token_rule(BodyRule::InvalidConstruction, token)?);
        };
        let owner = self.inferred_nominal_construction_type(nominal)?;
        self.finish_variant_construction(
            node,
            owner,
            variant,
            token,
            VariantInvocation::Member,
            expected,
        )
    }

    pub(super) fn check_explicit_construction_member(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let owner = self.resolve_explicit_construction_owner(node)?;
        let name = self.segment_symbol(owner.member)?;
        let Some(variant) = self
            .construction_surfaces
            .variant(owner.definition, name)
            .map_err(BodyCheckInternalError::from)?
        else {
            return Err(self.token_rule(BodyRule::InvalidConstruction, owner.member)?);
        };
        self.finish_variant_construction(
            node,
            NominalConstructionOwner {
                definition: owner.definition,
                arguments: NominalOwnerArguments::Fixed(owner.arguments),
            },
            variant,
            owner.member,
            VariantInvocation::Member,
            expected,
        )
    }

    fn finish_variant_construction(
        &mut self,
        node: NodeId,
        owner: NominalConstructionOwner,
        variant: VariantId,
        member_token: SyntaxToken,
        invocation: VariantInvocation,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let mut plan = self.nominal_construction_plan(node, owner)?;
        if plan.definition
            != self
                .graph
                .declarations()
                .variants()
                .get(variant)
                .ok_or(BodyCheckInternalError::InvalidSyntax(node))?
                .owner()
        {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let payload = self
            .graph
            .declarations()
            .variants()
            .get(variant)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?
            .payload()
            .to_vec();
        let (called, argument_syntax) = match invocation {
            VariantInvocation::Member => (false, Vec::new()),
            VariantInvocation::Call(arguments) => (true, arguments),
        };
        if called == payload.is_empty() {
            return Err(self.token_rule(BodyRule::InvalidConstruction, member_token)?);
        }
        if payload.len() != argument_syntax.len() {
            return Err(self.token_rule(BodyRule::InvalidConstruction, member_token)?);
        }
        let destination_types = payload
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                let parameter = self
                    .graph
                    .declarations()
                    .parameters()
                    .get(*parameter)
                    .copied()
                    .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
                if parameter.owner() != ParameterOwner::Variant(variant)
                    || parameter.role()
                        != (ParameterRole::Ordinary {
                            position,
                            variadic: false,
                        })
                {
                    return Err(BodyCheckInternalError::InvalidSyntax(node));
                }
                plan.substitution
                    .apply_type(self.types, parameter.ty())
                    .map_err(BodyCheckInternalError::CallSubstitution)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result_pattern = plan
            .substitution
            .apply_type(self.types, plan.result_pattern)
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let (drafts, inferred) = self.infer_positional_values(
            argument_syntax,
            PositionalValueContext {
                owner: node,
                result: result_pattern,
                inference_parameters: &plan.inference_parameters,
                destination_types: &destination_types,
                requirements: &[],
                expected,
                failure_rule: BodyRule::InvalidConstruction,
            },
        )?;
        bind_inferred_arguments(&mut plan.substitution, &inferred);
        if !self.nominal_construction_requirements_hold(plan.definition, &plan.substitution)? {
            return Err(self.rule(BodyRule::InvalidConstruction, node)?);
        }
        let values =
            self.materialize_positional_values(drafts, destination_types, &plan.substitution)?;
        let ty = plan
            .substitution
            .apply_type(self.types, plan.result_pattern)
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        self.project_variant_member(member_token, variant)?;
        let aggregate = self.add_node(
            node,
            ty,
            CheckedOperation::Aggregate(AggregateConstruction::Enum {
                variant,
                payload: values.into_boxed_slice(),
            }),
        )?;
        expected.map_or(Ok(aggregate), |expected| {
            self.apply_expected(node, aggregate, expected)
        })
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

    fn project_variant_member(
        &mut self,
        token: SyntaxToken,
        variant: VariantId,
    ) -> Result<(), BodyCheckInternalError> {
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        self.projections.push(super::NodeProjection {
            entity: SemanticEntity::Variant(variant),
            origin,
        });
        Ok(())
    }

    pub(super) fn construction_target_requirements_hold(
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
