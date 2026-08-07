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
            "name a storage-carrying parameter or `static`".to_string(),
        ),
        ContractOriginError::UnknownParameter(name) => (
            "E0440",
            format!("`{name}` is not a parameter of this callable"),
            "name a declared storage-carrying parameter or `static`".to_string(),
        ),
        ContractOriginError::NonStorageCarryingParameter(name) => (
            "E0441",
            format!("parameter `{name}` cannot carry result storage"),
            "only a storage-carrying parameter can be a result origin".to_string(),
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

pub(in crate::typecheck) fn missing_external_result_contract_diagnostic(
    sources: &SourceMap,
    return_span: ByteSpan,
    candidates: &[String],
) -> Diagnostic {
    let message = if candidates.len() > 1 {
        "the result retains one of multiple possible caller-managed origins"
    } else {
        "the result origin cannot be safely elided"
    };
    let mut diagnostic = Diagnostic::error("E0444", message);
    diagnostic.primary_span = sources.span_to_json(return_span).ok().map(Box::new);
    diagnostic.notes.push(DiagnosticNote {
        message: match candidates {
            [] => "no caller-managed input origin is inferable".to_string(),
            [candidate] => format!("the only inferred origin is {candidate}"),
            _ => format!("eligible origins are {}", candidates.join(", ")),
        },
        span: None,
    });
    diagnostic.help = Some(if candidates.is_empty() {
        "return only fresh, static, or otherwise storage-independent data".to_string()
    } else {
        format!(
            "add `from {}` with only the origins that the result may retain",
            candidates.join(" | ")
        )
    });
    diagnostic
}

pub(in crate::typecheck) fn ambiguous_bodyless_result_contract_diagnostic(
    sources: &SourceMap,
    return_span: ByteSpan,
    candidates: &[String],
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0446",
        "a bodyless result origin cannot be inferred from multiple inputs",
    );
    diagnostic.primary_span = sources.span_to_json(return_span).ok().map(Box::new);
    diagnostic.notes.push(DiagnosticNote {
        message: format!("eligible origins are {}", candidates.join(", ")),
        span: None,
    });
    diagnostic.help = Some(format!(
        "add `from {}` and remove origins that the implementation cannot retain",
        candidates.join(" | ")
    ));
    diagnostic
}
