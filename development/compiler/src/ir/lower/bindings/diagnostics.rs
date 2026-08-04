use super::*;

pub(super) fn unsupported_binding_diagnostic(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error("E8008", message)]
}

pub(super) fn unsupported_assignment_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "native lowering can only lower simple `=` assignment to scalar local bindings, supported read-write slice elements, scalar aggregate fields, aggregate slots, copy aggregate fields, or drop-aware aggregate field replacement",
    )]
}
