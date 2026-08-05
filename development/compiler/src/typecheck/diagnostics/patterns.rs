use super::{
    Block, ByteSpan, Diagnostic, DiagnosticNote, IfIsStmt, SourceMap, SwitchArm, SwitchStmt, Type,
    TypeSymbol, type_symbol_kind_name,
};

pub(in crate::typecheck) fn switch_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &SwitchStmt,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0361",
        format!(
            "`match` target has type `{}`, but `match` requires an enum value",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.expression.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("match on a value whose type is an enum".to_string());
    diagnostic
}

pub(in crate::typecheck) fn switch_arm_unknown_enum_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!("`match` arm refers to unknown enum `{}`", arm.enum_name),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help = Some("use a visible enum type in the arm pattern".to_string());
    diagnostic
}

pub(in crate::typecheck) fn switch_arm_non_enum_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!(
            "`match` arm refers to `{}`, but that type is `{}`",
            arm.enum_name,
            type_symbol_kind_name(symbol.kind)
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help = Some("use an enum type in the arm pattern".to_string());
    diagnostic
}

pub(in crate::typecheck) fn switch_arm_enum_mismatch_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    expected: &TypeSymbol,
    actual: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0363",
        format!(
            "`match` arm uses enum `{}`, but the target enum is `{}`",
            actual.canonical_name, expected.canonical_name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help = Some("make every arm use the same enum type as the match target".to_string());
    diagnostic
}

pub(in crate::typecheck) fn switch_arm_unknown_variant_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    enum_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0364",
        format!(
            "enum `{}` has no variant `{}`",
            enum_symbol.canonical_name, arm.variant_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(arm.variant_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a variant declared by the enum".to_string());
    diagnostic
}

pub(in crate::typecheck) fn switch_arm_payload_mismatch_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    enum_symbol: &TypeSymbol,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0365",
        format!(
            "`{}.{}` has {} payload value(s), but the match arm provides {} payload pattern(s)",
            enum_symbol.canonical_name, arm.variant_name, expected, actual
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.span).ok().map(Box::new);
    diagnostic.help = Some(
        "match the variant payload shape; use either no payload or one payload pattern".to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn switch_arm_result_type_mismatch_diagnostic(
    sources: &SourceMap,
    body: &Block,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0366",
        format!(
            "`match` arm has type `{}`, but another arm has type `{}`",
            actual.display(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(block_result_span(body))
        .ok()
        .map(Box::new);
    diagnostic.help = Some("make every match arm produce the same value type".to_string());
    diagnostic
}

fn block_result_span(block: &Block) -> ByteSpan {
    block
        .result
        .as_ref()
        .map_or(block.span, |expression| expression.span())
}

pub(in crate::typecheck) fn duplicate_switch_arm_variant_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    enum_symbol: &TypeSymbol,
    first_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0398",
        format!(
            "`match` arm for `{}.{}` is duplicated",
            enum_symbol.canonical_name, arm.variant_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(arm.variant_name_span)
        .ok()
        .map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first arm for this variant is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("remove one of the duplicate match arms".to_string());
    diagnostic
}

pub(in crate::typecheck) fn if_is_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0361",
        format!(
            "`if is` target has type `{}`, but `if is` requires an enum value",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.expression.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use `value is Enum.variant` with an enum value".to_string());
    diagnostic
}

pub(in crate::typecheck) fn if_is_unknown_enum_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!(
            "`if is` pattern refers to unknown enum `{}`",
            statement.enum_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.enum_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a visible enum type in the pattern".to_string());
    diagnostic
}

pub(in crate::typecheck) fn if_is_non_enum_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!(
            "`if is` pattern refers to `{}`, but that type is `{}`",
            statement.enum_name,
            type_symbol_kind_name(symbol.kind)
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.enum_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use an enum type in the pattern".to_string());
    diagnostic
}

pub(in crate::typecheck) fn if_is_enum_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    expected: &TypeSymbol,
    actual: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0363",
        format!(
            "`if is` pattern uses enum `{}`, but the target enum is `{}`",
            actual.canonical_name, expected.canonical_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.enum_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("make the pattern use the same enum type as the target".to_string());
    diagnostic
}

pub(in crate::typecheck) fn if_is_unknown_variant_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    enum_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0364",
        format!(
            "enum `{}` has no variant `{}`",
            enum_symbol.canonical_name, statement.variant_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.variant_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a variant declared by the enum".to_string());
    diagnostic
}

pub(in crate::typecheck) fn if_is_payload_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    enum_symbol: &TypeSymbol,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0365",
        format!(
            "`{}.{}` has {} payload value(s), but the if-is pattern provides {} payload pattern(s)",
            enum_symbol.canonical_name, statement.variant_name, expected, actual
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.pattern_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "match the variant payload shape; use either no payload or one payload pattern".to_string(),
    );
    diagnostic
}
