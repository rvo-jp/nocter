//! Parser for Nocter source syntax.

use crate::ast::{
    AstFile, BindingKind, BindingStmt, Block, CallExpr, Expr, ExpressionStmt, FallibleType,
    FromImportItem, FunctionDecl, GroupExpr, IdentifierExpr, ImportedName, Item, LiteralExpr,
    MemberExpr, ModulePath, OptionalDefaultExpr, OptionalType, Parameter, ParameterList,
    ProgramDecl, ReturnStmt, Stmt, TryCatchExpr, TryCatchStmt, TryExpr, TryStmt, TypeExpr,
    TypeReference, UseItem,
};
use crate::diagnostics::Diagnostic;
use crate::lexer::{Keyword, Token, TokenKind};
use crate::source::{ByteSpan, SourceId, SourceMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOutput {
    pub ast: Option<AstFile>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(sources: &SourceMap, source: SourceId, tokens: &[Token]) -> ParseOutput {
    if tokens.is_empty() {
        return ParseOutput {
            ast: None,
            diagnostics: vec![Diagnostic::error(
                "E0200",
                "parser requires a token stream ending in EOF",
            )],
        };
    }

    let mut parser = Parser {
        sources,
        source,
        tokens,
        index: 0,
        diagnostics: Vec::new(),
    };

    match parser.parse_source_file() {
        Ok(ast) if parser.diagnostics.is_empty() => ParseOutput {
            ast: Some(ast),
            diagnostics: parser.diagnostics,
        },
        Ok(_) | Err(()) => ParseOutput {
            ast: None,
            diagnostics: parser.diagnostics,
        },
    }
}

type ParseResult<T> = Result<T, ()>;

struct Parser<'a> {
    sources: &'a SourceMap,
    source: SourceId,
    tokens: &'a [Token],
    index: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn parse_source_file(&mut self) -> ParseResult<AstFile> {
        let mut items = Vec::new();

        self.skip_newlines();
        while !self.at_eof() {
            items.push(self.parse_item()?);
            self.skip_newlines();
        }

        let eof = self.current().clone();
        Ok(AstFile {
            span: self.span(0, eof.span.end),
            items,
        })
    }

    fn parse_item(&mut self) -> ParseResult<Item> {
        if self.at_keyword(Keyword::Use) {
            return self.parse_use_item();
        }

        if self.at_keyword(Keyword::From) {
            return self.parse_from_import_item();
        }

        if self.at_keyword(Keyword::Program) {
            return self.parse_program_decl();
        }

        if self.at_keyword(Keyword::Func) {
            return self.parse_function_decl();
        }

        self.error_current("expected a top-level item");
        Err(())
    }

    fn parse_use_item(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Use, "`use`")?;
        let path = self.parse_module_path()?;
        let end = path.span.end;

        Ok(Item::Use(UseItem {
            span: self.span(start.span.start, end),
            path,
        }))
    }

    fn parse_from_import_item(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::From, "`from`")?;
        let path = self.parse_module_path()?;
        self.expect_keyword(Keyword::Import, "`import`")?;

        let first = self.expect_identifier("expected an imported name")?;
        let mut end = first.span.end;
        let mut names = vec![ImportedName {
            span: first.span,
            name: first.value,
        }];

        while self.match_punctuation(",").is_some() {
            self.skip_newlines();
            let name = self.expect_identifier("expected an imported name after `,`")?;
            end = name.span.end;
            names.push(ImportedName {
                span: name.span,
                name: name.value,
            });
        }

        Ok(Item::FromImport(FromImportItem {
            span: self.span(start.span.start, end),
            path,
            names,
        }))
    }

    fn parse_program_decl(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Program, "`program`")?;
        self.expect_punctuation("(", "`(`")?;
        self.expect_punctuation(")", "`)`")?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Item::Program(ProgramDecl {
            span: self.span(start.span.start, end),
            return_type,
            body,
        }))
    }

    fn parse_function_decl(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Func, "`func`")?;
        let name = self.expect_identifier("expected function name after `func`")?;
        let parameters = self.parse_parameter_list()?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Item::Function(FunctionDecl {
            span: self.span(start.span.start, end),
            name: name.value,
            name_span: name.span,
            parameters,
            return_type,
            body,
        }))
    }

    fn parse_parameter_list(&mut self) -> ParseResult<ParameterList> {
        let start = self.expect_punctuation("(", "`(`")?;
        let mut parameters = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation(")") {
            if self.at_eof() {
                self.error_current("expected `)` to close parameter list");
                return Err(());
            }

            let name = self.expect_identifier("expected parameter name")?;
            self.expect_punctuation(":", "`:`")?;
            let ty = self.parse_type()?;
            let end = ty.span().end;
            parameters.push(Parameter {
                span: self.span(name.span.start, end),
                name: name.value,
                name_span: name.span,
                ty,
            });

            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }

        let end = self.expect_punctuation(")", "`)`")?;
        Ok(ParameterList {
            span: self.span(start.span.start, end.span.end),
            parameters,
        })
    }

    fn parse_type(&mut self) -> ParseResult<TypeExpr> {
        let mut ty = self.parse_type_atom()?;

        loop {
            if let Some(question) = self.match_punctuation("?") {
                ty = TypeExpr::Optional(OptionalType {
                    span: self.span(ty.span().start, question.span.end),
                    inner: Box::new(ty),
                });
                continue;
            }

            if self.match_punctuation("!").is_some() {
                let error = self.parse_type()?;
                ty = TypeExpr::Fallible(FallibleType {
                    span: self.span(ty.span().start, error.span().end),
                    success: Box::new(ty),
                    error: Box::new(error),
                });
            }

            break;
        }

        Ok(ty)
    }

    fn parse_type_atom(&mut self) -> ParseResult<TypeExpr> {
        if self.at_keyword(Keyword::Void) {
            let token = self.bump();
            return Ok(TypeExpr::Reference(TypeReference {
                span: token.span,
                name: "void".to_string(),
            }));
        }

        if self.at_keyword(Keyword::Never) {
            let token = self.bump();
            return Ok(TypeExpr::Reference(TypeReference {
                span: token.span,
                name: "never".to_string(),
            }));
        }

        let name = self.expect_identifier("expected type")?;
        Ok(TypeExpr::Reference(TypeReference {
            span: name.span,
            name: name.value,
        }))
    }

    fn parse_block(&mut self) -> ParseResult<Block> {
        let start = self.expect_punctuation("{", "`{`")?;
        let mut statements = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_current("expected `}` to close block");
                return Err(());
            }

            statements.push(self.parse_statement()?);
            self.skip_newlines();
        }

        let end = self.expect_punctuation("}", "`}`")?;
        Ok(Block {
            span: self.span(start.span.start, end.span.end),
            statements,
        })
    }

    fn parse_statement(&mut self) -> ParseResult<Stmt> {
        self.skip_newlines();

        if self.at_keyword(Keyword::Return) {
            return self.parse_return_statement();
        }

        if self.at_keyword(Keyword::Try) {
            return self.parse_try_statement();
        }

        if self.at_keyword(Keyword::Let) || self.at_keyword(Keyword::Var) {
            return self.parse_binding_statement();
        }

        self.parse_expression_statement()
    }

    fn parse_return_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::Return, "`return`")?;
        if self.at_statement_end() {
            return Ok(Stmt::Return(ReturnStmt {
                span: self.span(start.span.start, start.span.end),
                expression: None,
            }));
        }

        let expression = self.parse_expression()?;
        let end = expression.span().end;
        Ok(Stmt::Return(ReturnStmt {
            span: self.span(start.span.start, end),
            expression: Some(expression),
        }))
    }

    fn parse_binding_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.bump();
        let kind = match start.kind {
            TokenKind::Keyword(Keyword::Let) => BindingKind::Let,
            TokenKind::Keyword(Keyword::Var) => BindingKind::Var,
            _ => unreachable!("parse_binding_statement starts at let or var"),
        };
        let name = self.expect_identifier("expected binding name")?;
        let ty = if self.match_punctuation(":").is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect_punctuation("=", "`=`")?;
        let initializer = self.parse_expression()?;
        let end = initializer.span().end;

        Ok(Stmt::Binding(BindingStmt {
            span: self.span(start.span.start, end),
            kind,
            name: name.value,
            name_span: name.span,
            ty,
            initializer,
        }))
    }

    fn parse_try_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::Try, "`try`")?;
        let expression = self.parse_expression()?;

        if self.match_keyword(Keyword::Catch).is_some() {
            let error = self.expect_identifier("expected catch binding name")?;
            let catch_block = self.parse_block()?;
            let end = catch_block.span.end;
            return Ok(Stmt::TryCatch(TryCatchStmt {
                span: self.span(start.span.start, end),
                expression,
                error_name: error.value,
                error_span: error.span,
                catch_block,
            }));
        }

        let end = expression.span().end;
        Ok(Stmt::Try(TryStmt {
            span: self.span(start.span.start, end),
            expression,
        }))
    }

    fn parse_expression_statement(&mut self) -> ParseResult<Stmt> {
        let expression = self.parse_expression()?;
        let span = expression.span();
        Ok(Stmt::Expression(ExpressionStmt { span, expression }))
    }

    fn parse_expression(&mut self) -> ParseResult<Expr> {
        self.parse_optional_default_expression()
    }

    fn parse_optional_default_expression(&mut self) -> ParseResult<Expr> {
        let left = self.parse_prefix_expression()?;

        if self.match_punctuation("??").is_some() {
            let right = self.parse_optional_default_expression()?;
            return Ok(Expr::OptionalDefault(OptionalDefaultExpr {
                span: self.span(left.span().start, right.span().end),
                value: Box::new(left),
                default: Box::new(right),
            }));
        }

        Ok(left)
    }

    fn parse_prefix_expression(&mut self) -> ParseResult<Expr> {
        if self.at_keyword(Keyword::Try) {
            return self.parse_try_expression();
        }

        self.parse_postfix_expression()
    }

    fn parse_try_expression(&mut self) -> ParseResult<Expr> {
        let start = self.expect_keyword(Keyword::Try, "`try`")?;
        let expression = self.parse_expression()?;

        if self.match_keyword(Keyword::Catch).is_some() {
            let error = self.expect_identifier("expected catch binding name")?;
            let catch_block = self.parse_block()?;
            let end = catch_block.span.end;
            return Ok(Expr::TryCatch(TryCatchExpr {
                span: self.span(start.span.start, end),
                expression: Box::new(expression),
                error_name: error.value,
                error_span: error.span,
                catch_block,
            }));
        }

        let end = expression.span().end;
        Ok(Expr::Try(TryExpr {
            span: self.span(start.span.start, end),
            expression: Box::new(expression),
        }))
    }

    fn parse_postfix_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_primary_expression()?;

        loop {
            if self.at_punctuation("(") {
                expression = self.finish_call_expression(expression)?;
                continue;
            }

            if self.match_punctuation(".").is_some() {
                let member = self.expect_identifier("expected member name after `.`")?;
                expression = Expr::Member(MemberExpr {
                    span: self.span(expression.span().start, member.span.end),
                    object: Box::new(expression),
                    member: member.value,
                    member_span: member.span,
                });
                continue;
            }

            break;
        }

        Ok(expression)
    }

    fn finish_call_expression(&mut self, callee: Expr) -> ParseResult<Expr> {
        let start = callee.span().start;
        let open = self.expect_punctuation("(", "`(`")?;
        let mut arguments = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation(")") {
            if self.at_eof() {
                self.error_current("expected `)` to close argument list");
                return Err(());
            }

            arguments.push(self.parse_expression()?);
            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }

        let close = self.expect_punctuation(")", "`)`")?;
        Ok(Expr::Call(CallExpr {
            span: self.span(start, close.span.end),
            callee: Box::new(callee),
            arguments_span: self.span(open.span.start, close.span.end),
            arguments,
        }))
    }

    fn parse_primary_expression(&mut self) -> ParseResult<Expr> {
        match self.current().kind {
            TokenKind::Identifier => {
                let token = self.bump();
                Ok(Expr::Identifier(IdentifierExpr {
                    span: token.span,
                    name: self.lexeme(&token),
                }))
            }
            TokenKind::IntegerLiteral => {
                let token = self.bump();
                Ok(Expr::IntegerLiteral(LiteralExpr {
                    span: token.span,
                    value: self.lexeme(&token),
                }))
            }
            TokenKind::StringLiteral => {
                let token = self.bump();
                Ok(Expr::StringLiteral(LiteralExpr {
                    span: token.span,
                    value: self.lexeme(&token),
                }))
            }
            TokenKind::Keyword(Keyword::None) => {
                let token = self.bump();
                Ok(Expr::NoneLiteral(LiteralExpr {
                    span: token.span,
                    value: "none".to_string(),
                }))
            }
            TokenKind::Punctuation("(") => {
                let start = self.bump();
                let expression = self.parse_expression()?;
                let end = self.expect_punctuation(")", "`)`")?;
                Ok(Expr::Group(GroupExpr {
                    span: self.span(start.span.start, end.span.end),
                    expression: Box::new(expression),
                }))
            }
            _ => {
                self.error_current("expected expression");
                Err(())
            }
        }
    }

    fn parse_module_path(&mut self) -> ParseResult<ModulePath> {
        let mut value = String::new();
        let mut segments = Vec::new();
        let mut start = self.current().span.start;

        while self.at_punctuation(".") {
            let dot = self.bump();
            start = dot.span.start;

            if self.match_punctuation("/").is_some() {
                value.push_str("./");
                segments.push(".".to_string());
                break;
            }

            if self.match_punctuation(".").is_some() {
                self.expect_punctuation("/", "`/`")?;
                value.push_str("../");
                segments.push("..".to_string());
                continue;
            }

            self.error_current("expected `/` or `.` in relative module path");
            return Err(());
        }

        let first = self.expect_identifier("expected module path segment")?;
        let mut end = first.span.end;
        segments.push(first.value);

        while self.match_punctuation("/").is_some() {
            let segment = self.expect_identifier("expected module path segment after `/`")?;
            end = segment.span.end;
            segments.push(segment.value);
        }

        let path_segments = segments
            .iter()
            .filter(|segment| segment.as_str() != "." && segment.as_str() != "..")
            .cloned()
            .collect::<Vec<_>>();
        value.push_str(&path_segments.join("/"));

        Ok(ModulePath {
            span: self.span(start, end),
            value,
            segments,
        })
    }

    fn expect_identifier(&mut self, message: &str) -> ParseResult<ParsedIdentifier> {
        if self.current().kind == TokenKind::Identifier {
            let token = self.bump();
            return Ok(ParsedIdentifier {
                value: self.lexeme(&token),
                span: token.span,
            });
        }

        self.error_current(message);
        Err(())
    }

    fn expect_keyword(&mut self, keyword: Keyword, expected: &str) -> ParseResult<Token> {
        if self.at_keyword(keyword) {
            return Ok(self.bump());
        }

        self.error_current(format!("expected {expected}"));
        Err(())
    }

    fn expect_punctuation(&mut self, punctuation: &str, expected: &str) -> ParseResult<Token> {
        if self.at_punctuation(punctuation) {
            return Ok(self.bump());
        }

        self.error_current(format!("expected {expected}"));
        Err(())
    }

    fn match_keyword(&mut self, keyword: Keyword) -> Option<Token> {
        if self.at_keyword(keyword) {
            Some(self.bump())
        } else {
            None
        }
    }

    fn match_punctuation(&mut self, punctuation: &str) -> Option<Token> {
        if self.at_punctuation(punctuation) {
            Some(self.bump())
        } else {
            None
        }
    }

    fn at_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current().kind, TokenKind::Keyword(actual) if actual == keyword)
    }

    fn at_punctuation(&self, punctuation: &str) -> bool {
        matches!(self.current().kind, TokenKind::Punctuation(actual) if actual == punctuation)
    }

    fn at_statement_end(&self) -> bool {
        self.current().kind == TokenKind::Newline || self.at_punctuation("}") || self.at_eof()
    }

    fn at_eof(&self) -> bool {
        self.current().kind == TokenKind::Eof
    }

    fn skip_newlines(&mut self) {
        while self.current().kind == TokenKind::Newline {
            self.index += 1;
        }
    }

    fn current(&self) -> &Token {
        self.tokens
            .get(self.index)
            .unwrap_or_else(|| self.tokens.last().expect("parser requires an EOF token"))
    }

    fn bump(&mut self) -> Token {
        let token = self.current().clone();
        if !self.at_eof() {
            self.index += 1;
        }
        token
    }

    fn span(&self, start: usize, end: usize) -> ByteSpan {
        ByteSpan::new(self.source, start, end)
    }

    fn lexeme(&self, token: &Token) -> String {
        self.sources
            .get(token.span.source)
            .and_then(|file| file.text().get(token.span.start..token.span.end))
            .unwrap_or("")
            .to_string()
    }

    fn error_current(&mut self, message: impl Into<String>) {
        let token = self.current();
        let primary_span = self
            .sources
            .span_to_json(ByteSpan::new(
                token.span.source,
                token.span.start,
                token.span.end,
            ))
            .ok()
            .map(Box::new);
        let mut diagnostic = Diagnostic::error("E0200", message);
        diagnostic.primary_span = primary_span;
        self.diagnostics.push(diagnostic);
    }
}

