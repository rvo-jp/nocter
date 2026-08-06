//! Declaration-level contracts for native source tests.

use crate::ast::{AstFile, Item};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

pub(super) fn check_test_declarations(
    sources: &SourceMap,
    ast: &AstFile,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut names: HashMap<&str, ByteSpan> = HashMap::new();
    for item in &ast.items {
        let Item::Test(test) = item else {
            continue;
        };
        let Some(first) = names.insert(&test.name, test.name_span) else {
            continue;
        };
        let mut diagnostic = Diagnostic::error(
            "E0400",
            format!("test `{}` is already declared in this module", test.name),
        )
        .with_primary_span_if_absent(sources, test.name_span);
        if let Ok(span) = sources.span_to_json(first) {
            diagnostic.notes.push(DiagnosticNote {
                message: "first test declaration is here".to_string(),
                span: Some(span),
            });
        }
        diagnostics.push(diagnostic);
    }
}
