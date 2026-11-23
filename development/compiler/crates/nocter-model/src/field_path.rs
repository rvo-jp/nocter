use crate::{AccessFieldId, VariantId};

/// A logical aggregate component, independent of source spelling and physical offsets.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueProjection<I> {
    Record(AccessFieldId),
    Tuple(usize),
    /// A typed active-variant payload; datatype projection alone grants no active-tag access.
    Variant(bool),
    /// A nominal enum payload position. The case belongs to the projection, not to an offset.
    EnumField {
        variant: VariantId,
        field: AccessFieldId,
    },
    /// An index value or recipe key; its identity alone does not prove evaluated equality.
    Array(I),
}
impl<I> From<AccessFieldId> for ValueProjection<I> {
    fn from(field: AccessFieldId) -> Self {
        Self::Record(field)
    }
}
impl<I> ValueProjection<I> {
    /// Change the array-index domain while preserving every declaration-owned selector.
    #[must_use]
    pub fn map_index<J>(&self, map: impl FnOnce(&I) -> J) -> ValueProjection<J> {
        match self.try_map_index(|index| Ok::<_, std::convert::Infallible>(map(index))) {
            Ok(projection) => projection,
            Err(never) => match never {},
        }
    }

    /// Interpret an array selector only if the caller can resolve its index.
    /// # Errors
    /// Propagates the caller's error for an array index.
    pub fn try_map_index<J, E>(
        &self,
        map: impl FnOnce(&I) -> Result<J, E>,
    ) -> Result<ValueProjection<J>, E> {
        Ok(match self {
            Self::Record(field) => ValueProjection::Record(*field),
            Self::Tuple(index) => ValueProjection::Tuple(*index),
            Self::Variant(primary) => ValueProjection::Variant(*primary),
            Self::EnumField { variant, field } => ValueProjection::EnumField {
                variant: *variant,
                field: *field,
            },
            Self::Array(index) => ValueProjection::Array(map(index)?),
        })
    }
}

/// Concrete component-key domain for declaration-validated projections, independent of index recipes.
pub type ValueField = ValueProjection<u64>;

/// Disposition of one source-visible bounds check. Checking is the sole semantic producer;
/// consumers preserve the decision rather than strengthening it from constants or layout.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IndexBoundsCheck {
    Required,
    ProvenInBounds,
    ProvenTrap,
}

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

    /// Recognize one array selection followed by an exact suffix. This checks shape only;
    /// equality of the selected number and ownership of its backing require separate evidence.
    #[must_use]
    pub fn matches_array_element_shape(&self, array: &Self, suffix: &Self) -> bool
    where
        I: PartialEq,
    {
        let Some((ValueProjection::Array(_), tail)) = self
            .fields()
            .strip_prefix(array.fields())
            .and_then(|tail| tail.split_first())
        else {
            return false;
        };
        tail == suffix.fields()
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
                (ValueProjection::Variant(a), ValueProjection::Variant(b)) if a == b => {
                    IndexRelation::Equal
                }
                (
                    ValueProjection::EnumField {
                        variant: a,
                        field: left,
                    },
                    ValueProjection::EnumField {
                        variant: b,
                        field: right,
                    },
                ) if a == b => {
                    if left == right {
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

#[cfg(test)]
mod variant_tests {
    use super::*;
    #[test]
    fn variant_keys_never_supply_cross_variant_disjointness_or_active_tag_proof() {
        let first = FieldPath::new([ValueField::Variant(true)]);
        let second = FieldPath::new([ValueField::Variant(false)]);
        assert_eq!(
            first.relation(&first, |_, _| IndexRelation::Unknown),
            PathRelation::Equal
        );
        assert_eq!(
            first.relation(&second, |_, _| IndexRelation::Unknown),
            PathRelation::MayOverlap
        );
    }
}
