use nocter_diagnostics::{DiagnosticNote, SourceDiagnostic};
use nocter_source_index::SourceOrigin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConformanceRule {
    MissingMethod,
    ExtraMethod,
    IncompatibleMethod,
    OverlappingConformance,
    UnsatisfiedAssociatedBound,
}

impl ConformanceRule {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingMethod => "E0350",
            Self::ExtraMethod => "E0351",
            Self::IncompatibleMethod => "E0352",
            Self::OverlappingConformance => "E0353",
            Self::UnsatisfiedAssociatedBound => "E0354",
        }
    }
}

pub(super) fn missing_method(
    primary: SourceOrigin,
    interface_method: SourceOrigin,
) -> SourceDiagnostic {
    SourceDiagnostic::new(
        ConformanceRule::MissingMethod.code(),
        "conformance does not implement a required interface method",
        primary,
        [DiagnosticNote::new(
            "the required method is declared here",
            interface_method,
        )],
        Some("add one method with the interface method's name and signature"),
    )
}

pub(super) fn extra_method(primary: SourceOrigin, interface: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        ConformanceRule::ExtraMethod.code(),
        "conformance method does not implement a method of this interface",
        primary,
        [DiagnosticNote::new(
            "the interface is declared here",
            interface,
        )],
        Some("remove the method or declare it on the interface"),
    )
}

pub(super) fn incompatible_method(
    primary: SourceOrigin,
    interface_method: SourceOrigin,
) -> SourceDiagnostic {
    SourceDiagnostic::new(
        ConformanceRule::IncompatibleMethod.code(),
        "conformance method signature does not match its interface method",
        primary,
        [DiagnosticNote::new(
            "the required signature is declared here",
            interface_method,
        )],
        Some(
            "match the receiver, generic arity, parameter types, result, requirements, and provenance contract",
        ),
    )
}

pub(super) fn overlapping(primary: SourceOrigin, previous: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        ConformanceRule::OverlappingConformance.code(),
        "conformance overlaps another conformance for the same interface",
        primary,
        [DiagnosticNote::new(
            "the overlapping conformance is declared here",
            previous,
        )],
        Some("make the target or interface patterns disjoint"),
    )
}

pub(super) fn unsatisfied_associated_bound(
    primary: SourceOrigin,
    bound: SourceOrigin,
) -> SourceDiagnostic {
    SourceDiagnostic::new(
        ConformanceRule::UnsatisfiedAssociatedBound.code(),
        "conformance associated type does not satisfy its declared capability",
        primary,
        [DiagnosticNote::new(
            "the associated type capability is required here",
            bound,
        )],
        Some("select a type with this capability or add the required conditional bound"),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::ConformanceRule;

    #[test]
    fn conformance_rule_codes_are_closed_and_unique() {
        let rules = [
            ConformanceRule::MissingMethod,
            ConformanceRule::ExtraMethod,
            ConformanceRule::IncompatibleMethod,
            ConformanceRule::OverlappingConformance,
            ConformanceRule::UnsatisfiedAssociatedBound,
        ];
        let codes: HashSet<_> = rules.into_iter().map(ConformanceRule::code).collect();
        assert_eq!(codes.len(), rules.len());
    }
}
