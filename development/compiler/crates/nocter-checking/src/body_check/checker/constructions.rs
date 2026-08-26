use nocter_declarations::{ExportedEntity, ParameterOwner, ParameterRole};
use nocter_model::{BodyNodeId, GenericParameterId, TypeId, TypeKind, VariantId};
use nocter_source_index::{SemanticEntity, SourceOrigin};
use nocter_syntax::{NodeId, SyntaxToken};

use super::BodyChecker;
use super::call_planning::DeclaredCallGenerics;
use super::construction_planning::bind_inferred_arguments;
use super::type_uses::{NominalConstructionOwner, NominalOwnerArguments};
use super::value_planning::{CallResultContext, PositionalValueContext};
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::type_relations::TypeSubstitution;
use crate::{
    AggregateConstruction, CallTarget, CheckedCall, CheckedOperation, ConstructionCompletionOwner,
    GenericArgument, NameTarget, StaticDispatch, StaticSelection, TypedBodyInterruption,
    TypedBodyInterruptionKind,
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
        result_context: Option<CallResultContext>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let owner = self.resolve_inferred_construction_owner(owner)?;
        let owner_reference = owner.reference;
        let owner_target = owner.target;
        let completion_owner = construction_completion_owner(owner_target);
        let Some(member_token) = crate::syntax::direct_identifier(self.tree(), member) else {
            if let Some(owner) = completion_owner {
                self.record_construction_interruption_node(member, owner)?;
            }
            return Err(BodyCheckInternalError::InvalidSyntax(member).into());
        };
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
                    .variant(
                        self.graph,
                        nominal,
                        member_name,
                        self.source_access_context(),
                    )
                    .map_err(BodyCheckInternalError::from)?
                {
                    let owner = self.inferred_nominal_construction_type(nominal)?;
                    return self.finish_variant_construction(
                        node,
                        owner,
                        variant,
                        member_token,
                        VariantInvocation::Call(crate::syntax::child_nodes(
                            self.tree(),
                            call_suffix,
                        )),
                        result_context.and_then(CallResultContext::complete_type),
                    );
                }
                let Some(construction) = self.construction_surfaces.for_nominal(nominal) else {
                    self.record_construction_interruption(member_token, completion_owner)?;
                    return Err(self.rule(BodyRule::InvalidCall, member)?);
                };
                construction
            }
            NameTarget::Exported(ExportedEntity::BuiltinType(builtin)) => {
                let ty = self.types.builtin(builtin);
                let Some(construction) = self.construction_surfaces.for_type(self.types, ty) else {
                    self.record_construction_interruption(member_token, completion_owner)?;
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
            result_context,
        )
    }

    pub(super) fn check_explicit_construction_function_call(
        &mut self,
        node: NodeId,
        owner: NodeId,
        call_suffix: NodeId,
        result_context: Option<CallResultContext>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        if let Some(incomplete) = self.resolve_incomplete_explicit_construction_owner(owner)? {
            self.record_construction_interruption_node(
                owner,
                ConstructionCompletionOwner::Nominal(incomplete.definition),
            )?;
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let owner = self.resolve_explicit_construction_owner(owner)?;
        let member_name = self
            .graph
            .symbols()
            .get(self.token_text(owner.member)?)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        if let Some(variant) = self
            .construction_surfaces
            .variant(
                self.graph,
                owner.definition,
                member_name,
                self.source_access_context(),
            )
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
                VariantInvocation::Call(crate::syntax::child_nodes(self.tree(), call_suffix)),
                result_context.and_then(CallResultContext::complete_type),
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
            result_context,
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
        let completion_owner = construction_completion_owner(owner.target);
        let Some(token) = crate::syntax::direct_identifier(self.tree(), member) else {
            if let Some(owner) = completion_owner {
                self.record_construction_interruption_node(member, owner)?;
            }
            return Err(BodyCheckInternalError::InvalidSyntax(member).into());
        };
        let NameTarget::Exported(ExportedEntity::NominalType(nominal)) = owner.target else {
            self.record_construction_interruption(token, completion_owner)?;
            return Err(self.rule(BodyRule::InvalidConstruction, node)?);
        };
        self.consumed_uses
            .insert(super::calls::call_origin(self, reference)?);
        let name = self.segment_symbol(token)?;
        let Some(variant) = self
            .construction_surfaces
            .variant(self.graph, nominal, name, self.source_access_context())
            .map_err(BodyCheckInternalError::from)?
        else {
            self.record_construction_interruption(token, completion_owner)?;
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
        if let Some(owner) = self.resolve_incomplete_explicit_construction_owner(node)? {
            self.record_construction_interruption_node(
                node,
                ConstructionCompletionOwner::Nominal(owner.definition),
            )?;
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let owner = self.resolve_explicit_construction_owner(node)?;
        let name = self.segment_symbol(owner.member)?;
        let Some(variant) = self
            .construction_surfaces
            .variant(
                self.graph,
                owner.definition,
                name,
                self.source_access_context(),
            )
            .map_err(BodyCheckInternalError::from)?
        else {
            self.record_construction_interruption(
                owner.member,
                Some(ConstructionCompletionOwner::Nominal(owner.definition)),
            )?;
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
                    || parameter.role() != (ParameterRole::Ordinary { position })
                {
                    return Err(BodyCheckInternalError::InvalidSyntax(node));
                }
                self.apply_type_substitution(&plan.substitution, parameter.ty())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result_pattern =
            self.apply_type_substitution(&plan.substitution, plan.result_pattern)?;
        let (drafts, inferred) = self.infer_positional_values(
            argument_syntax,
            PositionalValueContext {
                owner: node,
                result: result_pattern,
                inference_parameters: &plan.inference_parameters,
                destination_types: &destination_types,
                requirements: &[],
                result_context: super::value_planning::CallResultContext::complete(expected),
                failure_rule: BodyRule::InvalidConstruction,
            },
        )?;
        bind_inferred_arguments(&mut plan.substitution, &inferred);
        if !self.nominal_construction_requirements_hold(plan.definition, &plan.substitution)? {
            return Err(self.rule(BodyRule::InvalidConstruction, node)?);
        }
        let values =
            self.materialize_positional_values(drafts, destination_types, &plan.substitution)?;
        let ty = self.apply_type_substitution(&plan.substitution, plan.result_pattern)?;
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
        result_context: Option<CallResultContext>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let member_name = self
            .graph
            .symbols()
            .get(self.token_text(member_token)?)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let callable_id = self
            .construction_surfaces
            .named_function(
                self.graph,
                construction,
                member_name,
                self.source_access_context(),
            )
            .map_err(BodyCheckInternalError::from)?;
        let Some(callable_id) = callable_id else {
            let completion_owner = self.construction_declaration_completion_owner(construction)?;
            self.record_construction_interruption(member_token, completion_owner)?;
            return Err(self.token_rule(BodyRule::InvalidCall, member_token)?);
        };
        let callable = self
            .graph
            .declarations()
            .callables()
            .get(callable_id)
            .cloned()
            .ok_or(BodyCheckInternalError::MissingCallable(callable_id))?;
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
            result_context,
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
                plan.pack,
            )),
        )?;
        result_context
            .and_then(CallResultContext::complete_type)
            .map_or(Ok(call), |expected| {
                self.apply_expected(node, call, expected)
            })
    }

    fn record_construction_interruption_node(
        &mut self,
        node: NodeId,
        owner: ConstructionCompletionOwner,
    ) -> Result<(), BodyCheckInternalError> {
        let origin = SourceOrigin::from_node(self.tree(), node)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
        self.record_construction_interruption_origin(origin, owner);
        Ok(())
    }

    fn construction_declaration_completion_owner(
        &self,
        construction: nocter_model::ConstructionId,
    ) -> Result<Option<ConstructionCompletionOwner>, BodyCheckInternalError> {
        let declaration = self
            .graph
            .declarations()
            .constructions()
            .get(construction)
            .ok_or(crate::ConstructionSurfaceSelectionError::MissingConstruction(construction))?;
        match self.types.get(declaration.target()) {
            Some(TypeKind::Nominal { definition, .. }) => {
                Ok(Some(ConstructionCompletionOwner::Nominal(*definition)))
            }
            Some(TypeKind::Builtin(builtin)) => {
                Ok(Some(ConstructionCompletionOwner::Builtin(*builtin)))
            }
            Some(_) => Ok(None),
            None => Err(BodyCheckInternalError::UnknownType(declaration.target())),
        }
    }

    fn record_construction_interruption(
        &mut self,
        token: SyntaxToken,
        owner: Option<ConstructionCompletionOwner>,
    ) -> Result<(), BodyCheckInternalError> {
        let Some(owner) = owner else {
            return Ok(());
        };
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        self.record_construction_interruption_origin(origin, owner);
        Ok(())
    }

    fn record_construction_interruption_origin(
        &mut self,
        origin: SourceOrigin,
        owner: ConstructionCompletionOwner,
    ) {
        self.interruption = Some(TypedBodyInterruption::new(
            self.source.body(),
            origin,
            TypedBodyInterruptionKind::ConstructionSelection { owner },
        ));
    }

    fn project_construction_member(
        &mut self,
        token: SyntaxToken,
        callable: nocter_model::CallableId,
    ) -> Result<(), BodyCheckInternalError> {
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        self.projections.push(super::NodeProjection::new(
            SemanticEntity::Callable(callable),
            origin,
        ));
        Ok(())
    }

    fn project_variant_member(
        &mut self,
        token: SyntaxToken,
        variant: VariantId,
    ) -> Result<(), BodyCheckInternalError> {
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        self.projections.push(super::NodeProjection::new(
            SemanticEntity::Variant(variant),
            origin,
        ));
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
        let target = self.apply_type_substitution(&construction_substitution, target)?;
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

const fn construction_completion_owner(target: NameTarget) -> Option<ConstructionCompletionOwner> {
    match target {
        NameTarget::Exported(ExportedEntity::NominalType(nominal)) => {
            Some(ConstructionCompletionOwner::Nominal(nominal))
        }
        NameTarget::Exported(ExportedEntity::BuiltinType(builtin)) => {
            Some(ConstructionCompletionOwner::Builtin(builtin))
        }
        NameTarget::Exported(_)
        | NameTarget::Parameter(_)
        | NameTarget::Local(_)
        | NameTarget::Capture(_) => None,
    }
}

fn combined_parameters(
    owner: &[GenericParameterId],
    callable: &[GenericParameterId],
) -> Vec<GenericParameterId> {
    owner.iter().chain(callable).copied().collect::<Vec<_>>()
}
