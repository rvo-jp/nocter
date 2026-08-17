use nocter_declarations::{
    CallableKind, CallableOwner, LiteralShape, ParameterOwner, ParameterRole,
    StandardDeclarationRole,
};
use nocter_model::{BorrowCapability, BuiltinType, CallableId, TypeId, TypeKind};
use nocter_source_index::{SemanticEntity, SourceOrigin};
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, TokenKind};

use super::BodyChecker;
use super::construction_planning::bind_inferred_arguments;
use super::type_uses::NominalOwnerArguments;
use super::value_planning::PositionalValueContext;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::conformance::normalize_requirements;
use crate::syntax::{direct_child, direct_children};
use crate::type_relations::TypeSubstitution;
use crate::{
    AllocationSelection, CallableInference, CheckedOperation, CheckedSequence, GenericArgument,
    GenericArguments, SequenceElement, StaticDispatch, StaticSelection,
};

struct LiteralPlan {
    definition: nocter_model::NominalTypeId,
    construction: nocter_model::ConstructionId,
    constructor: CallableId,
    construction_parameters: Box<[nocter_model::GenericParameterId]>,
    inference_parameters: Box<[nocter_model::GenericParameterId]>,
    substitution: TypeSubstitution,
    parameter: nocter_model::ParameterId,
    parameter_type: TypeId,
    result_pattern: TypeId,
    requirements: Box<[nocter_model::RequirementId]>,
}

