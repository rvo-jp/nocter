//! Structural construction selects fields and component value operations once, in authored order.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Boundary, ExpressionKind, FieldValue, value_type},
};
use nocter_declarations::value_access::{AggregateForm, PayloadSchema};
use nocter_model::{AccessExpressionId, ValueAccessMode as Mode};
use nocter_syntax::{NodeId, NodeKind, direct_identifier};
use std::collections::BTreeSet;
mod arrays;

impl Checker<'_, '_> {
    /// A component contract constrains a supplied witness rather than replacing its identity.
    pub(super) fn construction_value(
        &mut self,
        node: NodeId,
        expected: Option<nocter_model::TypeId>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let value = self.expression_with_context(node, expected, None, reachable)?;
        let data = match expected {
            Some(data) => match self.program().data_schema(data) {
                Some(PayloadSchema::Function(witness))
                    if matches!(
                        witness.origin,
                        nocter_model::ValueFunctionOrigin::Result(_)
                            | nocter_model::ValueFunctionOrigin::Parameter(
                                nocter_model::ValueParameter::Component { .. }
                            )
                    ) =>
                {
                    let contract = self
                        .program()
                        .function_contract(witness)
                        .ok_or(Failure::InconsistentSyntax)?;
                    if self.ty(value).access().is_some() {
                        self.function_constraint_data(node, value, &contract, false)?
                    } else {
                        data
                    }
                }
                _ => self
                    .ty(value)
                    .access()
                    .filter(|actual| self.program().matches_compound_input(actual.data(), data))
                    .map_or(data, nocter_model::ValueAccess::data),
            },
            None => return Ok(value),
        };
        self.boundary(node, value, Boundary::Value(data))
    }
    pub fn tuple_value(
        &mut self,
        node: NodeId,
        expected: Option<Boundary>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let expected = expected.map(|boundary| boundary.ty().data());
        let constraints = if let Some(data) = expected {
            match self.program().data_schema(data) {
                Some(PayloadSchema::Aggregate(schema)) => match schema.form() {
                    AggregateForm::Tuple(fields) => Some(fields.to_vec()),
                    AggregateForm::Record(_) => return self.reject(Rule::TypeMismatch, node),
                },
                _ => return self.reject(Rule::TypeMismatch, node),
            }
        } else {
            None
        };
        let entries = self.children(node);
        if constraints
            .as_ref()
            .is_some_and(|fields| fields.len() != entries.len())
        {
            return self.reject(Rule::TypeMismatch, node);
        }
        let mut fields = Vec::new();
        let mut data = Vec::new();
        let mut continues = true;
        for (index, entry) in entries.into_iter().enumerate() {
            let mut value = self.construction_value(
                entry,
                constraints.as_ref().map(|fields| fields[index]),
                reachable && continues,
            )?;
            // Expected data fixes this component's shape even if it exits without a value.
            // Inference alone cannot invent a type for an unknown result.
            let Some(field_data) = self
                .ty(value)
                .access()
                .map(nocter_model::ValueAccess::data)
                .or_else(|| constraints.as_ref().map(|fields| fields[index]))
            else {
                return self.reject(Rule::UnknownResultType, entry);
            };
            if self.program().payload_dependencies(field_data).is_none() {
                return self.reject(Rule::UnsupportedData, entry);
            }
            if constraints.is_none() {
                value = self.boundary(entry, value, Boundary::Value(field_data))?;
            }
            data.push(field_data);
            continues &= self.ty(value).may_complete();
            fields.push(FieldValue {
                field: nocter_model::ValueField::Tuple(index),
                value,
            });
        }
        let data = self
            .types
            .tuple(&data)
            .map_err(|_| Failure::InconsistentSyntax)?;
        self.emit(
            node,
            value_type(data, Mode::Own).with_continuation(continues),
            ExpressionKind::Aggregate {
                fields: fields.into(),
            },
        )
    }
    pub fn record_value(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let owner = self.first_child(node)?;
        let path = self.name_path(owner)?.ok_or(Failure::InconsistentSyntax)?;
        let path = path.into_iter().map(|(_, token)| token).collect::<Vec<_>>();
        let name = *path.last().ok_or(Failure::InconsistentSyntax)?;
        let data = self.annotation_data_path(&path, false)?;
        let Some(PayloadSchema::Aggregate(record)) = self.program().data_schema(data) else {
            return self.reject(Rule::InvalidConstruction, name);
        };
        let AggregateForm::Record(_) = record.form() else {
            return self.reject(Rule::InvalidConstruction, name);
        };
        let nominal = self
            .program()
            .record_identity(data)
            .ok_or(Failure::InconsistentSyntax)?;
        if !self.context.complete_record_visible(nominal) {
            return self.reject(Rule::InaccessibleRepresentation, name);
        }
        let count = record.len();
        let initializer = self.direct(node, NodeKind::StructInitializer)?;
        let mut seen = BTreeSet::new();
        let mut fields = Vec::new();
        let mut continues = true;
        for field in self.children(initializer) {
            let name = direct_identifier(self.syntax().tree(), field)
                .ok_or(Failure::InconsistentSyntax)?;
            let selected = self.field(data, name)?;
            if !seen.insert(selected) {
                return self.reject(Rule::InvalidConstruction, name);
            }
            let data = self
                .program()
                .component(data, selected)
                .ok_or(Failure::InconsistentSyntax)?
                .data;
            let expression = self.direct(field, NodeKind::Expression)?;
            let value = self.expression(
                expression,
                Some(Boundary::Value(data)),
                reachable && continues,
            )?;
            continues &= self.ty(value).may_complete();
            fields.push(FieldValue {
                field: selected,
                value,
            });
        }
        if seen.len() != count {
            return self.reject(Rule::InvalidConstruction, node);
        }
        self.emit(
            node,
            value_type(data, Mode::Own).with_continuation(continues),
            ExpressionKind::Aggregate {
                fields: fields.into_boxed_slice(),
            },
        )
    }
}
