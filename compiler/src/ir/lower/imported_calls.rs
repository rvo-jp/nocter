use crate::ast::{
    Block, Expr, FunctionDecl, IfIsStmt, IfLetStmt, IfStmt, InterpolatedStringPart, Stmt,
    SwitchStmt, WhileLetStmt, WhileStmt,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::{ResolveOutput, Symbol, SymbolKind};
use crate::source::SourceId;

pub(super) fn imported_call_diagnostics(
    function: &FunctionDecl,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    collect_block(&function.body, root_source, resolved, &mut diagnostics);
    diagnostics
}

fn collect_block(
    block: &Block,
    root_source: SourceId,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        collect_statement(statement, root_source, resolved, diagnostics);
    }
}

fn collect_statement(
    statement: &Stmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression(expression, root_source, resolved, diagnostics);
            }
        }
        Stmt::Binding(statement) => {
            collect_expression(&statement.initializer, root_source, resolved, diagnostics);
        }
        Stmt::Assignment(statement) => {
            collect_expression(&statement.target, root_source, resolved, diagnostics);
            collect_expression(&statement.value, root_source, resolved, diagnostics);
        }
        Stmt::If(statement) => collect_if(statement, root_source, resolved, diagnostics),
        Stmt::IfIs(statement) => collect_if_is(statement, root_source, resolved, diagnostics),
        Stmt::IfLet(statement) => collect_if_let(statement, root_source, resolved, diagnostics),
        Stmt::Switch(statement) => collect_switch(statement, root_source, resolved, diagnostics),
        Stmt::ForRange(statement) => {
            collect_expression(&statement.start, root_source, resolved, diagnostics);
            collect_expression(&statement.end, root_source, resolved, diagnostics);
            collect_block(&statement.body, root_source, resolved, diagnostics);
        }
        Stmt::While(statement) => collect_while(statement, root_source, resolved, diagnostics),
        Stmt::WhileLet(statement) => {
            collect_while_let(statement, root_source, resolved, diagnostics);
        }
        Stmt::Loop(statement) => collect_block(&statement.body, root_source, resolved, diagnostics),
        Stmt::Expression(statement) => {
            collect_expression(&statement.expression, root_source, resolved, diagnostics);
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_if(
    statement: &IfStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_expression(&statement.condition, root_source, resolved, diagnostics);
    collect_block(&statement.then_block, root_source, resolved, diagnostics);
    if let Some(block) = &statement.else_block {
        collect_block(block, root_source, resolved, diagnostics);
    }
}

fn collect_if_is(
    statement: &IfIsStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_expression(&statement.expression, root_source, resolved, diagnostics);
    collect_block(&statement.then_block, root_source, resolved, diagnostics);
    if let Some(block) = &statement.else_block {
        collect_block(block, root_source, resolved, diagnostics);
    }
}

fn collect_if_let(
    statement: &IfLetStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_expression(&statement.initializer, root_source, resolved, diagnostics);
    collect_block(&statement.then_block, root_source, resolved, diagnostics);
    if let Some(block) = &statement.else_block {
        collect_block(block, root_source, resolved, diagnostics);
    }
}

fn collect_switch(
    statement: &SwitchStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_expression(&statement.expression, root_source, resolved, diagnostics);
    for arm in &statement.arms {
        collect_block(&arm.body, root_source, resolved, diagnostics);
    }
    if let Some(arm) = &statement.else_arm {
        collect_block(&arm.body, root_source, resolved, diagnostics);
    }
}

fn collect_while(
    statement: &WhileStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_expression(&statement.condition, root_source, resolved, diagnostics);
    collect_block(&statement.body, root_source, resolved, diagnostics);
}

fn collect_while_let(
    statement: &WhileLetStmt,
    root_source: SourceId,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_expression(&statement.initializer, root_source, resolved, diagnostics);
    collect_block(&statement.body, root_source, resolved, diagnostics);
}

fn collect_expression(
    expression: &Expr,
    root_source: SourceId,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expr::Call(call) => {
            if let Some(symbol) = resolved.symbol_for_call(call)
                && symbol_is_imported_call_target(symbol, root_source)
            {
                diagnostics.push(unsupported_imported_call_diagnostic(
                    &resolved.call_name_for_diagnostic(call),
                ));
            }
            collect_expression(&call.callee, root_source, resolved, diagnostics);
            for argument in &call.arguments {
                collect_expression(argument, root_source, resolved, diagnostics);
            }
        }
        Expr::Unary(expression) => {
            collect_expression(&expression.operand, root_source, resolved, diagnostics);
        }
        Expr::Binary(expression) => {
            collect_expression(&expression.left, root_source, resolved, diagnostics);
            collect_expression(&expression.right, root_source, resolved, diagnostics);
        }
        Expr::TypeConversion(expression) => {
            collect_expression(&expression.expression, root_source, resolved, diagnostics);
        }
        Expr::Propagate(expression) => {
            collect_expression(&expression.expression, root_source, resolved, diagnostics);
        }
        Expr::Force(expression) => {
            collect_expression(&expression.expression, root_source, resolved, diagnostics);
        }
        Expr::Catch(expression) => {
            collect_expression(&expression.expression, root_source, resolved, diagnostics);
            collect_block(&expression.catch_block, root_source, resolved, diagnostics);
        }
        Expr::Member(expression) => {
            collect_expression(&expression.object, root_source, resolved, diagnostics);
        }
        Expr::Index(expression) => {
            collect_expression(&expression.object, root_source, resolved, diagnostics);
            collect_expression(&expression.index, root_source, resolved, diagnostics);
        }
        Expr::Group(expression) => {
            collect_expression(&expression.expression, root_source, resolved, diagnostics);
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression(element, root_source, resolved, diagnostics);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression(&field.value, root_source, resolved, diagnostics);
            }
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    collect_expression(&part.expression, root_source, resolved, diagnostics);
                }
            }
        }
        Expr::OptionalDefault(expression) => {
            collect_expression(&expression.value, root_source, resolved, diagnostics);
            collect_expression(&expression.default, root_source, resolved, diagnostics);
        }
        Expr::PatternConditional(expression) => {
            collect_expression(&expression.target, root_source, resolved, diagnostics);
            for arm in &expression.arms {
                collect_expression(&arm.expression, root_source, resolved, diagnostics);
            }
            collect_expression(&expression.fallback, root_source, resolved, diagnostics);
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn symbol_is_imported_call_target(symbol: &Symbol, root_source: SourceId) -> bool {
    matches!(symbol.kind, SymbolKind::Imported(_)) || symbol.declaration_span.source != root_source
}

fn unsupported_imported_call_diagnostic(call_name: &str) -> Diagnostic {
    Diagnostic::error(
        "E8006",
        format!(
            "IR v0 cannot lower imported function call `{call_name}` yet; imported standard-library calls need explicit backend call-target lowering"
        ),
    )
}
