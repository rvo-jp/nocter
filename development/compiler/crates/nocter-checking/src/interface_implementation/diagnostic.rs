use nocter_diagnostics::{DiagnosticNote, DiagnosticRepair, SourceDiagnostic};
use nocter_source_index::SourceOrigin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InterfaceImplementationRule {
    MissingMethod,
    ExtraMethod,
    IncompatibleMethod,
    OverlappingInterfaceImplementation,
    UnsatisfiedAssociatedBound,
    UnsatisfiedPrerequisite,
}

impl InterfaceImplementationRule {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingMethod => "E0350",
            Self::ExtraMethod => "E0351",
            Self::IncompatibleMethod => "E0352",
            Self::OverlappingInterfaceImplementation => "E0353",
            Self::UnsatisfiedAssociatedBound => "E0354",
            Self::UnsatisfiedPrerequisite => "E0358",
        }
    }
}

pub(super) fn missing_method(
    primary: SourceOrigin,
    interface_method: SourceOrigin,
) -> SourceDiagnostic {
    SourceDiagnostic::new(
        InterfaceImplementationRule::MissingMethod.code(),
        "interface implementation does not implement a required interface method",
        primary,
        [DiagnosticNote::new(
            "the required method is declared here",
            interface_method,
        )],
        Some("add one method with the interface method's name and signature"),
    )
    .with_repair(DiagnosticRepair::ImplementMissingInterfaceMethod)
}

pub(super) fn extra_method(primary: SourceOrigin, interface: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        InterfaceImplementationRule::ExtraMethod.code(),
        "interface implementation method does not implement a method of this interface",
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
        InterfaceImplementationRule::IncompatibleMethod.code(),
        "interface implementation method signature does not match its interface method",
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
        InterfaceImplementationRule::OverlappingInterfaceImplementation.code(),
        "interface implementation overlaps another interface implementation for the same interface",
        primary,
        [DiagnosticNote::new(
            "the overlapping interface implementation is declared here",
            previous,
        )],
        Some("make the target or interface patterns disjoint"),
    )
}

pub(super) fn unsatisfied_associated_bound(
    primary: SourceOrigin,
    bounds: impl IntoIterator<Item = SourceOrigin>,
) -> SourceDiagnostic {
    SourceDiagnostic::new(
        InterfaceImplementationRule::UnsatisfiedAssociatedBound.code(),
        "interface implementation associated type does not satisfy its declared capability",
        primary,
        bounds
            .into_iter()
            .map(|bound| {
                DiagnosticNote::new("the associated type capability is required here", bound)
            })
            .collect::<Vec<_>>(),
        Some("select a type with this capability or add the required conditional bound"),
    )
}

pub(super) fn unsatisfied_prerequisite(
    primary: SourceOrigin,
    prerequisites: impl IntoIterator<Item = SourceOrigin>,
) -> SourceDiagnostic {
    SourceDiagnostic::new(
        InterfaceImplementationRule::UnsatisfiedPrerequisite.code(),
        "interface implementation does not satisfy an interface prerequisite",
        primary,
        prerequisites
            .into_iter()
            .map(|prerequisite| {
                DiagnosticNote::new("the prerequisite is declared here", prerequisite)
            })
            .collect::<Vec<_>>(),
        Some("add the required implementation or structural operation for the same target"),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::InterfaceImplementationRule;

    #[test]
    fn interface_implementation_rule_codes_are_closed_and_unique() {
        let rules = [
            InterfaceImplementationRule::MissingMethod,
            InterfaceImplementationRule::ExtraMethod,
            InterfaceImplementationRule::IncompatibleMethod,
            InterfaceImplementationRule::OverlappingInterfaceImplementation,
            InterfaceImplementationRule::UnsatisfiedAssociatedBound,
            InterfaceImplementationRule::UnsatisfiedPrerequisite,
        ];
        let codes: HashSet<_> = rules
            .into_iter()
            .map(InterfaceImplementationRule::code)
            .collect();
        assert_eq!(codes.len(), rules.len());
    }
}
