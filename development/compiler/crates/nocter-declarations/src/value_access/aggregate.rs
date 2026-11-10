//! One closed component projection for nominal records and structural tuples.
use super::{Component, Field, Record};
use nocter_model::{AccessFieldId, SparseArena, TypeId, ValueField};

/// A positive family projection; consumers must handle every admitted aggregate family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AggregateForm<'a> {
    Record(&'a Record),
    Tuple(&'a [TypeId]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AggregateSchema<'a>(Shape<'a>);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Shape<'a> {
    Record(&'a Record, &'a SparseArena<AccessFieldId, Field>),
    Tuple(&'a [TypeId]),
}
impl<'a> AggregateSchema<'a> {
    pub(super) const fn record(
        record: &'a Record,
        fields: &'a SparseArena<AccessFieldId, Field>,
    ) -> Self {
        Self(Shape::Record(record, fields))
    }
    pub(super) const fn tuple(fields: &'a [TypeId]) -> Self {
        Self(Shape::Tuple(fields))
    }
    #[must_use]
    pub const fn form(self) -> AggregateForm<'a> {
        match self.0 {
            Shape::Record(record, _) => AggregateForm::Record(record),
            Shape::Tuple(fields) => AggregateForm::Tuple(fields),
        }
    }
    #[must_use]
    pub fn len(self) -> usize {
        match self.0 {
            Shape::Record(record, _) => record.fields().len(),
            Shape::Tuple(fields) => fields.len(),
        }
    }
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    #[must_use]
    pub fn component(self, field: ValueField) -> Option<Component> {
        match (self.0, field) {
            (Shape::Record(record, fields), ValueField::Record(id)) => {
                let selected = fields.get(id)?;
                (record.fields().get(selected.position()) == Some(&id)).then_some(Component {
                    field,
                    data: selected.data(),
                })
            }
            (Shape::Tuple(fields), ValueField::Tuple(position)) => Some(Component {
                field,
                data: *fields.get(position)?,
            }),
            _ => None,
        }
    }
    fn component_at(self, position: usize) -> Component {
        let field = match self.0 {
            Shape::Record(record, _) => ValueField::Record(record.fields()[position]),
            Shape::Tuple(_) => ValueField::Tuple(position),
        };
        self.component(field).expect("validated closed aggregate")
    }
    #[must_use]
    pub fn components(self) -> impl DoubleEndedIterator<Item = Component> + ExactSizeIterator + 'a {
        (0..self.len()).map(move |position| self.component_at(position))
    }
}

#[cfg(test)]
mod tests;
