use crate::ast::{
    Block, Expr, FunctionDecl, IfIsStmt, IfLetStmt, IfStmt, InterpolatedStringPart, Stmt,
    SwitchStmt, WhileLetStmt, WhileStmt,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::{ResolveOutput, Symbol, SymbolKind};
use crate::source::{ByteSpan, SourceId, SourceMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ImportedCallTarget {
    pub(super) call_name: String,
    pub(super) span: ByteSpan,
    pub(super) source: ImportedCallSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ImportedCallSource {
    Loaded(SourceId),
    UnloadedPath(String),
}

pub(super) fn imported_call_diagnostics(
    sources: &SourceMap,
    function: &FunctionDecl,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Vec<Diagnostic> {
    imported_call_diagnostics_for_block(sources, &function.body, root_source, resolved)
}

pub(super) fn imported_call_diagnostics_for_block(
    sources: &SourceMap,
    block: &Block,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Vec<Diagnostic> {
    imported_call_targets_for_block(block, root_source, resolved)
        .into_iter()
        .filter(|target| matches!(target.source, ImportedCallSource::UnloadedPath(_)))
        .map(|target| {
            unsupported_imported_call_diagnostic(&target.call_name)
                .with_primary_span_if_absent(sources, target.span)
        })
        .collect()
}

#[cfg(test)]
pub(super) fn imported_call_targets(
    function: &FunctionDecl,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Vec<ImportedCallTarget> {
    imported_call_targets_for_block(&function.body, root_source, resolved)
}

fn imported_call_targets_for_block(
    block: &Block,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Vec<ImportedCallTarget> {
    let mut targets = Vec::new();
    collect_block(block, root_source, resolved, &mut targets);
    targets
}

fn collect_block(
    block: &Block,
    root_source: SourceId,
    resolved: &ResolveOutput,
    targets: &mut Vec<ImportedCallTarget>,
) {
    for statement in &block.statements {
        collect_statement(statement, root_source, resolved, targets);
    }
}

fn collect_statement(
    statement: &Stmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    targets: &mut Vec<ImportedCallTarget>,
) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression(expression, root_source, resolved, targets);
            }
        }
        Stmt::Binding(statement) => {
            collect_expression(&statement.initializer, root_source, resolved, targets);
        }
        Stmt::Assignment(statement) => {
            collect_expression(&statement.target, root_source, resolved, targets);
            collect_expression(&statement.value, root_source, resolved, targets);
        }
        Stmt::If(statement) => collect_if(statement, root_source, resolved, targets),
        Stmt::IfIs(statement) => collect_if_is(statement, root_source, resolved, targets),
        Stmt::IfLet(statement) => collect_if_let(statement, root_source, resolved, targets),
        Stmt::Switch(statement) => collect_switch(statement, root_source, resolved, targets),
        Stmt::ForRange(statement) => {
            collect_expression(&statement.start, root_source, resolved, targets);
            collect_expression(&statement.end, root_source, resolved, targets);
            collect_block(&statement.body, root_source, resolved, targets);
        }
        Stmt::While(statement) => collect_while(statement, root_source, resolved, targets),
        Stmt::WhileLet(statement) => {
            collect_while_let(statement, root_source, resolved, targets);
        }
        Stmt::Loop(statement) => collect_block(&statement.body, root_source, resolved, targets),
        Stmt::Expression(statement) => {
            collect_expression(&statement.expression, root_source, resolved, targets);
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
    }
}

fn collect_if(
    statement: &IfStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    targets: &mut Vec<ImportedCallTarget>,
) {
    collect_expression(&statement.condition, root_source, resolved, targets);
    collect_block(&statement.then_block, root_source, resolved, targets);
    if let Some(block) = &statement.else_block {
        collect_block(block, root_source, resolved, targets);
    }
}

fn collect_if_is(
    statement: &IfIsStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    targets: &mut Vec<ImportedCallTarget>,
) {
    collect_expression(&statement.expression, root_source, resolved, targets);
    collect_block(&statement.then_block, root_source, resolved, targets);
    if let Some(block) = &statement.else_block {
        collect_block(block, root_source, resolved, targets);
    }
}

fn collect_if_let(
    statement: &IfLetStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    targets: &mut Vec<ImportedCallTarget>,
) {
    collect_expression(&statement.initializer, root_source, resolved, targets);
    collect_block(&statement.then_block, root_source, resolved, targets);
    if let Some(block) = &statement.else_block {
        collect_block(block, root_source, resolved, targets);
    }
}

fn collect_switch(
    statement: &SwitchStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    targets: &mut Vec<ImportedCallTarget>,
) {
    collect_expression(&statement.expression, root_source, resolved, targets);
    for arm in &statement.arms {
        collect_block(&arm.body, root_source, resolved, targets);
    }
    if let Some(arm) = &statement.else_arm {
        collect_block(&arm.body, root_source, resolved, targets);
    }
}

fn collect_while(
    statement: &WhileStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    targets: &mut Vec<ImportedCallTarget>,
) {
    collect_expression(&statement.condition, root_source, resolved, targets);
    collect_block(&statement.body, root_source, resolved, targets);
}

fn collect_while_let(
    statement: &WhileLetStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    targets: &mut Vec<ImportedCallTarget>,
) {
    collect_expression(&statement.initializer, root_source, resolved, targets);
    collect_block(&statement.body, root_source, resolved, targets);
}

fn collect_expression(
    expression: &Expr,
    root_source: SourceId,
    resolved: &ResolveOutput,
    targets: &mut Vec<ImportedCallTarget>,
) {
    match expression {
        Expr::Call(call) => {
            if let Some(symbol) = resolved.symbol_for_call(call)
                && let Some(target) = imported_call_target_for_symbol(
                    symbol,
                    root_source,
                    resolved.call_name_for_diagnostic(call),
                    call.span,
                )
            {
                targets.push(target);
            }
            collect_expression(&call.callee, root_source, resolved, targets);
            for argument in &call.arguments {
                collect_expression(argument, root_source, resolved, targets);
            }
        }
        Expr::Unary(expression) => {
            collect_expression(&expression.operand, root_source, resolved, targets);
        }
        Expr::Binary(expression) => {
            collect_expression(&expression.left, root_source, resolved, targets);
            collect_expression(&expression.right, root_source, resolved, targets);
        }
        Expr::TypeConversion(expression) => {
            collect_expression(&expression.expression, root_source, resolved, targets);
        }
        Expr::Propagate(expression) => {
            collect_expression(&expression.expression, root_source, resolved, targets);
        }
        Expr::Force(expression) => {
            collect_expression(&expression.expression, root_source, resolved, targets);
        }
        Expr::Catch(expression) => {
            collect_expression(&expression.expression, root_source, resolved, targets);
            collect_block(&expression.catch_block, root_source, resolved, targets);
        }
        Expr::Borrow(expression) => {
            collect_expression(&expression.expression, root_source, resolved, targets);
        }
        Expr::Member(expression) => {
            collect_expression(&expression.object, root_source, resolved, targets);
        }
        Expr::Index(expression) => {
            collect_expression(&expression.object, root_source, resolved, targets);
            collect_expression(&expression.index, root_source, resolved, targets);
        }
        Expr::Group(expression) => {
            collect_expression(&expression.expression, root_source, resolved, targets);
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression(element, root_source, resolved, targets);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression(&field.value, root_source, resolved, targets);
            }
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    collect_expression(&part.expression, root_source, resolved, targets);
                }
            }
        }
        Expr::OptionalDefault(expression) => {
            collect_expression(&expression.value, root_source, resolved, targets);
            collect_expression(&expression.default, root_source, resolved, targets);
        }
        Expr::PatternConditional(expression) => {
            collect_expression(&expression.target, root_source, resolved, targets);
            for arm in &expression.arms {
                collect_expression(&arm.expression, root_source, resolved, targets);
            }
            collect_expression(&expression.fallback, root_source, resolved, targets);
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn imported_call_target_for_symbol(
    symbol: &Symbol,
    root_source: SourceId,
    call_name: String,
    span: ByteSpan,
) -> Option<ImportedCallTarget> {
    match &symbol.kind {
        SymbolKind::Imported(imported) => Some(ImportedCallTarget {
            call_name,
            span,
            source: ImportedCallSource::UnloadedPath(imported.path.clone()),
        }),
        SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_)
            if symbol.declaration_span.source != root_source =>
        {
            Some(ImportedCallTarget {
                call_name,
                span,
                source: ImportedCallSource::Loaded(symbol.declaration_span.source),
            })
        }
        SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_) => None,
    }
}

fn unsupported_imported_call_diagnostic(call_name: &str) -> Diagnostic {
    Diagnostic::error(
        "E8006",
        format!(
            "IR v0 cannot lower unresolved imported function call `{call_name}`; the imported declaration must be loaded before backend call-target lowering"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{ImportedSymbol, SymbolId};
    use crate::source::{ByteSpan, SourceMap};

    #[test]
    fn imported_placeholder_symbol_becomes_unloaded_imported_call_target() {
        let root_source = SourceId::new(0);
        let symbol = Symbol {
            id: SymbolId::new(0),
            name: "print".to_string(),
            name_span: ByteSpan::new(root_source, 0, 5),
            declaration_span: ByteSpan::new(root_source, 0, 20),
            kind: SymbolKind::Imported(ImportedSymbol {
                path: "std/io".to_string(),
            }),
        };

        let call_span = ByteSpan::new(root_source, 30, 37);
        let target =
            imported_call_target_for_symbol(&symbol, root_source, "print".to_string(), call_span)
                .unwrap();

        assert_eq!(
            target,
            ImportedCallTarget {
                call_name: "print".to_string(),
                span: call_span,
                source: ImportedCallSource::UnloadedPath("std/io".to_string()),
            }
        );
    }

    #[test]
    fn unsupported_imported_call_diagnostic_can_use_call_span() {
        let mut sources = SourceMap::new();
        let source = sources.add_source("app.nct", None, "print()\n");
        let span = ByteSpan::new(source, 0, 7);

        let diagnostic = unsupported_imported_call_diagnostic("print")
            .with_primary_span_if_absent(&sources, span);

        assert_eq!(diagnostic.code, "E8006");
        assert!(diagnostic.primary_span.is_some());
    }
}
