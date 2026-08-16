use nocter_diagnostics::{DiagnosticNote, SourceDiagnostic};
use nocter_source_index::SourceOrigin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InstanceOperationRule {
    OverlappingInstance,
}

impl InstanceOperationRule {
    pub const ALL: &'static [Self] = &[Self::OverlappingInstance];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::OverlappingInstance => "E0355",
        }
    }
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