#[derive(Debug, Clone)]
struct ParsedIdentifier {
    value: String,
    span: ByteSpan,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ast::{Expr, Item, Stmt, TypeExpr};
    use crate::lexer::lex;

    fn parse_text(text: &str) -> ParseOutput {
        let mut sources = SourceMap::new();
        let source = sources.add_source("app.nct", None, text);
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        parse(&sources, source, &lexed.tokens)
    }

    #[test]
    fn parses_hello_program() {
        let output = parse_text(
            r#"use std/prelude

from std/io import print

program(): i32 {
    try print("Hello") catch error {
        return 1
    }

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        assert_eq!(ast.items.len(), 3);
        assert!(matches!(ast.items[0], Item::Use(_)));
        assert!(matches!(ast.items[1], Item::FromImport(_)));
        assert!(matches!(ast.items[2], Item::Program(_)));
    }

    #[test]
    fn parses_function_with_fallible_return_type() {
        let output = parse_text(
            r#"use std/prelude

from std/io import IOError
from std/io import print

program(): i32 {
    try run() catch error {
        return 1
    }

    return 0
}

func run(): void ! IOError {
    try print("Hello")
    return
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Some(Item::Function(function)) = ast.items.last() else {
            panic!("expected final item to be a function");
        };
        assert!(matches!(function.return_type, TypeExpr::Fallible(_)));
    }

    #[test]
    fn parses_optional_default_expression() {
        let output = parse_text(
            r#"use std/prelude

program(): i32 {
    let user = (try env("USER") catch error {
        return 1
    }) ?? "unknown"

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[1] else {
            panic!("expected program item");
        };
        let Stmt::Binding(binding) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        assert!(matches!(binding.initializer, Expr::OptionalDefault(_)));
    }

    #[test]
    fn parses_relative_import_paths() {
        let output = parse_text(
            r#"from ./config import Config
from ../shared/path import Path

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::FromImport(config) = &ast.items[0] else {
            panic!("expected first item to be a relative import");
        };
        let Item::FromImport(path) = &ast.items[1] else {
            panic!("expected second item to be a relative import");
        };

        assert_eq!(config.path.value, "./config");
        assert_eq!(path.path.value, "../shared/path");
    }

    #[test]
    fn diagnoses_unknown_top_level_item() {
        let output = parse_text(
            r#"module app/main

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.ast.is_none());
        assert_eq!(output.diagnostics.len(), 1);
        assert!(output.diagnostics[0].message.contains("top-level item"));
    }
}
