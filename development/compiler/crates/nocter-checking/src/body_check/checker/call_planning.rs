use nocter_declarations::{CallableDeclaration, ExportedEntity, ParameterRole};
use nocter_model::{
    ArgumentPack, ArgumentPackType, BodyNodeId, GenericParameterId, TypeId, TypeKind,
};
use nocter_syntax::{NodeId, NodeKind};

use super::BodyChecker;
use super::iterations::CheckedSpreadDraft;
use super::value_planning::{CallResultContext, PositionalValueContext, ValueDraft};
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::interface_implementation::normalize_requirements;
use crate::syntax::child_nodes;
use crate::type_relations::TypeSubstitution;
use crate::{
    ArgumentPackSegment, CallableInference, CheckedArgumentPack, CheckedPredicate,
    CheckedRequirement, GenericArgument, GenericArguments, NameTarget,
};

pub(super) struct DeclaredCallPlan {
    pub(super) arguments: Vec<BodyNodeId>,
    pub(super) pack: Option<CheckedArgumentPack>,
    pub(super) generic_arguments: GenericArguments,
    pub(super) result: TypeId,
}

struct DeclaredParameterShape {
    fixed: Vec<TypeId>,
    pack: Option<ArgumentPackType>,
}

enum PackSegmentDraft {
    Value(usize),
    KeyedValue { key: usize, value: usize },
    Spread(CheckedSpreadDraft),
}

enum ArgumentPackDraft {
    Prepared(Vec<PackSegmentDraft>),
    Forwarded(nocter_model::ParameterId),
}

#[derive(Clone, Copy)]
struct CallValuePatterns<'a> {
    fixed: &'a [TypeId],
    pack: Option<ArgumentPackType>,
    inference_parameters: &'a [GenericParameterId],
    requirements: &'a [CheckedRequirement],
}

