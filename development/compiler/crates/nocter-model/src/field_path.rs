use crate::AccessFieldId;

/// A logical aggregate component, independent of source spelling and physical offsets.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueProjection<I> {
    Record(AccessFieldId),
    Tuple(usize),
    /// An index value or recipe key; its identity alone does not prove evaluated equality.
    Array(I),
}
impl<I> From<AccessFieldId> for ValueProjection<I> {
    fn from(field: AccessFieldId) -> Self {
        Self::Record(field)
    }
}

/// Concrete component-key domain for declaration-validated projections, independent of index recipes.
pub type ValueField = ValueProjection<u64>;

/// Established relation between two evaluated indices in the caller's exact proof context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexRelation {
    Equal,
    Distinct,
    Unknown,
}

/// Logical region relation on the same root; unknown index equality cannot prove containment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathRelation {
    Equal,
    Ancestor,
    Descendant,
    Disjoint,
    MayOverlap,
}

/// An ordered logical field selection, interpreted only with its owning declaration authority.
/// Names, memory offsets and permissions do not participate in projection identity.
/// Equality of recipe keys is not evidence that runtime evaluations select the same element.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValuePath<I>(Box<[ValueProjection<I>]>);

/// The concrete path domain used by declaration projection and the concrete ownership ledger.
pub type FieldPath = ValuePath<u64>;

impl<I> Default for ValuePath<I> {
    fn default() -> Self {
        Self(Box::default())
    }
}

impl<I> ValuePath<I> {
    #[must_use]
    pub fn new<F: Into<ValueProjection<I>>>(fields: impl IntoIterator<Item = F>) -> Self {
        Self(fields.into_iter().map(Into::into).collect())
    }

    #[must_use]
    pub const fn fields(&self) -> &[ValueProjection<I>] {
        &self.0
    }

    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// Compare validated builtin structural paths on one root. The oracle must describe the
    /// evaluated indices in this context, not source spelling, operation IDs or physical offsets.
    /// Mismatched component families supply no disjointness proof; their schema validity belongs
    /// to the caller. Unknown indices can still be separated by distinct later component fields.
    #[must_use]
    pub fn relation(
        &self,
        other: &Self,
        mut indices: impl FnMut(&I, &I) -> IndexRelation,
    ) -> PathRelation {
        let mut uncertain = false;
        for (left, right) in self.fields().iter().zip(other.fields()) {
            let relation = match (left, right) {
                (ValueProjection::Record(a), ValueProjection::Record(b)) => {
                    if a == b {
                        IndexRelation::Equal
                    } else {
                        IndexRelation::Distinct
                    }
                }
                (ValueProjection::Tuple(a), ValueProjection::Tuple(b)) => {
                    if a == b {
                        IndexRelation::Equal
                    } else {
                        IndexRelation::Distinct
                    }
                }
                (ValueProjection::Array(a), ValueProjection::Array(b)) => indices(a, b),
                _ => return PathRelation::MayOverlap,
            };
            match relation {
                IndexRelation::Equal => {}
                IndexRelation::Distinct => return PathRelation::Disjoint,
                IndexRelation::Unknown => uncertain = true,
            }
        }
        if uncertain {
            return PathRelation::MayOverlap;
        }
        match self.fields().len().cmp(&other.fields().len()) {
            std::cmp::Ordering::Equal => PathRelation::Equal,
            std::cmp::Ordering::Less => PathRelation::Ancestor,
            std::cmp::Ordering::Greater => PathRelation::Descendant,
        }
    }
}

impl<I: Clone> ValuePath<I> {
    #[must_use]
    pub fn project(&self, field: impl Into<ValueProjection<I>>) -> Self {
        let mut fields = self.0.to_vec();
        fields.push(field.into());
        Self::new(fields)
    }

    #[must_use]
    pub fn concat(&self, suffix: &Self) -> Self {
        Self::new(self.fields().iter().chain(suffix.fields()).cloned())
    }
}

impl ValuePath<u64> {
    fn concrete_relation(&self, other: &Self) -> PathRelation {
        self.relation(other, |left, right| {
            if left == right {
                IndexRelation::Equal
            } else {
                IndexRelation::Distinct
            }
        })
    }
    #[must_use]
    pub fn is_prefix_of(&self, other: &Self) -> bool {
        matches!(
            self.concrete_relation(other),
            PathRelation::Equal | PathRelation::Ancestor
        )
    }

    /// Overlap for valid paths on the same root. Sibling fields are disjoint even at equal offsets.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.concrete_relation(other) != PathRelation::Disjoint
    }
}

#[cfg(test)]
mod relations;

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
