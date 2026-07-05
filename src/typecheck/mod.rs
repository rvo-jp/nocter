//! Type checking, ownership, borrowing, move, and drop checks.

use crate::ast::{
    AstFile, Block, Expr, FunctionDecl, Item, ProgramDecl, ReturnStmt, Stmt, TypeExpr,
};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::source::{ByteSpan, SourceMap};

pub fn check(sources: &SourceMap, ast: &AstFile) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_program_entry(sources, ast, &mut diagnostics);
    check_return_types(sources, ast, &mut diagnostics);

    diagnostics
}

fn check_program_entry(sources: &SourceMap, ast: &AstFile, diagnostics: &mut Vec<Diagnostic>) {
    let programs: Vec<&ProgramDecl> = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Program(program) => Some(program),
            _ => None,
        })
        .collect();

    match programs.as_slice() {
        [] => {
            if let Some(main) = find_main_function(ast) {
                diagnostics.push(main_is_not_entry_diagnostic(sources, main));
            } else {
                diagnostics.push(missing_program_diagnostic(sources, ast.span));
            }
        }
        [program] => {
            if !is_valid_program_return_type(&program.return_type) {
                diagnostics.push(invalid_program_return_type_diagnostic(
                    sources,
                    program.return_type.span(),
                ));
            }
        }
        [first, second, ..] => {
            diagnostics.push(duplicate_program_diagnostic(
                sources,
                first.span,
                second.span,
            ));
        }
    }
}

fn find_main_function(ast: &AstFile) -> Option<&FunctionDecl> {
    ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name == "main" => Some(function),
        _ => None,
    })
}

fn is_valid_program_return_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Reference(reference) if reference.name == "i32" || reference.name == "void")
}

fn check_return_types(sources: &SourceMap, ast: &AstFile, diagnostics: &mut Vec<Diagnostic>) {
    for item in &ast.items {
        match item {
            Item::Program(program) if is_valid_program_return_type(&program.return_type) => {
                let context = ReturnContext::new(
                    CallableKind::Program,
                    type_expr_to_type(&program.return_type),
                    program.return_type.span(),
                );
                check_block_returns(sources, &program.body, &context, diagnostics);
            }
            Item::Function(function) => {
                let context = ReturnContext::new(
                    CallableKind::Function(function.name.clone()),
                    type_expr_to_type(&function.return_type),
                    function.return_type.span(),
                );
                check_block_returns(sources, &function.body, &context, diagnostics);
            }
            _ => {}
        }
    }
}

fn check_block_returns(
    sources: &SourceMap,
    block: &Block,
    context: &ReturnContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        check_statement_returns(sources, statement, context, diagnostics);
    }

    if context.requires_explicit_return() && !block_guarantees_return(block) {
        diagnostics.push(missing_return_diagnostic(sources, block.span, context));
    }
}

fn check_statement_returns(
    sources: &SourceMap,
    statement: &Stmt,
    context: &ReturnContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Stmt::Return(statement) => check_return_statement(sources, statement, context, diagnostics),
        Stmt::Binding(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.initializer,
                context,
                diagnostics,
            );
        }
        Stmt::Try(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                diagnostics,
            );
        }
        Stmt::TryCatch(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                diagnostics,
            );
            for catch_statement in &statement.catch_block.statements {
                check_statement_returns(sources, catch_statement, context, diagnostics);
            }
        }
        Stmt::Expression(statement) => {
            check_expression_for_nested_returns(
                sources,
                &statement.expression,
                context,
                diagnostics,
            );
        }
    }
}

