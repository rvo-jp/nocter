use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Proves that one provenance atom is bounded by the union of `allowed` atoms.
///
/// `sources` supplies normalized graph edges of the form `target ⊆ source₁ ∪ ...`. Every
/// branch must reach an allowed atom; a cycle without such a bound proves nothing. Empty source
/// sets represent storage-independent values and therefore satisfy every upper bound, including an
/// empty one.
#[must_use]
pub fn provenance_is_bounded_by<'a, T>(
    target: T,
    allowed: &[T],
    mut sources: impl FnMut(T) -> Option<&'a [T]>,
) -> bool
where
    T: Copy + Ord + 'a,
{
    fn prove<'a, T>(
        current: T,
        allowed: &[T],
        sources: &mut impl FnMut(T) -> Option<&'a [T]>,
        active: &mut BTreeSet<T>,
        memo: &mut BTreeMap<T, bool>,
    ) -> bool
    where
        T: Copy + Ord + 'a,
    {
        if allowed.contains(&current) {
            return true;
        }
        if let Some(result) = memo.get(&current) {
            return *result;
        }
        if !active.insert(current) {
            return false;
        }
        let result = sources(current).is_some_and(|next| {
            next.iter()
                .copied()
                .all(|origin| prove(origin, allowed, sources, active, memo))
        });
        active.remove(&current);
        memo.insert(current, result);
        result
    }

    prove(
        target,
        allowed,
        &mut sources,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
    )
}

/// A zero-based callable-parameter position in normalized result provenance.
///
/// Authored names are resolved before this value is created. Parameter names therefore do not
/// become part of structural callable identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ParameterOrigin(usize);

impl ParameterOrigin {
    #[must_use]
    pub const fn new(position: usize) -> Self {
        Self(position)
    }

    #[must_use]
    pub const fn position(self) -> usize {
        self.0
    }
}

/// One canonical set of caller-managed callable-parameter origins.
///
/// The collection is sorted, unique, and independent from source clause order. Static and fresh
/// storage retain no caller place and are represented by an empty set.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProvenanceSet(Box<[ParameterOrigin]>);

impl ProvenanceSet {
    #[must_use]
    pub fn empty() -> Self {
        Self(Box::new([]))
    }

    /// Creates a canonical origin set.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateOrigin`] when one parameter position occurs more than once. Duplicate
    /// authored origins remain a semantic diagnostic instead of disappearing during normalization.
    pub fn from_origins(
        origins: impl IntoIterator<Item = ParameterOrigin>,
    ) -> Result<Self, DuplicateOrigin> {
        let mut origins: Vec<_> = origins.into_iter().collect();
        origins.sort_unstable();
        if let Some(duplicate) = origins
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0])
        {
            return Err(DuplicateOrigin(duplicate));
        }
        Ok(Self(origins.into_boxed_slice()))
    }

    #[must_use]
    pub const fn origins(&self) -> &[ParameterOrigin] {
        &self.0
    }
}

/// One structural callable input whose provenance is bounded by other inputs.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InputProvenanceConstraint {
    target: ParameterOrigin,
    sources: ProvenanceSet,
}

impl InputProvenanceConstraint {
    #[must_use]
    pub const fn new(target: ParameterOrigin, sources: ProvenanceSet) -> Self {
        Self { target, sources }
    }

    #[must_use]
    pub const fn target(&self) -> ParameterOrigin {
        self.target
    }

    #[must_use]
    pub const fn sources(&self) -> &ProvenanceSet {
        &self.sources
    }
}

/// The normalized provenance bounds attached to structural callable inputs.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InputProvenance(Box<[InputProvenanceConstraint]>);

impl InputProvenance {
    #[must_use]
    pub fn empty() -> Self {
        Self(Box::new([]))
    }

    /// Creates constraints sorted by target position.
    ///
    /// # Errors
    ///
    /// A target may occur only once and cannot name itself as one of its sources.
    pub fn from_constraints(
        constraints: impl IntoIterator<Item = InputProvenanceConstraint>,
    ) -> Result<Self, InvalidInputProvenance> {
        let mut constraints: Vec<_> = constraints.into_iter().collect();
        constraints.sort_unstable_by_key(InputProvenanceConstraint::target);
        for constraint in &constraints {
            if constraint
                .sources()
                .origins()
                .contains(&constraint.target())
            {
                return Err(InvalidInputProvenance::Tautology(constraint.target()));
            }
        }
        if let Some(target) = constraints
            .windows(2)
            .find(|pair| pair[0].target() == pair[1].target())
            .map(|pair| pair[0].target())
        {
            return Err(InvalidInputProvenance::DuplicateTarget(target));
        }
        Ok(Self(constraints.into_boxed_slice()))
    }

