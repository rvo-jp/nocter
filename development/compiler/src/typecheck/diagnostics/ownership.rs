use super::{ByteSpan, Diagnostic, DiagnosticNote, SourceMap, Type};

pub(in crate::typecheck) fn uninitialized_binding_diagnostic(
    sources: &SourceMap,
    name: &str,
    use_span: ByteSpan,
    action: &str,
    previous_action: &str,
    previous_span: ByteSpan,
) -> Diagnostic {
    let maybe_uninitialized = previous_action == "maybe uninitialized";
    let uninitialized = previous_action == "uninitialized";
    let message = if maybe_uninitialized {
        format!("cannot {action} `{name}` because it may be uninitialized")
    } else if uninitialized {
        format!("cannot {action} `{name}` because it is uninitialized")
    } else {
        format!("cannot {action} `{name}` because it was {previous_action}")
    };
    let mut diagnostic = Diagnostic::error("E0385", message);
    diagnostic.primary_span = sources.span_to_json(use_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(previous_span) {
        let note_message = if maybe_uninitialized {
            format!("`{name}` may have been moved or dropped on an earlier path")
        } else if uninitialized {
            format!("`{name}` was moved or dropped on all incoming paths")
        } else {
            format!("`{name}` was {previous_action} here")
        };
        diagnostic.notes.push(DiagnosticNote {
            message: note_message,
            span: Some(span),
        });
    }
    diagnostic.help = Some(
        "move or drop each owned value at most once, or reinitialize a `var` binding before using it again"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn invalid_drop_target_diagnostic(
    sources: &SourceMap,
    name: &str,
    name_span: ByteSpan,
    ty: Option<&Type>,
) -> Diagnostic {
    let message = match ty {
        Some(ty) => format!(
            "cannot explicitly drop `{name}` with type `{}` because it is not a move-only owned binding",
            ty.display()
        ),
        None => format!("cannot explicitly drop unknown binding `{name}`"),
    };
    let mut diagnostic = Diagnostic::error("E0386", message);
    diagnostic.primary_span = sources.span_to_json(name_span).ok().map(Box::new);
    diagnostic.help = Some(
        "`drop name` accepts initialized move-only owned bindings; copy values and plain borrow bindings are not explicitly dropped"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn active_borrow_conflict_diagnostic(
    sources: &SourceMap,
    source_name: &str,
    action: &str,
    action_span: ByteSpan,
    borrow_name: &str,
    borrow_span: ByteSpan,
    is_readwrite: bool,
) -> Diagnostic {
    let borrow_kind = if is_readwrite {
        "readwrite"
    } else {
        "readonly"
    };
    let mut diagnostic = Diagnostic::error(
        "E0434",
        format!("cannot {action} `{source_name}` while it is borrowed by `{borrow_name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(action_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(borrow_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("{borrow_kind} borrow `{borrow_name}` is created here"),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "use `{borrow_name}` before this operation, or move this operation after the borrow's last use"
    ));
    diagnostic
}

pub(in crate::typecheck) fn overlapping_expression_borrow_diagnostic(
    sources: &SourceMap,
    source_name: &str,
    later: &str,
    later_span: ByteSpan,
    earlier_span: ByteSpan,
    earlier_is_readwrite: bool,
) -> Diagnostic {
    let earlier_kind = if earlier_is_readwrite {
        "readwrite"
    } else {
        "readonly"
    };
    let mut diagnostic = Diagnostic::error(
        "E0434",
        format!(
            "cannot {later} `{source_name}` while an earlier operand in the same expression holds a borrow"
        ),
    );
    diagnostic.primary_span = sources.span_to_json(later_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(earlier_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("{earlier_kind} borrow is created by this earlier operand"),
            span: Some(span),
        });
    }
    diagnostic.help = Some(
        "finish the earlier borrow before this operand, or use compatible readonly borrows"
            .to_string(),
    );
    diagnostic
}