impl BodyChecker<'_, '_> {
    pub(super) fn check_typed_sequence_literal(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let mut plan = self.literal_plan(node, LiteralShape::Sequence)?;
        let parameter = self.literal_parameter(node, &plan)?;
        if parameter.role()
            != (ParameterRole::Ordinary {
                position: 0,
                variadic: true,
            })
        {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let body = direct_child(self.tree(), node, NodeKind::SequenceBody)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let mut element_syntax = Vec::new();
        for element in direct_children(self.tree(), body, NodeKind::SequenceElement) {
            if direct_child(self.tree(), element, NodeKind::SpreadExpression).is_some() {
                return Err(BodyCheckInternalError::UnsupportedSyntax(
                    element,
                    NodeKind::SpreadExpression,
                )
                .into());
            }
            element_syntax.push(
                direct_child(self.tree(), element, NodeKind::Expression)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(element))?,
            );
        }
        let allocation = self.literal_allocation(node)?;

        let element_pattern = plan
            .substitution
            .apply_type(self.types, plan.parameter_type)
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let destination_types = vec![element_pattern; element_syntax.len()];
        let result_pattern = plan
            .substitution
            .apply_type(self.types, plan.result_pattern)
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let requirements = normalize_requirements(
            self.graph,
            self.types,
            &plan.substitution,
            &plan.requirements,
        )
        .map_err(BodyCheckInternalError::CallSubstitution)?;
        let (drafts, inferred) = self.infer_positional_values(
            element_syntax,
            PositionalValueContext {
                owner: node,
                result: result_pattern,
                inference_parameters: &plan.inference_parameters,
                destination_types: &destination_types,
                requirements: &requirements,
                expected,
                failure_rule: BodyRule::InvalidConstruction,
            },
        )?;
        bind_inferred_arguments(&mut plan.substitution, &inferred);
        let values =
            self.materialize_positional_values(drafts, destination_types, &plan.substitution)?;
        let result = plan
            .substitution
            .apply_type(self.types, plan.result_pattern)
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let selection = self.finish_literal_selection(node, &plan, result)?;
        self.project_literal_constructor(sequence_open(self.tree(), body)?, plan.constructor)?;
        let checked = self.add_node(
            node,
            result,
            CheckedOperation::Sequence(CheckedSequence::new(
                selection,
                values
                    .into_iter()
                    .map(SequenceElement::Value)
                    .collect::<Vec<_>>(),
                allocation,
            )),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    pub(super) fn check_typed_string_literal(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let mut plan = self.literal_plan(node, LiteralShape::String)?;
        let parameter = self.literal_parameter(node, &plan)?;
        if parameter.role()
            != (ParameterRole::Ordinary {
                position: 0,
                variadic: false,
            })
            || !self.is_readonly_str(parameter.ty())
        {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let allocation = self.literal_allocation(node)?;
        let literal = direct_child(self.tree(), node, NodeKind::StringLiteral)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let text = nocter_syntax::decode_string_literal(
            self.input
                .sources()
                .get(self.tree().source())
                .ok_or(BodyCheckInternalError::InvalidSyntax(literal))?,
            self.tree(),
            literal,
        )
        .ok_or(BodyCheckInternalError::InvalidSyntax(literal))?;
        let result_pattern = plan
            .substitution
            .apply_type(self.types, plan.result_pattern)
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let mut inference = CallableInference::new(plan.inference_parameters.clone());
        if let Some(expected) = expected {
            inference
                .constrain_result_contextual(self.types, result_pattern, expected)
                .map_err(|error| {
                    self.inference_error(node, error, BodyRule::InvalidConstruction)
                })?;
        }
        let inferred = inference
            .finish(self.types)
            .map_err(|error| self.inference_error(node, error, BodyRule::InvalidConstruction))?;
        bind_inferred_arguments(&mut plan.substitution, &inferred);
        let result = plan
            .substitution
            .apply_type(self.types, plan.result_pattern)
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let selection = self.finish_literal_selection(node, &plan, result)?;
        self.project_literal_constructor(string_open(self.tree(), literal)?, plan.constructor)?;
        let checked = self.add_node(
            node,
            result,
            CheckedOperation::StringLiteral {
                constructor: selection,
                text,
                allocation,
            },
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    fn literal_plan(
        &mut self,
        node: NodeId,
        shape: LiteralShape,
    ) -> Result<LiteralPlan, BodyCheckError> {
        let owner_syntax = direct_child(self.tree(), node, NodeKind::NamedType)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let owner = self.resolve_nominal_construction_type(owner_syntax)?;
        let Some(construction) = self.construction_surfaces.for_nominal(owner.definition) else {
            return Err(self.rule(BodyRule::InvalidConstruction, node)?);
        };
        let Some(constructor) = self
            .construction_surfaces
            .literal(self.graph, construction, shape, self.source.module())
            .map_err(BodyCheckInternalError::from)?
        else {
            return Err(self.rule(BodyRule::InvalidConstruction, node)?);
        };
        let callable = self
            .graph
            .declarations()
            .callables()
            .get(constructor)
            .cloned()
            .ok_or(BodyCheckInternalError::MissingCallable(constructor))?;
        let construction_declaration = self
            .graph
            .declarations()
            .constructions()
            .get(construction)
            .ok_or(crate::ConstructionSurfaceSelectionError::MissingConstruction(construction))
            .map_err(BodyCheckInternalError::from)?;
        let construction_parameters = construction_declaration.generic_parameters().to_vec();
        let (inference_parameters, substitution) = match owner.arguments {
            NominalOwnerArguments::Inferred(_) => {
                (construction_parameters.clone(), TypeSubstitution::default())
            }
            NominalOwnerArguments::Fixed(arguments) => {
                if construction_parameters.len() != arguments.len() {
                    return Err(self.rule(BodyRule::InvalidConstruction, node)?);
                }
                let mut substitution = TypeSubstitution::default();
                for (parameter, argument) in construction_parameters
                    .iter()
                    .copied()
                    .zip(arguments.iter().copied())
                {
                    substitution.bind_generic(parameter, argument);
                }
                (Vec::new(), substitution)
            }
        };
        let specialized_target = substitution
            .apply_type(self.types, construction_declaration.target())
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        if !matches!(
            self.types.get(specialized_target),
            Some(TypeKind::Nominal { definition, .. }) if *definition == owner.definition
        ) {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let [parameter] = callable.parameters() else {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        };
        if callable.kind() != CallableKind::Literal(shape)
            || callable.owner() != CallableOwner::Construction(construction)
            || callable.receiver().is_some()
            || !callable.generic_parameters().is_empty()
        {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let declaration = self
            .graph
            .declarations()
            .parameters()
            .get(*parameter)
            .copied()
            .ok_or(BodyCheckInternalError::MissingParameterType(
                crate::NameTarget::Exported(nocter_declarations::ExportedEntity::Callable(
                    constructor,
                )),
            ))?;
        if declaration.owner() != ParameterOwner::Callable(constructor) {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        Ok(LiteralPlan {
            definition: owner.definition,
            construction,
            constructor,
            construction_parameters: construction_parameters.into_boxed_slice(),
            inference_parameters: inference_parameters.into_boxed_slice(),
            substitution,
            parameter: *parameter,
            parameter_type: declaration.ty(),
            result_pattern: callable.result(),
            requirements: callable.requirements().into(),
        })
    }

    fn literal_parameter(
        &self,
        node: NodeId,
        plan: &LiteralPlan,
    ) -> Result<nocter_declarations::Parameter, BodyCheckError> {
        self.graph
            .declarations()
            .parameters()
            .get(plan.parameter)
            .copied()
            .ok_or_else(|| BodyCheckInternalError::InvalidSyntax(node).into())
    }

    fn finish_literal_selection(
        &mut self,
        node: NodeId,
        plan: &LiteralPlan,
        result: TypeId,
    ) -> Result<StaticSelection, BodyCheckError> {
        let Some(TypeKind::Nominal {
            definition,
            arguments: _,
        }) = self.types.get(result)
        else {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        };
        if *definition != plan.definition {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        if self
            .graph
            .declarations()
            .nominal_types()
            .get(*definition)
            .is_none()
        {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let generic_arguments = GenericArguments::new(
            plan.construction_parameters
                .iter()
                .copied()
                .map(|parameter| {
                    let pattern = self
                        .types
                        .intern(TypeKind::GenericParameter(parameter))
                        .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
                    let ty = plan
                        .substitution
                        .apply_type(self.types, pattern)
                        .map_err(BodyCheckInternalError::CallSubstitution)?;
                    Ok(GenericArgument::new(parameter, ty))
                })
                .collect::<Result<Vec<_>, BodyCheckInternalError>>()?,
        )
        .map_err(BodyCheckInternalError::CallGenericArguments)?;
        if !self.requirements_hold(&plan.requirements, &plan.substitution)?
            || !self.construction_target_requirements_hold(
                self.graph
                    .declarations()
                    .constructions()
                    .get(plan.construction)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(node))?
                    .target(),
                &generic_arguments,
            )?
        {
            return Err(self.rule(BodyRule::InvalidConstruction, node)?);
        }
        Ok(StaticSelection::new(
            StaticDispatch::Direct(plan.constructor),
            generic_arguments,
        ))
    }

    fn project_literal_constructor(
        &mut self,
        token: SyntaxToken,
        constructor: CallableId,
    ) -> Result<(), BodyCheckInternalError> {
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        self.projections.push(super::NodeProjection {
            entity: SemanticEntity::Callable(constructor),
            origin,
        });
        Ok(())
    }

    fn literal_allocation(&mut self, node: NodeId) -> Result<AllocationSelection, BodyCheckError> {
        let Some(allocation) = direct_child(self.tree(), node, NodeKind::AllocationOverride) else {
            return Ok(AllocationSelection::CurrentRegion);
        };
        let allocator = direct_child(self.tree(), allocation, NodeKind::AllocatorPlace)
            .ok_or(BodyCheckInternalError::InvalidSyntax(allocation))?;
        let named = direct_child(self.tree(), allocator, NodeKind::NamedPlace)
            .ok_or(BodyCheckInternalError::InvalidSyntax(allocator))?;
        let place = self.named_place(named)?;
        let candidate = match self.types.get(place.ty) {
            Some(TypeKind::Borrow { referent, .. }) => *referent,
            Some(_) => place.ty,
            None => return Err(BodyCheckInternalError::UnknownType(place.ty).into()),
        };
        let Some(TypeKind::Nominal {
            definition,
            arguments,
        }) = self.types.get(candidate)
        else {
            return Err(self.rule(BodyRule::InvalidAllocationContext, named)?);
        };
        let allocation_roles = [
            self.standard_semantics
                .nominal(StandardDeclarationRole::AbortingAllocator),
            self.standard_semantics
                .nominal(StandardDeclarationRole::AllocationContext),
        ];
        if allocation_roles.iter().all(Option::is_none) {
            return Err(BodyCheckInternalError::MissingAllocationSemanticRoles.into());
        }
        let allowed = arguments.is_empty()
            && allocation_roles
                .into_iter()
                .flatten()
                .any(|allowed| allowed == *definition);
        if !allowed {
            return Err(self.rule(BodyRule::InvalidAllocationContext, named)?);
        }
        let checked = self.add_node(named, place.ty, CheckedOperation::Place(place.id))?;
        Ok(AllocationSelection::Explicit(checked))
    }

    fn is_readonly_str(&self, ty: TypeId) -> bool {
        matches!(
            self.types.get(ty),
            Some(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            }) if *referent == self.types.builtin(BuiltinType::Str)
        )
    }
}

fn sequence_open(
    tree: &nocter_syntax::SyntaxTree,
    node: NodeId,
) -> Result<SyntaxToken, BodyCheckInternalError> {
    direct_matching_token(tree, node, |kind| {
        kind == TokenKind::Punctuation(Punctuation::LeftBracket)
    })
}

fn string_open(
    tree: &nocter_syntax::SyntaxTree,
    node: NodeId,
) -> Result<SyntaxToken, BodyCheckInternalError> {
    direct_matching_token(tree, node, |kind| matches!(kind, TokenKind::StringStart(_)))
}

fn direct_matching_token(
    tree: &nocter_syntax::SyntaxTree,
    node: NodeId,
    matches: impl Fn(TokenKind) -> bool,
) -> Result<SyntaxToken, BodyCheckInternalError> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) if matches(token.kind()) => Some(*token),
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
        .ok_or(BodyCheckInternalError::InvalidSyntax(node))
}
