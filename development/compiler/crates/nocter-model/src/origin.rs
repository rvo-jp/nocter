use std::fmt;

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

/// The caller-managed parameter origins retained by a callable result.
///
/// The collection is sorted, unique, and independent from source clause order. Static and fresh
/// storage retain no caller place and are represented by an empty set.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResultProvenance(Box<[ParameterOrigin]>);

impl ResultProvenance {
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
    use super::{ParameterOrigin, ResultProvenance};

    #[test]
    fn provenance_is_a_canonical_parameter_set() {
        let first = ParameterOrigin::new(0);
        let second = ParameterOrigin::new(1);
        let provenance = ResultProvenance::from_origins([second, first]).unwrap();

        assert_eq!(provenance.origins(), &[first, second]);
        assert_eq!(
            ResultProvenance::from_origins([first, first])
                .unwrap_err()
                .origin(),
            first
        );
    }
}
