use super::{Diagnostic, DiagnosticNote, SourceMap};
use crate::ast::{ResultProvenanceClause, ResultProvenanceOrigin, TypeExpr};
use crate::source::ByteSpan;
use crate::typecheck::provenance::ContractOriginError;

pub(in crate::typecheck) fn invalid_provenance_origin_diagnostic(
    sources: &SourceMap,
    origin: &ResultProvenanceOrigin,
    error: &ContractOriginError,
) -> Diagnostic {
    let (code, message, help) = match error {
        ContractOriginError::ReceiverOutsideMethod => (
            "E0440",
            "`self` is only a result origin on methods".to_string(),
            "name a borrowed parameter, `static`, or `current`".to_string(),
        ),
        ContractOriginError::OwnedReceiver => (
            "E0441",
            "an owned method receiver cannot be a borrowed result origin".to_string(),
            "borrow the receiver with `&self` or `&+self`, or choose another origin".to_string(),
        ),
        ContractOriginError::UnknownParameter(name) => (
            "E0440",
            format!("`{name}` is not a parameter of this callable"),
            "name a declared borrowed parameter, `static`, or `current`".to_string(),
        ),
        ContractOriginError::NonBorrowLikeParameter(name) => (
            "E0441",
            format!("parameter `{name}` cannot carry borrowed result storage"),
            "only a borrowed or storage-carrying parameter can be a result origin".to_string(),
        ),
        ContractOriginError::Duplicate(name) => (
            "E0442",
            format!("result origin `{name}` is listed more than once"),
            "list each result origin once".to_string(),
        ),
    };
    let mut diagnostic = Diagnostic::error(code, message);
    diagnostic.primary_span = sources.span_to_json(origin.span).ok().map(Box::new);
    diagnostic.help = Some(help);
    diagnostic
}

pub(in crate::typecheck) fn independent_result_contract_diagnostic(
    sources: &SourceMap,
    clause: &ResultProvenanceClause,
    return_type: &TypeExpr,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0443",
        "a storage-independent result cannot declare a provenance contract",
    );
    diagnostic.primary_span = sources.span_to_json(clause.span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(return_type.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: "this return type carries no tracked storage".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("remove the `from` clause".to_string());
    diagnostic
}

pub(in crate::typecheck) fn missing_result_contract_diagnostic(
    sources: &SourceMap,
    return_span: ByteSpan,
    eligible_origins: usize,
) -> Diagnostic {
    let detail = if eligible_origins == 0 {
        "the result has no input origin that can be inferred"
    } else {
        "the result could originate from more than one input"
    };
    let mut diagnostic = Diagnostic::error(
        "E0444",
        format!(
            "a bodyless storage-carrying callable needs an explicit result provenance contract: {detail}"
        ),
    );
    diagnostic.primary_span = sources.span_to_json(return_span).ok().map(Box::new);
    diagnostic.help = Some(
        "add `from self`, `from parameter`, `from static`, or `from current` after the return type"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn result_contract_violation_diagnostic(
    sources: &SourceMap,
    body_span: ByteSpan,
    clause: &ResultProvenanceClause,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0445",
        "the callable body can return storage outside its declared provenance contract",
    );
    diagnostic.primary_span = sources.span_to_json(body_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(clause.span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the public result provenance contract is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(
        "return only values covered by the `from` clause, or widen the declared contract"
            .to_string(),
    );
    diagnostic
}
