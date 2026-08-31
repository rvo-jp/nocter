use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, DiagnosticRepair, SourceDiagnostic};
use nocter_source_index::SourceOrigin;

/// Closed body-name rule family.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NameRule {
    UnknownName,
    NameCollision,
    MissingBlockImport,
    InaccessibleBlockImport,
    InvalidCaptureTarget,
    CaptureCollision,
    ImplicitCapture,
    MissingModuleMember,
    InaccessibleModuleMember,
    NonTypeSelection,
}

impl NameRule {
    #[must_use]
    pub const fn code(self) -> DiagnosticCode {
        match self {
            Self::UnknownName => DiagnosticCode::E0340,
            Self::NameCollision => DiagnosticCode::E0341,
            Self::MissingBlockImport => DiagnosticCode::E0342,
            Self::InaccessibleBlockImport => DiagnosticCode::E0343,
            Self::InvalidCaptureTarget => DiagnosticCode::E0344,
            Self::CaptureCollision => DiagnosticCode::E0345,
            Self::ImplicitCapture => DiagnosticCode::E0346,
            Self::MissingModuleMember => DiagnosticCode::E0347,
            Self::InaccessibleModuleMember => DiagnosticCode::E0348,
            Self::NonTypeSelection => DiagnosticCode::E0349,
        }
    }
}

pub(super) fn unknown_name(name: &str, primary: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        NameRule::UnknownName.code(),
        format!("name `{name}` is not visible in this body"),
        primary,
        [],
        Some("declare or import the name before using it"),
    )
    .with_repair(DiagnosticRepair::ImportUnknownName { name: name.into() })
}

pub(super) fn name_collision(
    name: &str,
    primary: SourceOrigin,
    related: Option<SourceOrigin>,
) -> SourceDiagnostic {
    SourceDiagnostic::new(
        NameRule::NameCollision.code(),
        format!("name `{name}` is already visible in this scope"),
        primary,
        notes(related, "the existing name is introduced here"),
        Some("choose a distinct binding or import alias"),
    )
}

pub(super) fn missing_block_import(name: &str, primary: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        NameRule::MissingBlockImport.code(),
        format!("block import target does not export `{name}`"),
        primary,
        [],
        Some("import a name exported by the target module"),
    )
}

pub(super) fn inaccessible_block_import(name: &str, primary: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        NameRule::InaccessibleBlockImport.code(),
        format!("block import cannot access `{name}` from this module"),
        primary,
        [],
        Some("use a name whose visibility includes the importing module"),
    )
}

pub(super) fn non_type_selection(name: &str, primary: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        NameRule::NonTypeSelection.code(),
        format!("selected import `{name}` does not name a type or interface"),
        primary,
        [],
        Some("import the owning module namespace and access this name through that namespace"),
    )
}

pub(super) fn missing_module_member(name: &str, primary: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        NameRule::MissingModuleMember.code(),
        format!("module does not export `{name}`"),
        primary,
        [],
        Some("use a name exported by the selected module"),
    )
}

pub(super) fn inaccessible_module_member(name: &str, primary: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        NameRule::InaccessibleModuleMember.code(),
        format!("module member `{name}` is not visible from this module"),
        primary,
        [],
        Some("use a member whose visibility includes the current module"),
    )
}

pub(super) fn invalid_capture_target(name: &str, primary: SourceOrigin) -> SourceDiagnostic {
    SourceDiagnostic::new(
        NameRule::InvalidCaptureTarget.code(),
        format!("capture `{name}` is not an enclosing callable binding"),
        primary,
        [],
        Some("capture a local, parameter, or capture from the enclosing callable body"),
    )
}

pub(super) fn capture_collision(
    name: &str,
    primary: SourceOrigin,
    related: Option<SourceOrigin>,
) -> SourceDiagnostic {
    SourceDiagnostic::new(
        NameRule::CaptureCollision.code(),
        format!("closure name `{name}` conflicts with an explicit capture"),
        primary,
        notes(related, "the capture is introduced here"),
        Some("remove the duplicate capture or rename the closure parameter"),
    )
}

pub(super) fn implicit_capture(
    name: &str,
    primary: SourceOrigin,
    related: Option<SourceOrigin>,
) -> SourceDiagnostic {
    SourceDiagnostic::new(
        NameRule::ImplicitCapture.code(),
        format!("closure uses `{name}` without an explicit capture"),
        primary,
        notes(related, "the enclosing binding is introduced here"),
        Some(format!(
            "add `&{name}`, `&+{name}`, or `move {name}` to the closure capture list"
        )),
    )
}

fn notes(origin: Option<SourceOrigin>, message: &'static str) -> Vec<DiagnosticNote> {
    origin
        .map(|origin| vec![DiagnosticNote::new(message, origin)])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::NameRule;

    #[test]
    fn name_rule_codes_are_closed_and_unique() {
        let rules = [
            NameRule::UnknownName,
            NameRule::NameCollision,
            NameRule::MissingBlockImport,
            NameRule::InaccessibleBlockImport,
            NameRule::InvalidCaptureTarget,
            NameRule::CaptureCollision,
            NameRule::ImplicitCapture,
            NameRule::MissingModuleMember,
            NameRule::InaccessibleModuleMember,
            NameRule::NonTypeSelection,
        ];
        let codes: HashSet<_> = rules.into_iter().map(NameRule::code).collect();
        assert_eq!(codes.len(), rules.len());
        assert!(codes.iter().all(|code| code.as_str().starts_with('E')));
    }
}
