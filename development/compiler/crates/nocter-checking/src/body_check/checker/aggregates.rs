use std::collections::HashSet;

use nocter_model::{BodyNodeId, TypeId, TypeKind};
use nocter_source_index::{SemanticEntity, SourceOrigin};
use nocter_syntax::{NodeId, NodeKind};

use super::construction_planning::bind_inferred_arguments;
use super::value_planning::PositionalValueContext;
use super::{BodyChecker, NodeProjection};
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::field_selection::{FieldSelectionError, select_structural_field};
use crate::syntax::{direct_child, direct_children, direct_identifier, direct_nodes};
use crate::{AggregateConstruction, CheckedOperation, TypePosition, validate_type};

struct StructFieldDraft {
    field: nocter_model::FieldId,
    ty: TypeId,
    expression: NodeId,
}

impl BodyChecker<'_, '_> {
    pub(super) fn check_struct_literal(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let owner_syntax = direct_child(self.tree(), node, NodeKind::NamedType)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let owner = self.resolve_nominal_construction_type(owner_syntax)?;
        let mut plan = self.nominal_construction_plan(node, owner)?;
        let selected = self.struct_field_drafts(node, plan.definition)?;

        let destination_types = selected
            .iter()
            .map(|field| {
                plan.substitution
                    .apply_type(self.types, field.ty)
                    .map_err(BodyCheckInternalError::CallSubstitution)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let value_syntax = selected.iter().map(|field| field.expression).collect();
        let result_pattern = plan
            .substitution
            .apply_type(self.types, plan.result_pattern)
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let (drafts, inferred) = self.infer_positional_values(
            value_syntax,
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
        let fields = selected
            .into_iter()
            .map(|field| field.field)
            .zip(values)
            .collect::<Vec<_>>();
        let aggregate = self.add_node(
            node,
            ty,
            CheckedOperation::Aggregate(AggregateConstruction::Struct {
                definition: plan.definition,
                fields: fields.into_boxed_slice(),
            }),
        )?;
        expected.map_or(Ok(aggregate), |expected| {
            self.apply_expected(node, aggregate, expected)
        })
    }

    fn struct_field_drafts(
        &mut self,
        node: NodeId,
        definition: nocter_model::NominalTypeId,
    ) -> Result<Vec<StructFieldDraft>, BodyCheckError> {
        let structural_fields = self
            .construction_surfaces
            .structural_fields(self.graph, definition, self.source.module())
            .map_err(BodyCheckInternalError::from)?;
        let Some(declared_fields) = structural_fields.map(<[nocter_model::FieldId]>::to_vec) else {
            return Err(self.rule(BodyRule::InvalidConstruction, node)?);
        };
        let initializer = direct_child(self.tree(), node, NodeKind::StructInitializer)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let initializers = direct_children(self.tree(), initializer, NodeKind::FieldInitializer);
        if initializers.len() != declared_fields.len() {
            return Err(self.rule(BodyRule::InvalidConstruction, initializer)?);
        }

        let mut selected = Vec::with_capacity(initializers.len());
        let mut seen = HashSet::new();
        for initializer in initializers {
            let token = direct_identifier(self.tree(), initializer)
                .ok_or(BodyCheckInternalError::InvalidSyntax(initializer))?;
            let name = self.segment_symbol(token)?;
            let field = self
                .construction_surfaces
                .structural_field(definition, name)
                .map_err(BodyCheckInternalError::from)?;
            let Some(field) = field else {
                return Err(self.token_rule(BodyRule::UnknownField, token)?);
            };
            if !seen.insert(field) {
                return Err(self.token_rule(BodyRule::InvalidConstruction, token)?);
            }
            let field = match select_structural_field(
                self.graph,
                self.source.module(),
                definition,
                field,
            ) {
                Ok(field) => field,
                Err(FieldSelectionError::InaccessibleField(_)) => {
                    return Err(self.token_rule(BodyRule::InaccessibleField, token)?);
                }
                Err(_) => return Err(BodyCheckInternalError::FieldSelection.into()),
            };
            let expression = direct_child(self.tree(), initializer, NodeKind::Expression)
                .ok_or(BodyCheckInternalError::InvalidSyntax(initializer))?;
            self.project_field_token(token, field.field())?;
            selected.push(StructFieldDraft {
                field: field.field(),
                ty: field.ty(),
                expression,
            });
        }
        if seen.len() != declared_fields.len()
            || declared_fields.iter().any(|field| !seen.contains(field))
        {
            return Err(self.rule(BodyRule::InvalidConstruction, initializer)?);
        }

        Ok(selected)
    }

    pub(super) fn check_array_literal(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let elements = direct_nodes(self.tree(), node)
            .into_iter()
            .filter(|child| {
                self.kind(*child)
                    .is_ok_and(|kind| kind == NodeKind::Expression)
            })
            .collect::<Vec<_>>();
        let length = u64::try_from(elements.len())
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
        let contextual_element = match expected {
            Some(expected) => self.expected_array_element(node, expected, length)?,
            None => None,
        };
        let (element, values) = if let Some(element) = contextual_element {
            let values = elements
                .into_iter()
                .map(|element_syntax| self.check_expression(element_syntax, Some(element)))
                .collect::<Result<Vec<_>, _>>()?;
            (element, values)
        } else {
            let Some((first, remaining)) = elements.split_first() else {
                return Err(self.rule(BodyRule::InvalidConstruction, node)?);
            };
            let first_value = self.check_expression(*first, None)?;
            let element = self.node_type(first_value)?;
            if validate_type(self.types, element, TypePosition::Data).is_err() {
                return Err(self.rule(BodyRule::InvalidConstruction, node)?);
            }
            let mut values = Vec::with_capacity(elements.len());
            values.push(first_value);
            for element_syntax in remaining {
                values.push(self.check_expression(*element_syntax, Some(element))?);
            }
            (element, values)
        };
        let ty = self
            .types
            .intern(TypeKind::FixedArray { element, length })
            .map_err(|_| BodyCheckInternalError::UnknownType(element))?;
        let aggregate = self.add_node(
            node,
            ty,
            CheckedOperation::Aggregate(AggregateConstruction::FixedArray(
                values.into_boxed_slice(),
            )),
        )?;
        expected.map_or(Ok(aggregate), |expected| {
            self.apply_expected(node, aggregate, expected)
        })
    }

    fn expected_array_element(
        &self,
        node: NodeId,
        mut expected: TypeId,
        length: u64,
    ) -> Result<Option<TypeId>, BodyCheckError> {
        loop {
            match self.types.get(expected) {
                Some(TypeKind::FixedArray {
                    element,
                    length: expected_length,
                }) => {
                    if *expected_length != length {
                        return Err(self.rule(BodyRule::InvalidConstruction, node)?);
                    }
                    return Ok(Some(*element));
                }
                Some(TypeKind::Optional(payload) | TypeKind::Fallible(payload)) => {
                    expected = *payload;
                }
                Some(_) => return Ok(None),
                None => return Err(BodyCheckInternalError::UnknownType(expected).into()),
            }
        }
    }

    fn project_field_token(
        &mut self,
        token: nocter_syntax::SyntaxToken,
        field: nocter_model::FieldId,
    ) -> Result<(), BodyCheckInternalError> {
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        self.projections.push(NodeProjection {
            entity: SemanticEntity::Field(field),
            origin,
        });
        Ok(())
    }
}