struct DraftedCallValues {
    values: Vec<ValueDraft>,
    destinations: Vec<TypeId>,
    pack: Option<ArgumentPackDraft>,
    inference: CallableInference,
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
        result_context: Option<CallResultContext>,
    ) -> Result<DeclaredCallPlan, BodyCheckError> {
        let argument_syntax = child_nodes(self.tree(), suffix);
        let mut substitution = generics.owner_substitution.cloned().unwrap_or_default();
        for argument in generics.fixed_arguments {
            substitution.bind_generic(argument.parameter(), argument.ty());
        }
        let parameters = self.declared_parameter_shape(callable_id, callable)?;
        if argument_syntax.len() < parameters.fixed.len()
            || parameters.pack.is_none() && argument_syntax.len() != parameters.fixed.len()
            || argument_syntax
                .iter()
                .take(parameters.fixed.len())
                .any(|argument| {
                    matches!(
                        self.kind(*argument).ok(),
                        Some(NodeKind::SpreadExpression | NodeKind::KeyedArgument)
                    )
                })
        {
            return Err(self.rule(BodyRule::InvalidCall, suffix)?);
        }
        let fixed_patterns = parameters
            .fixed
            .iter()
            .copied()
            .map(|parameter| self.apply_type_substitution(&substitution, parameter))
            .collect::<Result<Vec<_>, _>>()?;
        let pack_pattern = parameters
            .pack
            .map(|pack| pack.try_map(|ty| self.apply_type_substitution(&substitution, ty)))
            .transpose()?;
        let result = self.apply_type_substitution(&substitution, callable.result())?;
        let requirements = normalize_requirements(
            self.graph,
            self.types,
            &substitution,
            callable.requirements(),
        )
        .map_err(BodyCheckInternalError::CallSubstitution)?;
        let DraftedCallValues {
            mut values,
            destinations,
            pack,
            inference,
        } = self.draft_declared_call_values(
            &argument_syntax,
            CallValuePatterns {
                fixed: &fixed_patterns,
                pack: pack_pattern,
                inference_parameters: generics.inference_parameters,
                requirements: &requirements,
            },
        )?;
        let context = PositionalValueContext {
            owner: node,
            result,
            inference_parameters: generics.inference_parameters,
            destination_types: &destinations,
            requirements: &requirements,
            result_context,
            failure_rule: BodyRule::InvalidCall,
        };
        let inferred_arguments =
            self.finish_positional_inference(&mut values, &context, inference)?;
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
        let values = self.materialize_positional_values(values, destinations, &substitution)?;
        let arguments = values[..fixed_patterns.len()].to_vec();
        if parameters.pack.is_some() != pack.is_some() {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let pack_shape = parameters
            .pack
            .map(|shape| shape.try_map(|ty| self.apply_type_substitution(&substitution, ty)))
            .transpose()?;
        let pack = pack
            .zip(pack_shape)
            .map(|(pack, shape)| materialize_argument_pack(node, pack, shape, &values))
            .transpose()?;
        let result = self.apply_type_substitution(&substitution, result)?;
        Ok(DeclaredCallPlan {
            arguments,
            pack,
            generic_arguments,
            result,
        })
    }

    fn draft_declared_call_values(
        &mut self,
        arguments: &[NodeId],
        patterns: CallValuePatterns<'_>,
    ) -> Result<DraftedCallValues, BodyCheckError> {
        let mut inference = CallableInference::new(patterns.inference_parameters);
        let mut values = Vec::new();
        let mut destinations = Vec::new();
        for (syntax, destination) in arguments
            .iter()
            .take(patterns.fixed.len())
            .copied()
            .zip(patterns.fixed.iter().copied())
        {
            values.push(self.draft_positional_value(
                syntax,
                destination,
                patterns.inference_parameters,
                patterns.requirements,
                &mut inference,
                BodyRule::InvalidCall,
            )?);
            destinations.push(destination);
        }
        let pack = if let Some(shape) = patterns.pack {
            let mut segments = Vec::new();
            let pack_syntax = &arguments[patterns.fixed.len()..];
            for (position, syntax) in pack_syntax.iter().copied().enumerate() {
                if self.kind(syntax)? == NodeKind::SpreadExpression {
                    let source = self.required_child(syntax, NodeKind::Expression)?;
                    if let Some((parameter, contribution)) = self.argument_pack_parameter(source)? {
                        if position != 0 || pack_syntax.len() != 1 {
                            return Err(self.rule(BodyRule::InvalidArgumentPackUse, syntax)?);
                        }
                        match (shape, contribution) {
                            (ArgumentPack::Values(expected), ArgumentPack::Values(actual)) => {
                                inference.constrain_exact(expected, actual)
                            }
                            (
                                ArgumentPack::Keyed {
                                    key: expected_key,
                                    value: expected_value,
                                },
                                ArgumentPack::Keyed {
                                    key: actual_key,
                                    value: actual_value,
                                },
                            ) => {
                                inference.constrain_exact(expected_key, actual_key);
                                inference.constrain_exact(expected_value, actual_value);
                            }
                            _ => return Err(self.rule(BodyRule::InvalidCall, syntax)?),
                        }
                        self.register_argument_pack_forwarding(parameter, syntax)?;
                        return Ok(DraftedCallValues {
                            values,
                            destinations,
                            pack: Some(ArgumentPackDraft::Forwarded(parameter)),
                            inference,
                        });
                    }
                    let ArgumentPack::Values(element) = shape else {
                        return Err(self.rule(BodyRule::InvalidCall, syntax)?);
                    };
                    let spread = self.check_argument_spread(syntax, syntax)?;
                    inference.constrain_exact(element, spread.contribution);
                    segments.push(PackSegmentDraft::Spread(spread));
                    continue;
                }
                match shape {
                    ArgumentPack::Values(element) => {
                        if self.kind(syntax)? == NodeKind::KeyedArgument {
                            return Err(self.rule(BodyRule::InvalidCall, syntax)?);
                        }
                        let position = values.len();
                        values.push(self.draft_positional_value(
                            syntax,
                            element,
                            patterns.inference_parameters,
                            patterns.requirements,
                            &mut inference,
                            BodyRule::InvalidCall,
                        )?);
                        destinations.push(element);
                        segments.push(PackSegmentDraft::Value(position));
                    }
                    ArgumentPack::Keyed { key, value } => {
                        if self.kind(syntax)? != NodeKind::KeyedArgument {
                            return Err(self.rule(BodyRule::InvalidCall, syntax)?);
                        }
                        let parts = child_nodes(self.tree(), syntax);
                        let [key_syntax, value_syntax] = parts.as_slice() else {
                            return Err(BodyCheckInternalError::InvalidSyntax(syntax).into());
                        };
                        let key_position = values.len();
                        values.push(self.draft_positional_value(
                            *key_syntax,
                            key,
                            patterns.inference_parameters,
                            patterns.requirements,
                            &mut inference,
                            BodyRule::InvalidCall,
                        )?);
                        destinations.push(key);
                        let value_position = values.len();
                        values.push(self.draft_positional_value(
                            *value_syntax,
                            value,
                            patterns.inference_parameters,
                            patterns.requirements,
                            &mut inference,
                            BodyRule::InvalidCall,
                        )?);
                        destinations.push(value);
                        segments.push(PackSegmentDraft::KeyedValue {
                            key: key_position,
                            value: value_position,
                        });
                    }
                }
            }
            Some(ArgumentPackDraft::Prepared(segments))
        } else {
            None
        };
        Ok(DraftedCallValues {
            values,
            destinations,
            pack,
            inference,
        })
    }

    fn declared_parameter_shape(
        &self,
        callable_id: nocter_model::CallableId,
        callable: &CallableDeclaration,
    ) -> Result<DeclaredParameterShape, BodyCheckError> {
        let mut fixed = Vec::new();
        let mut pack = None;
        for parameter in callable.parameters().iter().copied() {
            let parameter = self
                .graph
                .declarations()
                .parameters()
                .get(parameter)
                .copied()
                .ok_or(BodyCheckInternalError::MissingParameterType(
                    NameTarget::Exported(ExportedEntity::Callable(callable_id)),
                ))?;
            match parameter.role() {
                ParameterRole::Ordinary { .. } if pack.is_none() => {
                    fixed.push(parameter.ty());
                }
                ParameterRole::ArgumentPack { .. } if pack.is_none() => {
                    pack = parameter.argument_pack();
                }
                ParameterRole::Ordinary { .. }
                | ParameterRole::ArgumentPack { .. }
                | ParameterRole::Receiver(_) => {
                    return Err(BodyCheckInternalError::InvalidSyntax(self.source.block()).into());
                }
            }
        }
        Ok(DeclaredParameterShape { fixed, pack })
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
            let CheckedPredicate::Callable { subject, contract } = requirement.predicate() else {
                ordinary.push(requirement);
                continue;
            };
            let Some(TypeKind::Closure {
                definition: closure,
                ..
            }) = self.types.get(*subject)
            else {
                ordinary.push(requirement);
                continue;
            };
            let closure = *closure;
            let signature = self
                .closures
                .signature(closure)
                .ok_or(BodyCheckInternalError::MissingClosure(closure))?
                .clone();
            if !super::closures::concrete_closure_satisfies(contract, &signature) {
                return Ok(false);
            }
            self.closures
                .require_callable(self.source.body(), closure, contract.clone())
                .map_err(BodyCheckInternalError::from)?;
        }
        let mut selector = self.instance_selector();
        selector
            .requirements_hold(&ordinary, &TypeSubstitution::default())
            .map_err(BodyCheckInternalError::from)
            .map_err(Into::into)
    }
}

