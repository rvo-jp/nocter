//! Structural construction selects fields and component value operations once, in authored order.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Boundary, ExpressionKind, FieldValue, value_type},
};
use nocter_declarations::value_access::{
    AggregateForm, Namespace, PayloadReadiness, PayloadSchema,
};
use nocter_model::{AccessExpressionId, ValueAccessMode as Mode};
use nocter_syntax::{NodeId, NodeKind, direct_identifier};
use std::collections::BTreeSet;
mod arrays;

impl Checker<'_, '_> {
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
            let mut value = self.expression(
                entry,
                constraints
                    .as_ref()
                    .map(|fields| Boundary::Value(fields[index])),
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
            if self.program().payload_readiness(field_data) == PayloadReadiness::Unsupported {
                return self.reject(Rule::InvalidConstruction, entry);
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
        let owner = self.direct(node, NodeKind::ReferenceExpression)?;
        let name = self.token(owner)?;
        self.require_header(Namespace::Type, name)?;
        let Some(data) = self
            .program()
            .type_named_in(self.context.lookup_scope(), self.text(name)?)
        else {
            return self.reject(Rule::UnknownType, name);
        };
        let Some(PayloadSchema::Aggregate(record)) = self.program().data_schema(data) else {
            return self.reject(Rule::InvalidConstruction, name);
        };
        let AggregateForm::Record(_) = record.form() else {
            return self.reject(Rule::InvalidConstruction, name);
        };
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
