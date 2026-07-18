use super::{
    BindingStmt, Block, Diagnostic, IfLetStmt, OptionalDefaultExpr, SourceMap, Type, WhileLetStmt,
    binding_keyword,
};

pub(in crate::typecheck) fn optional_if_let_non_optional_diagnostic(
    sources: &SourceMap,
    statement: &IfLetStmt,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0356",
        format!(
            "`if {keyword}` requires an optional initializer, but the initializer has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "use an initializer whose type is `T?`, or use a regular `if` condition instead of `if {keyword}`"
    ));
    diagnostic
}

pub(in crate::typecheck) fn optional_while_let_non_optional_diagnostic(
    sources: &SourceMap,
    statement: &WhileLetStmt,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0358",
        format!(
            "`while {keyword}` requires an optional initializer, but the initializer has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "use an initializer whose type is `T?`, or use a regular `while` condition instead of `while {keyword}`"
    ));
    diagnostic
}

pub(in crate::typecheck) fn optional_let_else_non_optional_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0340",
        format!(
            "`{keyword} ... else` requires an optional initializer, but the initializer has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "remove `else`, or use an initializer whose type is `T?` for `{keyword} ... else`"
    ));
    diagnostic
}

pub(in crate::typecheck) fn optional_let_else_fallthrough_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    else_block: &Block,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0341",
        format!("`{keyword} ... else` requires an `else` block that cannot fall through"),
    );
    diagnostic.primary_span = sources.span_to_json(else_block.span).ok().map(Box::new);
    diagnostic.help =
        Some("end the `else` block with `return` or a call returning `never`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn optional_let_else_projection_binding_kind_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    is_readwrite: bool,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let construct = format!("{keyword} ... else");
    let mut diagnostic = Diagnostic::error(
        "E0429",
        optional_projection_binding_kind_message(&construct, is_readwrite),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(optional_projection_binding_kind_help(is_readwrite));
    diagnostic
}

pub(in crate::typecheck) fn optional_if_let_projection_binding_kind_diagnostic(
    sources: &SourceMap,
    statement: &IfLetStmt,
    is_readwrite: bool,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let construct = format!("if {keyword}");
    let mut diagnostic = Diagnostic::error(
        "E0429",
        optional_projection_binding_kind_message(&construct, is_readwrite),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(optional_projection_binding_kind_help(is_readwrite));
    diagnostic
}

pub(in crate::typecheck) fn optional_while_let_projection_deferred_diagnostic(
    sources: &SourceMap,
    statement: &WhileLetStmt,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0430",
        format!("borrowed optional projections in `while {keyword}` are deferred after v0"),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help =
        Some("use `while let` with an expression that produces `T?` directly".to_string());
    diagnostic
}

fn optional_projection_binding_kind_message(construct: &str, is_readwrite: bool) -> String {
    let projection = if is_readwrite {
        "readwrite"
    } else {
        "readonly"
    };
    format!("`{construct}` cannot bind a {projection} optional projection in v0")
}

fn optional_projection_binding_kind_help(is_readwrite: bool) -> String {
    if is_readwrite {
        "use `var name = &+place` for a readwrite projection".to_string()
    } else {
        "use `let name = &place` for a readonly projection".to_string()
    }
}

pub(in crate::typecheck) fn optional_default_non_optional_diagnostic(
    sources: &SourceMap,
    expression: &OptionalDefaultExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0396",
        format!(
            "`??` requires an optional left operand, but the left operand has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.value.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a value whose type is `T?` on the left side of `??`".to_string());
    diagnostic
}

pub(in crate::typecheck) fn optional_default_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &OptionalDefaultExpr,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0397",
        format!(
            "`??` fallback has type `{}`, but the optional payload has type `{}`",
            actual.display(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.default.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "change the fallback expression to produce `{}`",
        expected.display()
    ));
    diagnostic
}