fn materialize_argument_pack(
    owner: NodeId,
    pack: ArgumentPackDraft,
    shape: ArgumentPackType,
    values: &[BodyNodeId],
) -> Result<CheckedArgumentPack, BodyCheckError> {
    let segments = match pack {
        ArgumentPackDraft::Forwarded(parameter) => {
            return Ok(CheckedArgumentPack::forwarded(parameter, shape));
        }
        ArgumentPackDraft::Prepared(segments) => segments,
    };
    let segments = segments
        .into_iter()
        .map(|segment| match segment {
            PackSegmentDraft::Value(position) => values
                .get(position)
                .copied()
                .map(ArgumentPackSegment::Value)
                .ok_or(BodyCheckInternalError::InvalidSyntax(owner).into()),
            PackSegmentDraft::KeyedValue { key, value } => Ok(ArgumentPackSegment::KeyedValue {
                key: *values
                    .get(key)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(owner))?,
                value: *values
                    .get(value)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(owner))?,
            }),
            PackSegmentDraft::Spread(spread) => Ok(ArgumentPackSegment::Spread {
                mode: spread.mode,
                iteration: spread.iteration,
                exact_size: spread.exact_size,
            }),
        })
        .collect::<Result<Vec<_>, BodyCheckError>>()?;
    Ok(CheckedArgumentPack::new(shape, segments))
}