fn check_expression_for_nested_returns(
    sources: &SourceMap,
    expression: &Expr,
    context: &ReturnContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expr::Try(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                diagnostics,
            );
        }
        Expr::TryCatch(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                diagnostics,
            );
            for statement in &expression.catch_block.statements {
                check_statement_returns(sources, statement, context, diagnostics);
            }
        }
        Expr::Call(expression) => {
            check_expression_for_nested_returns(sources, &expression.callee, context, diagnostics);
            for argument in &expression.arguments {
                check_expression_for_nested_returns(sources, argument, context, diagnostics);
            }
        }
        Expr::Member(expression) => {
            check_expression_for_nested_returns(sources, &expression.object, context, diagnostics);
        }
        Expr::Group(expression) => {
            check_expression_for_nested_returns(
                sources,
                &expression.expression,
                context,
                diagnostics,
            );
        }
        Expr::OptionalDefault(expression) => {
            check_expression_for_nested_returns(sources, &expression.value, context, diagnostics);
            check_expression_for_nested_returns(sources, &expression.default, context, diagnostics);
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn check_return_statement(
    sources: &SourceMap,
    statement: &ReturnStmt,
    context: &ReturnContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expected = context.success_type();

    match (&statement.expression, expected) {
        (None, Type::Void) => {}
        (None, Type::Unknown) | (None, Type::Named(_)) => {}
        (None, _) => diagnostics.push(missing_return_value_diagnostic(sources, statement, context)),
        (Some(expression), Type::Void) => {
            diagnostics.push(unexpected_return_value_diagnostic(
                sources, expression, context,
            ));
        }
        (Some(expression), expected) => {
            let actual = expression_type(expression);
            if actual.is_unknown() || expected.is_unknown_or_unresolved() {
                return;
            }

            if !is_assignable(expected, &actual) {
                diagnostics.push(return_type_mismatch_diagnostic(
                    sources, expression, expected, &actual, context,
                ));
            }
        }
    }
}

fn block_guarantees_return(block: &Block) -> bool {
    block
        .statements
        .last()
        .is_some_and(statement_guarantees_return)
}

fn statement_guarantees_return(statement: &Stmt) -> bool {
    matches!(statement, Stmt::Return(_))
}

fn type_expr_to_type(ty: &TypeExpr) -> Type {
    match ty {
        TypeExpr::Reference(reference) => match reference.name.as_str() {
            "i32" => Type::I32,
            "void" => Type::Void,
            "never" => Type::Never,
            "StringView" => Type::StringView,
            name => Type::Named(name.to_string()),
        },
        TypeExpr::Optional(ty) => Type::Optional(Box::new(type_expr_to_type(&ty.inner))),
        TypeExpr::Fallible(ty) => Type::Fallible {
            success: Box::new(type_expr_to_type(&ty.success)),
            error: Box::new(type_expr_to_type(&ty.error)),
        },
    }
}

fn expression_type(expression: &Expr) -> Type {
    match expression {
        Expr::IntegerLiteral(_) => Type::I32,
        Expr::StringLiteral(_) => Type::StringView,
        Expr::NoneLiteral(_) => Type::None,
        Expr::Group(expression) => expression_type(&expression.expression),
        Expr::OptionalDefault(expression) => expression_type(&expression.default),
        Expr::Identifier(_)
        | Expr::Try(_)
        | Expr::TryCatch(_)
        | Expr::Call(_)
        | Expr::Member(_) => Type::Unknown,
    }
}

fn is_assignable(expected: &Type, actual: &Type) -> bool {
    if actual == &Type::Never {
        return true;
    }

    match (expected, actual) {
        (Type::Optional(_), Type::None) => true,
        (Type::Optional(inner), actual) => is_assignable(inner, actual),
        _ => expected == actual,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Type {
    I32,
    Void,
    Never,
    StringView,
    None,
    Optional(Box<Type>),
    Fallible {
        success: Box<Type>,
        error: Box<Type>,
    },
    Named(String),
    Unknown,
}

impl Type {
    fn display(&self) -> String {
        match self {
            Type::I32 => "i32".to_string(),
            Type::Void => "void".to_string(),
            Type::Never => "never".to_string(),
            Type::StringView => "StringView".to_string(),
            Type::None => "none".to_string(),
            Type::Optional(inner) => format!("{}?", inner.display()),
            Type::Fallible { success, error } => {
                format!("{} ! {}", success.display(), error.display())
            }
            Type::Named(name) => name.clone(),
            Type::Unknown => "<unknown>".to_string(),
        }
    }

    fn is_unknown(&self) -> bool {
        matches!(self, Type::Unknown)
    }

    fn is_unknown_or_unresolved(&self) -> bool {
        matches!(self, Type::Unknown | Type::Named(_))
    }

    fn success_type(&self) -> &Type {
        match self {
            Type::Fallible { success, .. } => success,
            _ => self,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReturnContext {
    kind: CallableKind,
    declared_type: Type,
    return_type_span: ByteSpan,
}

impl ReturnContext {
    fn new(kind: CallableKind, declared_type: Type, return_type_span: ByteSpan) -> Self {
        Self {
            kind,
            declared_type,
            return_type_span,
        }
    }

    fn success_type(&self) -> &Type {
        self.declared_type.success_type()
    }

    fn requires_explicit_return(&self) -> bool {
        let success_type = self.success_type();
        !matches!(success_type, Type::Void | Type::Unknown | Type::Named(_))
    }

    fn subject(&self) -> String {
        match &self.kind {
            CallableKind::Program => "`program`".to_string(),
            CallableKind::Function(name) => format!("function `{name}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CallableKind {
    Program,
    Function(String),
}

fn missing_program_diagnostic(sources: &SourceMap, span: ByteSpan) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0300",
        "executable root file must define exactly one `program` entry",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("add `program(): i32 { ... }` or `program(): void { ... }`".to_string());
    diagnostic
}

fn main_is_not_entry_diagnostic(sources: &SourceMap, function: &FunctionDecl) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0301",
        "`func main` is an ordinary function; Nocter executable entry uses `program`",
    );
    diagnostic.primary_span = sources.span_to_json(function.name_span).ok().map(Box::new);
    diagnostic.help = Some(
        "replace the entry declaration with `program(): i32 { ... }` or `program(): void { ... }`"
            .to_string(),
    );
    diagnostic
}

fn duplicate_program_diagnostic(
    sources: &SourceMap,
    first_span: ByteSpan,
    second_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0302",
        "executable root file must not define more than one `program` entry",
    );
    diagnostic.primary_span = sources.span_to_json(second_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first `program` entry is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("keep exactly one top-level `program` declaration".to_string());
    diagnostic
}

fn invalid_program_return_type_diagnostic(
    sources: &SourceMap,
    return_type_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0303",
        "`program` return type must be `i32` or `void` in v0",
    );
    diagnostic.primary_span = sources.span_to_json(return_type_span).ok().map(Box::new);
    diagnostic.help = Some(
        "use `program(): i32` for an exit status or `program(): void` for status 0".to_string(),
    );
    diagnostic
}

fn missing_return_value_diagnostic(
    sources: &SourceMap,
    statement: &ReturnStmt,
    context: &ReturnContext,
) -> Diagnostic {
    let expected = context.success_type();
    let mut diagnostic = Diagnostic::error(
        "E0310",
        format!(
            "`return` has no value, but {} returns `{}`",
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(statement.span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!("return a value of type `{}`", expected.display()));
    diagnostic
}

fn unexpected_return_value_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0311",
        format!(
            "`return` has a value, but {} returns `void`",
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.span()).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("remove the returned value or change the return type".to_string());
    diagnostic
}

fn return_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    expected: &Type,
    actual: &Type,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0312",
        format!(
            "`return` value has type `{}`, but {} returns `{}`",
            actual.display(),
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.span()).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!("return a value of type `{}`", expected.display()));
    diagnostic
}

fn missing_return_diagnostic(
    sources: &SourceMap,
    block_span: ByteSpan,
    context: &ReturnContext,
) -> Diagnostic {
    let expected = context.success_type();
    let mut diagnostic = Diagnostic::error(
        "E0313",
        format!(
            "{} may reach the end without returning `{}`",
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(block_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!(
        "add a `return` with a value of type `{}`",
        expected.display()
    ));
    diagnostic
}

fn add_declared_return_note(
    sources: &SourceMap,
    diagnostic: &mut Diagnostic,
    context: &ReturnContext,
) {
    if let Ok(span) = sources.span_to_json(context.return_type_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!(
                "{} declares return type `{}`",
                context.subject(),
                context.declared_type.display()
            ),
            span: Some(span),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;

    fn check_text(text: &str) -> Vec<Diagnostic> {
        let mut sources = SourceMap::new();
        let source = sources.add_source("app.nct", None, text);
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let parsed = parse(&sources, source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        check(&sources, &parsed.ast.unwrap())
    }

    #[test]
    fn accepts_program_i32() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_program_void() {
        let diagnostics = check_text(
            r#"program(): void {
    return
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn diagnoses_main_without_program() {
        let diagnostics = check_text(
            r#"func main(): i32 {
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0301");
    }

    #[test]
    fn diagnoses_invalid_program_return_type() {
        let diagnostics = check_text(
            r#"program(): u64 {
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0303");
    }

    #[test]
    fn diagnoses_duplicate_program() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

program(): void {
    return
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0302");
    }

    #[test]
    fn diagnoses_string_return_from_i32_program() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return "hello"
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0312");
    }

    #[test]
    fn diagnoses_bare_return_from_i32_program() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0310");
    }

    #[test]
    fn diagnoses_value_return_from_void_program() {
        let diagnostics = check_text(
            r#"program(): void {
    return 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0311");
    }

    #[test]
    fn diagnoses_missing_return_from_i32_program() {
        let diagnostics = check_text(
            r#"program(): i32 {
    let value = 0
}
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0313");
    }

    #[test]
    fn accepts_stringview_function_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func title(): StringView {
    return "hello"
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn accepts_optional_none_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func lookup(): i32? {
    return none
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn checks_success_type_of_fallible_return() {
        let diagnostics = check_text(
            r#"program(): i32 {
    return 0
}

func run(): void ! IOError {
    return
}
"#,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}
