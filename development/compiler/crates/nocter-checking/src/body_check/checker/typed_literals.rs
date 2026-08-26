use nocter_declarations::LiteralShape;
use nocter_model::{CallableId, TypeId, TypeKind};
use nocter_source_index::{SemanticEntity, SourceOrigin};
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, TokenKind};

use super::BodyChecker;
use super::construction_planning::bind_inferred_arguments;
use super::iterations::CheckedSpreadDraft;
use super::type_uses::NominalOwnerArguments;
use super::value_planning::PositionalValueContext;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::interface_implementation::normalize_requirements;
use crate::syntax::{direct_child, direct_children};
use crate::type_relations::TypeSubstitution;
use crate::{
    AllocationSelection, ArgumentPackSegment, CallableInference, CheckedOperation, CheckedSequence,
    GenericArgument, GenericArguments, StaticDispatch, StaticSelection,
};

struct LiteralPlan {
    definition: nocter_model::NominalTypeId,
    constructor: CallableId,
    construction_target: TypeId,
    construction_parameters: Box<[nocter_model::GenericParameterId]>,
    inference_parameters: Box<[nocter_model::GenericParameterId]>,
    substitution: TypeSubstitution,
    parameter_type: TypeId,
    result_pattern: TypeId,
    requirements: Box<[nocter_model::RequirementId]>,
}

enum SequenceElementDraft {
    Value(usize),
    Spread(CheckedSpreadDraft),
}

impl BodyChecker<'_, '_> {
    pub(super) fn check_typed_sequence_literal(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let mut plan = self.literal_plan(node, LiteralShape::Sequence)?;
        let body = direct_child(self.tree(), node, NodeKind::SequenceBody)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let allocation = self.literal_allocation(node)?;

        let element_pattern = plan
            .substitution
            .apply_type(self.types, plan.parameter_type)
            .map_err(BodyCheckInternalError::CallSubstitution)?;
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
        let mut inference = CallableInference::new(plan.inference_parameters.clone());
        let mut values = Vec::new();
        let mut destinations = Vec::new();
        let mut elements = Vec::new();
        for element in direct_children(self.tree(), body, NodeKind::SequenceElement) {
            if let Some(spread) = direct_child(self.tree(), element, NodeKind::SpreadExpression) {
                let spread = self.check_argument_spread(element, spread)?;
                inference.constrain_exact(element_pattern, spread.contribution);
                elements.push(SequenceElementDraft::Spread(spread));
                continue;
            }
            let syntax = direct_child(self.tree(), element, NodeKind::Expression)
                .ok_or(BodyCheckInternalError::InvalidSyntax(element))?;
            let draft = self.draft_positional_value(
                syntax,
                element_pattern,
                &plan.inference_parameters,
                &requirements,
                &mut inference,
                BodyRule::InvalidConstruction,
            )?;
            let position = values.len();
            values.push(draft);
            destinations.push(element_pattern);
            elements.push(SequenceElementDraft::Value(position));
        }
        let context = PositionalValueContext {
            owner: node,
            result: result_pattern,
            inference_parameters: &plan.inference_parameters,
            destination_types: &destinations,
            requirements: &requirements,
            result_context: super::value_planning::CallResultContext::complete(expected),
            failure_rule: BodyRule::InvalidConstruction,
        };
        let inferred = self.finish_positional_inference(&mut values, &context, inference)?;
        bind_inferred_arguments(&mut plan.substitution, &inferred);
        let values =
            self.materialize_positional_values(values, destinations, &plan.substitution)?;
        let elements = elements
            .into_iter()
            .map(|element| match element {
                SequenceElementDraft::Value(position) => {
                    ArgumentPackSegment::Value(values[position])
                }
                SequenceElementDraft::Spread(spread) => ArgumentPackSegment::Spread {
                    mode: spread.mode,
                    iteration: spread.iteration,
                    exact_size: spread.exact_size,
                },
            })
            .collect::<Vec<_>>();
        let result = plan
            .substitution
            .apply_type(self.types, plan.result_pattern)
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let selection = self.finish_literal_selection(node, &plan, result)?;
        self.project_literal_constructor(sequence_open(self.tree(), body)?, plan.constructor)?;
        let checked = self.add_node(
            node,
            result,
            CheckedOperation::Sequence(CheckedSequence::new(selection, elements, allocation)),
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
            .literal(
                self.graph,
                construction,
                shape,
                self.source_access_context(),
            )
            .map_err(BodyCheckInternalError::from)?
        else {
            return Err(self.rule(BodyRule::InvalidConstruction, node)?);
        };
        let construction_parameters = constructor.construction_parameters().to_vec();
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
            .apply_type(self.types, constructor.construction_target())
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        if !matches!(
            self.types.get(specialized_target),
            Some(TypeKind::Nominal { definition, .. }) if *definition == owner.definition
        ) {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        Ok(LiteralPlan {
            definition: owner.definition,
            constructor: constructor.callable(),
            construction_target: constructor.construction_target(),
            construction_parameters: construction_parameters.into_boxed_slice(),
            inference_parameters: inference_parameters.into_boxed_slice(),
            substitution,
            parameter_type: constructor.parameter_type(),
            result_pattern: constructor.result(),
            requirements: constructor.requirements().into(),
        })
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
                plan.construction_target,
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
        self.projections.push(super::NodeProjection::new(
            SemanticEntity::Callable(constructor),
            origin,
        ));
        Ok(())
    }

    fn literal_allocation(&mut self, node: NodeId) -> Result<AllocationSelection, BodyCheckError> {
        let Some(allocation) = direct_child(self.tree(), node, NodeKind::AllocationOverride) else {
            return Ok(AllocationSelection::CurrentRegion);
        };
        let allocator = direct_child(self.tree(), allocation, NodeKind::AllocatorPlace)
            .ok_or(BodyCheckInternalError::InvalidSyntax(allocation))?;
        Ok(AllocationSelection::Explicit(
            self.check_allocation_place(allocator)?,
        ))
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
