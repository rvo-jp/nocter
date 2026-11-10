//! Ordinary component typing before the separate array body/resource/storage admission gate.
use crate::value_access::{
    Failure, Rule,
    checker::Checker,
    ir::{Boundary, ExpressionKind, FieldValue, value_type},
};
use nocter_declarations::value_access::{ArraySchema, PayloadSchema};
use nocter_model::{AccessExpressionId, TypeId, ValueAccessMode as Mode, ValueField};
use nocter_syntax::NodeId;

impl Checker<'_, '_> {
    fn array_element(&self, node: NodeId, data: TypeId) -> Result<(), Failure> {
        match self
            .program()
            .data_schema(data)
            .ok_or(Failure::InconsistentSyntax)?
        {
            PayloadSchema::Function(_) => self.reject(Rule::UnsupportedData, node),
            PayloadSchema::Builtin(_) | PayloadSchema::Aggregate(_) | PayloadSchema::Array(_) => {
                Ok(())
            }
        }
    }
    fn array_context(
        &self,
        node: NodeId,
        hint: Option<TypeId>,
    ) -> Result<Option<(TypeId, ArraySchema)>, Failure> {
        hint.map(|data| match self.program().data_schema(data) {
            Some(PayloadSchema::Array(array)) => Ok((data, array)),
            _ => self.reject(Rule::TypeMismatch, node),
        })
        .transpose()
    }
    pub(in crate::value_access) fn array_value(
        &mut self,
        node: NodeId,
        hint: Option<TypeId>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let context = self.array_context(node, hint)?;
        let entries = self.children(node);
        let length = u64::try_from(entries.len()).map_err(|_| Failure::InconsistentSyntax)?;
        if context.is_some_and(|(_, array)| array.length() != length) {
            return self.reject(Rule::TypeMismatch, node);
        }
        let expected = context.map(|(_, array)| array.element());
        let mut element = expected;
        let mut fields = Vec::new();
        let mut continues = true;
        for (index, entry) in entries.into_iter().enumerate() {
            // Inference has no first-element coercion: only an authored context constrains entries.
            let mut value =
                self.expression(entry, expected.map(Boundary::Value), reachable && continues)?;
            let Some(data) = self
                .ty(value)
                .access()
                .map(nocter_model::ValueAccess::data)
                .or(expected)
            else {
                return self.reject(Rule::UnknownResultType, entry);
            };
            if element.is_some_and(|element| element != data) {
                return self.reject(Rule::TypeMismatch, entry);
            }
            element = Some(data);
            self.array_element(entry, data)?;
            if expected.is_none() {
                value = self.boundary(entry, value, Boundary::Value(data))?;
            }
            continues &= self.ty(value).may_complete();
            fields.push(FieldValue {
                field: ValueField::Array(
                    u64::try_from(index).map_err(|_| Failure::InconsistentSyntax)?,
                ),
                value,
            });
        }
        let Some(element) = element else {
            return self.reject_message(
                Rule::UnknownResultType,
                node,
                "an empty array needs an element type from its context".into(),
                Vec::new(),
            );
        };
        let data = match context {
            Some((data, _)) => data,
            None => self
                .types
                .array(element, length)
                .map_err(|_| Failure::InconsistentSyntax)?,
        };
        self.emit(
            node,
            value_type(data, Mode::Own).with_continuation(continues),
            ExpressionKind::Aggregate {
                fields: fields.into(),
            },
        )
    }
    pub(in crate::value_access) fn array_repetition(
        &mut self,
        node: NodeId,
        hint: Option<TypeId>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let (seed, count) = nocter_syntax::array_repetition_parts(self.syntax().tree(), node)
            .ok_or(Failure::InconsistentSyntax)?;
        let length = self.required_length(count)?;
        let context = self.array_context(node, hint)?;
        if context.is_some_and(|(_, array)| array.length() != length) {
            return self.reject(Rule::TypeMismatch, node);
        }
        let expected = context.map(|(_, array)| array.element());
        let value = self.expression(seed, expected.map(Boundary::Value), reachable)?;
        let Some(element) = self
            .ty(value)
            .access()
            .map(nocter_model::ValueAccess::data)
            .or(expected)
        else {
            return self.reject(Rule::UnknownResultType, seed);
        };
        self.array_element(seed, element)?;
        if expected.is_none() {
            self.boundary(seed, value, Boundary::Value(element))?;
        }
        if length > 1 && !self.copyable(element) {
            return self.reject_message(
                Rule::InvalidConstruction,
                seed,
                "array repetition greater than one requires Copy element data".into(),
                Vec::new(),
            );
        }
        // Count and ordinary seed typing do not qualify seed cleanup, compact repeated storage or
        // execution. In particular zero count never suppresses seed preparation.
        self.reject(Rule::UnsupportedData, node)
    }
}
