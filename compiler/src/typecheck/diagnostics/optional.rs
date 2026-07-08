use super::{
    BindingStmt, Block, Diagnostic, IfLetStmt, SourceMap, Type, WhileLetStmt, binding_keyword,
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
    diagnostic.help = Some(
        "end the `else` block with `return` in parser/check v0; later phases will add `break`, `continue`, and `never` support"
            .to_string(),
    );
    diagnostic
}
