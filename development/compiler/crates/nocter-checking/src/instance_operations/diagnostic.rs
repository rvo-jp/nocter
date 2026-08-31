use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_source_index::SourceOrigin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InstanceOperationRule {
    OverlappingInstance,
    DuplicateCoercion,
    InvalidSignature,
}

impl InstanceOperationRule {
    pub const ALL: &'static [Self] = &[
        Self::OverlappingInstance,
        Self::DuplicateCoercion,
        Self::InvalidSignature,
    ];

    #[must_use]
    pub const fn code(self) -> DiagnosticCode {
        match self {
            Self::OverlappingInstance => DiagnosticCode::E0355,
            Self::DuplicateCoercion => DiagnosticCode::E0356,
            Self::InvalidSignature => DiagnosticCode::E0357,
        }
    }
}

pub(super) fn invalid_signature(primary: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        InstanceOperationRule::InvalidSignature.code(),
        "instance operation has an invalid signature",
        primary,
        [],
        Some("use the receiver, parameters, and result required by this operation kind"),
    )
}

pub(super) fn duplicate_coercion(
    primary: SourceOrigin,
    previous: SourceOrigin,
) -> SourceDiagnostic {
    SourceDiagnostic::new(
        InstanceOperationRule::DuplicateCoercion.code(),
        "instance repeats the same borrow coercion identity",
        primary,
        [DiagnosticNote::new(
            "the first coercion with these borrow capabilities and target is declared here",
            previous,
        )],
        Some("keep one coercion for each receiver/result-capability and canonical-target identity"),
    )
}

pub(super) fn overlapping(primary: SourceOrigin, previous: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        InstanceOperationRule::OverlappingInstance.code(),
        "instance target pattern overlaps another instance",
        primary,
        [DiagnosticNote::new(
            "the overlapping instance is declared here",
            previous,
        )],
        Some("make the target patterns disjoint; declaration order never selects an instance"),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::InstanceOperationRule;

    #[test]
    fn instance_operation_rule_codes_are_closed_and_unique() {
        let codes = InstanceOperationRule::ALL
            .iter()
            .copied()
            .map(InstanceOperationRule::code)
            .collect::<HashSet<_>>();
        assert_eq!(codes.len(), InstanceOperationRule::ALL.len());
    }
}
