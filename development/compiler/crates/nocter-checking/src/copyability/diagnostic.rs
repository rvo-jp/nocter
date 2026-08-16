use std::fmt;

use nocter_diagnostics::SourceDiagnostic;
use nocter_source_index::SourceOrigin;

use super::CopyabilityError;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CopyabilityRule {
    UnconditionallyMoveOnlyField,
}

impl CopyabilityRule {
    pub const ALL: &'static [Self] = &[Self::UnconditionallyMoveOnlyField];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnconditionallyMoveOnlyField => "E0366",
        }
    }

    pub(super) fn diagnostic(self, primary: SourceOrigin) -> SourceDiagnostic {
        match self {
            Self::UnconditionallyMoveOnlyField => SourceDiagnostic::new(
                self.code(),
                "copy struct field is move-only for every specialization",
                primary,
                [],
                Some("remove `copy` or use a copyable or generic-dependent field type"),
            ),
        }
    }
}

#[derive(Debug)]
pub enum CopyabilityBuildError {
    Rule(SourceDiagnostic),
    Internal(CopyabilityError),
}

impl CopyabilityBuildError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Rule(diagnostic) => Some(diagnostic),
            Self::Internal(_) => None,
        }
    }
}

impl fmt::Display for CopyabilityBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(diagnostic) => {
                write!(formatter, "{}: {}", diagnostic.code(), diagnostic.message())
            }
            Self::Internal(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CopyabilityBuildError {}

impl From<CopyabilityError> for CopyabilityBuildError {
    fn from(error: CopyabilityError) -> Self {
        Self::Internal(error)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::CopyabilityRule;

    #[test]
    fn copyability_rule_codes_are_closed_and_unique() {
        let codes = CopyabilityRule::ALL
            .iter()
            .copied()
            .map(CopyabilityRule::code)
            .collect::<HashSet<_>>();
        assert_eq!(codes.len(), CopyabilityRule::ALL.len());
    }
}
