use super::{Diagnostic, DiagnosticNote, SourceMap};
use crate::source::ByteSpan;

pub(in crate::typecheck) fn missing_result_allocation_contract_diagnostic(
    sources: &SourceMap,
    declaration_span: ByteSpan,
    witness_span: Option<ByteSpan>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0462",
        "the callable can return newly allocated storage but is not marked `alloc`",
    );
    diagnostic.primary_span = sources.span_to_json(declaration_span).ok().map(Box::new);
    if let Some(witness_span) = witness_span
        && let Ok(span) = sources.span_to_json(witness_span)
    {
        diagnostic.notes.push(DiagnosticNote {
            message: "this returned expression retains newly allocated storage".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("add `alloc` before the callable keyword".to_string());
    diagnostic
}

pub(in crate::typecheck) fn unjustified_result_allocation_contract_diagnostic(
    sources: &SourceMap,
    modifier_span: ByteSpan,
    body_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0463",
        "the `alloc` contract is not justified by any returned allocation",
    );
    diagnostic.primary_span = sources.span_to_json(modifier_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(body_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "this body does not return newly allocated storage".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("remove `alloc` from this callable".to_string());
    diagnostic
}

pub(in crate::typecheck) fn incompatible_trusted_result_allocation_contract_diagnostic(
    sources: &SourceMap,
    modifier_span: ByteSpan,
    declaration_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0463",
        "the `alloc` contract contradicts this trusted primitive's semantic role",
    );
    diagnostic.primary_span = sources.span_to_json(modifier_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(declaration_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the compiler-defined operation does not return newly allocated storage"
                .to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("remove `alloc` from this primitive".to_string());
    diagnostic
}