    #[must_use]
    pub const fn constraints(&self) -> &[InputProvenanceConstraint] {
        &self.0
    }

    #[must_use]
    pub fn sources(&self, target: ParameterOrigin) -> Option<&ProvenanceSet> {
        self.0
            .binary_search_by_key(&target, InputProvenanceConstraint::target)
            .ok()
            .map(|index| self.0[index].sources())
    }

    /// Reports whether these assumptions prove every constraint required by `required`.
    #[must_use]
    pub fn implies(&self, required: &Self) -> bool {
        required.constraints().iter().all(|constraint| {
            provenance_is_bounded_by(
                constraint.target(),
                constraint.sources().origins(),
                |origin| self.sources(origin).map(ProvenanceSet::origins),
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidInputProvenance {
    DuplicateTarget(ParameterOrigin),
    Tautology(ParameterOrigin),
}

impl InvalidInputProvenance {
    #[must_use]
    pub const fn target(self) -> ParameterOrigin {
        match self {
            Self::DuplicateTarget(target) | Self::Tautology(target) => target,
        }
    }
}

impl fmt::Display for InvalidInputProvenance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateTarget(_) => {
                formatter.write_str("callable input has more than one provenance constraint")
            }
            Self::Tautology(_) => {
                formatter.write_str("callable input provenance names the input itself")
            }
        }
    }
}

impl std::error::Error for InvalidInputProvenance {}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DuplicateOrigin(ParameterOrigin);

impl DuplicateOrigin {
    #[must_use]
    pub const fn origin(self) -> ParameterOrigin {
        self.0
    }
}

impl fmt::Debug for DuplicateOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DuplicateOrigin")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for DuplicateOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "parameter origin {} occurs more than once",
            self.0.position()
        )
    }
}

impl std::error::Error for DuplicateOrigin {}

#[cfg(test)]
mod tests {
    use super::{
        InputProvenance, InputProvenanceConstraint, InvalidInputProvenance, ParameterOrigin,
        ProvenanceSet,
    };

    #[test]
    fn provenance_is_a_canonical_parameter_set() {
        let first = ParameterOrigin::new(0);
        let second = ParameterOrigin::new(1);
        let provenance = ProvenanceSet::from_origins([second, first]).unwrap();

        assert_eq!(provenance.origins(), &[first, second]);
        assert_eq!(
            ProvenanceSet::from_origins([first, first])
                .unwrap_err()
                .origin(),
            first
        );
    }

    #[test]
    fn input_constraints_are_canonical_and_non_tautological() {
        let first = ParameterOrigin::new(0);
        let second = ParameterOrigin::new(1);
        let inputs = InputProvenance::from_constraints([InputProvenanceConstraint::new(
            second,
            ProvenanceSet::from_origins([first]).unwrap(),
        )])
        .unwrap();

        assert_eq!(inputs.sources(second).unwrap().origins(), &[first]);
        assert_eq!(
            InputProvenance::from_constraints([InputProvenanceConstraint::new(
                first,
                ProvenanceSet::from_origins([first]).unwrap(),
            )])
            .unwrap_err(),
            InvalidInputProvenance::Tautology(first)
        );
    }

    #[test]
    fn input_implication_requires_every_union_branch_and_handles_cycles() {
        let first = ParameterOrigin::new(0);
        let second = ParameterOrigin::new(1);
        let third = ParameterOrigin::new(2);
        let fourth = ParameterOrigin::new(3);
        let available = InputProvenance::from_constraints([
            InputProvenanceConstraint::new(
                first,
                ProvenanceSet::from_origins([second, third]).unwrap(),
            ),
            InputProvenanceConstraint::new(second, ProvenanceSet::from_origins([first]).unwrap()),
        ])
        .unwrap();
        let admits_union = InputProvenance::from_constraints([InputProvenanceConstraint::new(
            first,
            ProvenanceSet::from_origins([second, third]).unwrap(),
        )])
        .unwrap();
        let drops_branch = InputProvenance::from_constraints([InputProvenanceConstraint::new(
            first,
            ProvenanceSet::from_origins([second]).unwrap(),
        )])
        .unwrap();
        let unrelated = InputProvenance::from_constraints([InputProvenanceConstraint::new(
            first,
            ProvenanceSet::from_origins([fourth]).unwrap(),
        )])
        .unwrap();

        assert!(available.implies(&admits_union));
        assert!(!available.implies(&drops_branch));
        assert!(!available.implies(&unrelated));
    }
}
