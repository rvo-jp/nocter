use nocter_diagnostics::SourceDiagnostic;
use nocter_source_index::SourceOrigin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BodyRule {
    TypeMismatch,
    ImplicitMove,
    InvalidStatementValue,
    MissingBodyResult,
    UnreachableCode,
    IntegerOutOfRange,
}

impl BodyRule {
    pub const ALL: &'static [Self] = &[
        Self::TypeMismatch,
        Self::ImplicitMove,
        Self::InvalidStatementValue,
        Self::MissingBodyResult,
        Self::UnreachableCode,
        Self::IntegerOutOfRange,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TypeMismatch => "E0370",
            Self::ImplicitMove => "E0371",
            Self::InvalidStatementValue => "E0372",
            Self::MissingBodyResult => "E0373",
            Self::UnreachableCode => "E0374",
            Self::IntegerOutOfRange => "E0375",
        }
    }

    pub(super) fn diagnostic(self, primary: SourceOrigin) -> SourceDiagnostic {
        let (message, help) = match self {
            Self::TypeMismatch => (
                "expression type is incompatible with its expected destination type",
                "produce the exact expected type or use one applicable explicit conversion",
            ),
            Self::ImplicitMove => (
                "using this place would require an implicit move",
                "write `move place` when ownership transfer is intended",
            ),
            Self::InvalidStatementValue => (
                "non-final expression statement produces a value",
                "bind the value, make it the body result, or explicitly discard it with `let _ =`",
            ),
            Self::MissingBodyResult => (
                "callable can complete without producing its declared result",
                "add a body result or return on every reachable path",
            ),
            Self::UnreachableCode => (
                "statement is unreachable",
                "remove it or move it before the terminating expression",
            ),
            Self::IntegerOutOfRange => (
                "integer literal is outside the expected integer type's range",
                "use a value representable by the destination integer type",
            ),
        };
        SourceDiagnostic::new(self.code(), message, primary, [], Some(help))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::BodyRule;

    #[test]
    fn body_rule_codes_are_closed_and_unique() {
        let codes = BodyRule::ALL
            .iter()
            .copied()
            .map(BodyRule::code)
            .collect::<HashSet<_>>();
        assert_eq!(codes.len(), BodyRule::ALL.len());
    }
}
