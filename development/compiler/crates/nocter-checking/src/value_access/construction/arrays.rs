//! Listed and repeated construction through ordinary value supply and one array type authority.
use crate::value_access::{
    Failure, Rule,
    checker::Checker,
    ir::{Boundary, ExpressionKind, FieldValue, value_type},
};
use nocter_declarations::value_access::{ArraySchema, PayloadReadiness, PayloadSchema};
use nocter_model::{AccessExpressionId, TypeId, ValueAccessMode as Mode, ValueField};
use nocter_syntax::NodeId;

enum ArrayEntry {
    Value(AccessExpressionId),
    Absent { node: NodeId, reachable: bool },
}

impl Checker<'_, '_> {
    fn array_element(&self, node: NodeId, data: TypeId) -> Result<(), Failure> {
        if self.program().payload_dependencies(data).is_none() {
            self.reject(Rule::UnsupportedData, node)
        } else {
            Ok(())
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
        let mut element = None;
        let mut supplied = Vec::new();
        let mut continues = true;
        for entry in entries {
            // Absence-only construction contributes no callable witness. Check it once the
            // other entries establish the element; aggregate fields still retain source order.
            if expected.is_some_and(|data| self.absence_type(data).is_some())
                && self.absence_construction(entry)?
            {
                supplied.push(ArrayEntry::Absent {
                    node: entry,
                    reachable: reachable && continues,
                });
                continue;
            }
            // Inference has no first-element coercion: only an authored context constrains entries.
            let mut value = self.construction_value(entry, expected, reachable && continues)?;
            let Some(data) = self
                .ty(value)
                .access()
                .map(nocter_model::ValueAccess::data)
                .or(expected)
            else {
                return self.reject(Rule::UnknownResultType, entry);
            };
            if element.is_some_and(|element| {
                element != data
                    && !self.defer_equal(
                        nocter_model::ValueAccess::new(element, Mode::Own),
                        nocter_model::ValueAccess::new(data, Mode::Own),
                    )
            }) {
                return self.reject(Rule::TypeMismatch, entry);
            }
            element = Some(data);
            self.array_element(entry, data)?;
            if expected.is_none() {
                value = self.boundary(entry, value, Boundary::Value(data))?;
            }
            continues &= self.ty(value).may_complete();
            supplied.push(ArrayEntry::Value(value));
        }
        let Some(element) = element.or(expected) else {
            return self.reject_message(
                Rule::UnknownResultType,
                node,
                "an empty array needs an element type from its context".into(),
                Vec::new(),
            );
        };
        let fields = self.array_fields(supplied, element)?;
        let data = match context {
            Some((data, array)) if array.element() == element => data,
            _ => self
                .types
                .array(element, length)
                .map_err(|_| Failure::InconsistentSyntax)?,
        };
        self.emit(
            node,
            value_type(data, Mode::Own).with_continuation(continues),
            ExpressionKind::Aggregate {
                enum_case: None,
                fields: fields.into(),
            },
        )
    }
    fn array_fields(
        &mut self,
        supplied: Vec<ArrayEntry>,
        element: TypeId,
    ) -> Result<Vec<FieldValue>, Failure> {
        supplied
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                let value = match entry {
                    ArrayEntry::Value(value) => value,
                    ArrayEntry::Absent { node, reachable } => {
                        self.array_element(node, element)?;
                        self.construction_value(node, Some(element), reachable)?
                    }
                };
                Ok(FieldValue {
                    field: ValueField::Array(
                        u64::try_from(index).map_err(|_| Failure::InconsistentSyntax)?,
                    ),
                    value,
                })
            })
            .collect()
    }
    pub(in crate::value_access) fn array_repetition(
        &mut self,
        node: NodeId,
        hint: Option<TypeId>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let (seed, count) = nocter_syntax::array_repetition_parts(self.syntax().tree(), node)
            .ok_or(Failure::InconsistentSyntax)?;
        let length = self.required_usize(count)?;
        let context = self.array_context(node, hint)?;
        if context.is_some_and(|(_, array)| array.length() != length) {
            return self.reject(Rule::TypeMismatch, node);
        }
        let expected = context.map(|(_, array)| array.element());
        let mut value = self.construction_value(seed, expected, reachable)?;
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
            value = self.boundary(seed, value, Boundary::Value(element))?;
        }
        if length > 1
            && !self.copyable(element)
            && self.program().payload_readiness(element) != PayloadReadiness::Pending
        {
            return self.reject_message(
                Rule::InvalidConstruction,
                seed,
                "array repetition greater than one requires Copy element data".into(),
                Vec::new(),
            );
        }
        let data = match context {
            Some((data, array)) if array.element() == element => data,
            _ => self
                .types
                .array(element, length)
                .map_err(|_| Failure::InconsistentSyntax)?,
        };
        self.emit(
            node,
            value_type(data, Mode::Own).with_continuation(self.ty(value).may_complete()),
            ExpressionKind::Repeat { seed: value },
        )
    }
}
