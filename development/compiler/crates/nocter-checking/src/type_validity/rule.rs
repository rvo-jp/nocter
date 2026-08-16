use nocter_diagnostics::SourceDiagnostic;
use nocter_source_index::SourceOrigin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TypeValidityRule {
    InvalidOutcomeShape,
    OptionalVoid,
    OutcomeNever,
    NeverData,
    VoidData,
    UnsizedData,
}

impl TypeValidityRule {
    pub const ALL: &'static [Self] = &[
        Self::InvalidOutcomeShape,
        Self::OptionalVoid,
        Self::OutcomeNever,
        Self::NeverData,
        Self::VoidData,
        Self::UnsizedData,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidOutcomeShape => "E0360",
            Self::OptionalVoid => "E0361",
            Self::OutcomeNever => "E0362",
            Self::NeverData => "E0363",
            Self::VoidData => "E0364",
            Self::UnsizedData => "E0365",
        }
    }

    pub(super) fn diagnostic(self, primary: SourceOrigin) -> SourceDiagnostic {
        let (message, help) = match self {
            Self::InvalidOutcomeShape => (
                "outcome type repeats a layer or contains more than two layers",
                "use at most one optional layer and one fallible layer",
            ),
            Self::OptionalVoid => (
                "optional outcome has `void` as its eventual payload",
                "use `void!` for recoverable completion or an enum for absence versus completion",
            ),
            Self::OutcomeNever => (
                "outcome type has `never` as its eventual payload",
                "use `void!` for recoverable completion or an enum for a value-level state",
            ),
            Self::NeverData => (
                "`never` is used in a data-bearing type position",
                "use `never` only as a complete callable result type",
            ),
            Self::VoidData => (
                "`void` is used in a data-bearing type position",
                "use an empty struct when a storable zero-sized value is required",
            ),
            Self::UnsizedData => (
                "unsized `str` or `[T]` is used by value",
                "place the unsized data behind a borrow or raw pointer",
            ),
        };
        SourceDiagnostic::new(self.code(), message, primary, [], Some(help))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::TypeValidityRule;

    #[test]
    fn type_validity_codes_are_closed_and_unique() {
        let codes = TypeValidityRule::ALL
            .iter()
            .copied()
            .map(TypeValidityRule::code)
            .collect::<HashSet<_>>();
        assert_eq!(codes.len(), TypeValidityRule::ALL.len());
    }
}
