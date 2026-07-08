use super::{
    Diagnostic, IfIsStmt, PatternConditionalArm, PatternConditionalExpr, SourceMap, SwitchArm,
    SwitchStmt, Type, TypeSymbol, type_symbol_kind_name,
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
            "`{}.{}` has {} payload value(s), but the match arm provides {} binding(s)",
            enum_symbol.canonical_name, arm.variant_name, expected, actual
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.span).ok().map(Box::new);
    diagnostic.help = Some(
        "match the variant payload shape; v0 supports either no payload or one payload binding"
            .to_string(),
    );
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
    diagnostic.help = Some("use `if value is Enum.variant` with an enum value".to_string());
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
            "`{}.{}` has {} payload value(s), but the if-is pattern provides {} binding(s)",
            enum_symbol.canonical_name, statement.variant_name, expected, actual
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.pattern_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "match the variant payload shape; v0 supports either no payload or one payload binding"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn pattern_conditional_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &PatternConditionalExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0361",
        format!(
            "`?{{}}` target has type `{}`, but pattern conditional requires an enum value",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.target.span())
        .ok()
        .map(Box::new);
    diagnostic.help =
        Some("use `value ?{ Enum.variant : result : fallback }` with an enum value".to_string());
    diagnostic
}

pub(in crate::typecheck) fn pattern_conditional_arm_unknown_enum_diagnostic(
    sources: &SourceMap,
    arm: &PatternConditionalArm,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!("`?{{}}` arm refers to unknown enum `{}`", arm.enum_name),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help = Some("use a visible enum type in the arm pattern".to_string());
    diagnostic
}

pub(in crate::typecheck) fn pattern_conditional_arm_non_enum_diagnostic(
    sources: &SourceMap,
    arm: &PatternConditionalArm,
    symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!(
            "`?{{}}` arm refers to `{}`, but that type is `{}`",
            arm.enum_name,
            type_symbol_kind_name(symbol.kind)
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help = Some("use an enum type in the arm pattern".to_string());
    diagnostic
}

pub(in crate::typecheck) fn pattern_conditional_arm_enum_mismatch_diagnostic(
    sources: &SourceMap,
    arm: &PatternConditionalArm,
    expected: &TypeSymbol,
    actual: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0363",
        format!(
            "`?{{}}` arm uses enum `{}`, but the target enum is `{}`",
            actual.canonical_name, expected.canonical_name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help = Some("make every arm use the same enum type as the target".to_string());
    diagnostic
}

pub(in crate::typecheck) fn pattern_conditional_arm_unknown_variant_diagnostic(
    sources: &SourceMap,
    arm: &PatternConditionalArm,
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

pub(in crate::typecheck) fn pattern_conditional_arm_payload_mismatch_diagnostic(
    sources: &SourceMap,
    arm: &PatternConditionalArm,
    enum_symbol: &TypeSymbol,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0365",
        format!(
            "`{}.{}` has {} payload value(s), but the `?{{}}` arm provides {} binding(s)",
            enum_symbol.canonical_name, arm.variant_name, expected, actual
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.span).ok().map(Box::new);
    diagnostic.help = Some(
        "match the variant payload shape; v0 supports either no payload or one payload binding"
            .to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn pattern_conditional_arm_type_mismatch_diagnostic(
    sources: &SourceMap,
    arm: &PatternConditionalArm,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0366",
        format!(
            "`?{{}}` arm has type `{}`, but the fallback arm has type `{}`",
            actual.display(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(arm.expression.span())
        .ok()
        .map(Box::new);
    diagnostic.help =
        Some("make every pattern conditional arm produce the same value type".to_string());
    diagnostic
}
