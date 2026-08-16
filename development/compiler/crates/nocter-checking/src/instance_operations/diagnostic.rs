use nocter_diagnostics::{DiagnosticNote, SourceDiagnostic};
use nocter_source_index::SourceOrigin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InstanceOperationRule {
    OverlappingInstance,
    DuplicateCoercion,
}

impl InstanceOperationRule {
    pub const ALL: &'static [Self] = &[Self::OverlappingInstance, Self::DuplicateCoercion];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::OverlappingInstance => "E0355",
            Self::DuplicateCoercion => "E0356",
        }
    }
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
            "the first coercion with this receiver capability and target is declared here",
            previous,
        )],
        Some("keep one coercion for each receiver-capability and canonical-target pair"),
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
