use crate::AccessFieldId;

/// A logical aggregate component, independent of source spelling and physical offsets.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueField {
    Record(AccessFieldId),
    Tuple(usize),
    /// A concrete logical array position, not a tuple ordinal or a dynamic-index proof.
    Array(u64),
}
impl From<AccessFieldId> for ValueField {
    fn from(field: AccessFieldId) -> Self {
        Self::Record(field)
    }
}

/// An ordered logical field selection, interpreted only with its owning declaration authority.
/// Names, memory offsets and permissions do not participate in projection identity.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FieldPath(Box<[ValueField]>);

impl FieldPath {
    #[must_use]
    pub fn new<F: Into<ValueField>>(fields: impl IntoIterator<Item = F>) -> Self {
        Self(fields.into_iter().map(Into::into).collect())
    }

    #[must_use]
    pub const fn fields(&self) -> &[ValueField] {
        &self.0
    }

    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn project(&self, field: impl Into<ValueField>) -> Self {
        let mut fields = self.0.to_vec();
        fields.push(field.into());
        Self::new(fields)
    }

    #[must_use]
    pub fn concat(&self, suffix: &Self) -> Self {
        Self::new(
            self.fields()
                .iter()
                .chain(suffix.fields())
                .copied()
                .collect::<Vec<_>>(),
        )
    }

    #[must_use]
    pub fn is_prefix_of(&self, other: &Self) -> bool {
        other.0.starts_with(&self.0)
    }

    /// Overlap for valid paths on the same root. Sibling fields are disjoint even at equal offsets.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.is_prefix_of(other) || other.is_prefix_of(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ArenaBuilder;

    #[test]
    fn logical_prefixes_overlap_but_distinct_siblings_do_not() {
        let mut fields = ArenaBuilder::<AccessFieldId, ()>::new();
        let a = fields.insert(());
        let b = fields.insert(());
        let root = FieldPath::default();
        let left = root.project(a);
        let right = root.project(b);
        let nested = left.project(b);
        for (one, other, overlap) in [
            (&root, &root, true),
            (&root, &nested, true),
            (&left, &left, true),
            (&left, &nested, true),
            (&left, &right, false),
            (&nested, &right, false),
        ] {
            assert_eq!(one.overlaps(other), overlap);
            assert_eq!(other.overlaps(one), overlap);
        }
        assert!(root.is_root());
        assert!(!nested.is_root());
        assert!(root.is_prefix_of(&nested));
        assert!(!nested.is_prefix_of(&root));
        assert_eq!(
            nested.fields(),
            [ValueField::Record(a), ValueField::Record(b)]
        );
        let element = left.project(ValueField::Array(u64::MAX - 1));
        assert!(left.overlaps(&element));
        assert!(!element.overlaps(&left.project(ValueField::Array(0))));
    }
}
