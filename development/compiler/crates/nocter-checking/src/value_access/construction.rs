//! Structural construction selects fields and component value operations once, in authored order.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Boundary, ExpressionKind, FieldValue, value_type},
};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{AccessExpressionId, ValueAccessMode as Mode};
use nocter_syntax::{NodeId, NodeKind, direct_identifier};
use std::collections::BTreeSet;

impl Checker<'_, '_> {
    pub fn record_value(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let owner = self.direct(node, NodeKind::ReferenceExpression)?;
        let name = self.token(owner)?;
        let Some(data) = self.program().type_named(self.text(name)?) else {
            return self.reject(Rule::UnknownType, name);
        };
        let Some(PayloadSchema::Record(record)) = self.program().data_schema(data) else {
            return self.reject(Rule::InvalidConstruction, name);
        };
        let count = record.fields().len();
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
                .fields()
                .get(selected)
                .ok_or(Failure::InconsistentSyntax)?
                .data();
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
            ExpressionKind::Record {
                fields: fields.into_boxed_slice(),
            },
        )
    }
}
