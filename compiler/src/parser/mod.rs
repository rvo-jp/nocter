//! Parser for Nocter source syntax.

mod cursor;
mod items;
mod support;
mod types;

use crate::ast::{
    ArrayLiteralExpr, AstFile, BinaryExpr, BindingKind, BindingStmt, Block, BreakStmt, CallExpr,
    CatchExpr, ContinueStmt, Expr, ExpressionStmt, FailStmt, ForRangeStmt, ForceExpr, GroupExpr,
    IdentifierExpr, IfIsStmt, IfLetStmt, IfStmt, IndexExpr, LiteralExpr, LoopStmt, MemberExpr,
    OptionalDefaultExpr, PropagationExpr, ReturnStmt, Stmt, StructLiteralExpr, StructLiteralField,
    SwitchArm, SwitchElseArm, SwitchPayloadBinding, SwitchStmt, TypeConversionExpr, TypeExpr,
    TypeReference, UnaryExpr, WhileLetStmt, WhileStmt,
};
use crate::diagnostics::Diagnostic;
use crate::lexer::{Keyword, Token, TokenKind};
use crate::source::{SourceId, SourceMap};
use support::ParsedEnumPattern;

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

        if self.at_keyword(Keyword::Fail) {
            return self.parse_fail_statement();
        }

        if self.at_keyword(Keyword::If) {
            return self.parse_if_statement();
        }

        if self.at_keyword(Keyword::Switch) {
            return self.parse_switch_statement();
        }

        if self.at_keyword(Keyword::For) {
            return self.parse_for_statement();
        }

        if self.at_keyword(Keyword::While) {
            return self.parse_while_statement();
        }

        if self.at_keyword(Keyword::Loop) {
            return self.parse_loop_statement();
        }

        if self.at_keyword(Keyword::Break) {
            return self.parse_break_statement();
        }

        if self.at_keyword(Keyword::Continue) {
            return self.parse_continue_statement();
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

    fn parse_fail_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::Fail, "`fail`")?;
        let expression = self.parse_expression()?;
        let end = expression.span().end;
        Ok(Stmt::Fail(FailStmt {
            span: self.span(start.span.start, end),
            expression,
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
        let else_block = if self.match_keyword(Keyword::Else).is_some() {
            Some(self.parse_block()?)
        } else {
            None
        };
        let end = else_block
            .as_ref()
            .map_or(initializer.span().end, |block| block.span.end);

        Ok(Stmt::Binding(BindingStmt {
            span: self.span(start.span.start, end),
            kind,
            name: name.value,
            name_span: name.span,
            ty,
            initializer,
            else_block,
        }))
    }

    fn parse_if_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::If, "`if`")?;
        if self.at_keyword(Keyword::Let) || self.at_keyword(Keyword::Var) {
            return self.parse_if_let_statement(start.span.start);
        }

        let condition = self.parse_expression()?;
        if let Some(is_token) = self.match_keyword(Keyword::Is) {
            return self.parse_if_is_statement(start.span.start, condition, is_token.span.start);
        }

        let then_block = self.parse_block()?;
        let else_block = self.parse_else_block()?;
        let end = else_block
            .as_ref()
            .map_or(then_block.span.end, |block| block.span.end);

        Ok(Stmt::If(IfStmt {
            span: self.span(start.span.start, end),
            condition,
            then_block,
            else_block,
        }))
    }

    fn parse_if_is_statement(
        &mut self,
        start: usize,
        expression: Expr,
        pattern_start: usize,
    ) -> ParseResult<Stmt> {
        let pattern = self.parse_enum_pattern_after_is(pattern_start)?;
        let then_block = self.parse_block()?;
        let else_block = self.parse_else_block()?;
        let end = else_block
            .as_ref()
            .map_or(then_block.span.end, |block| block.span.end);

        Ok(Stmt::IfIs(IfIsStmt {
            span: self.span(start, end),
            expression,
            pattern_span: pattern.span,
            enum_name: pattern.enum_name,
            enum_name_span: pattern.enum_name_span,
            variant_name: pattern.variant_name,
            variant_name_span: pattern.variant_name_span,
            payload: pattern.payload,
            then_block,
            else_block,
        }))
    }

    fn parse_if_let_statement(&mut self, start: usize) -> ParseResult<Stmt> {
        let binding = self.bump();
        let kind = match binding.kind {
            TokenKind::Keyword(Keyword::Let) => BindingKind::Let,
            TokenKind::Keyword(Keyword::Var) => BindingKind::Var,
            _ => unreachable!("parse_if_let_statement starts at let or var"),
        };
        let name = self.expect_identifier("expected optional binding name")?;
        self.expect_punctuation("=", "`=`")?;
        let initializer = self.parse_expression()?;
        let then_block = self.parse_block()?;
        let else_block = self.parse_else_block()?;
        let end = else_block
            .as_ref()
            .map_or(then_block.span.end, |block| block.span.end);

        Ok(Stmt::IfLet(IfLetStmt {
            span: self.span(start, end),
            kind,
            name: name.value,
            name_span: name.span,
            initializer,
            then_block,
            else_block,
        }))
    }

    fn parse_else_block(&mut self) -> ParseResult<Option<Block>> {
        let Some(else_token) = self.match_keyword(Keyword::Else) else {
            return Ok(None);
        };

        if self.at_keyword(Keyword::If) {
            let statement = self.parse_if_statement()?;
            return Ok(Some(Block {
                span: self.span(else_token.span.start, statement.span().end),
                statements: vec![statement],
            }));
        }

        Ok(Some(self.parse_block()?))
    }

    fn parse_switch_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::Switch, "`switch`")?;
        let expression = self.parse_expression()?;
        let open = self.expect_punctuation("{", "`{`")?;
        let mut arms = Vec::new();
        let mut else_arm = None;
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close switch statement");
                return Err(());
            }

            if self.at_keyword(Keyword::Else) {
                if else_arm.is_some() {
                    self.error_current("`switch` can have only one `else` arm");
                    return Err(());
                }

                else_arm = Some(self.parse_switch_else_arm()?);
                self.skip_newlines();
                continue;
            }

            if else_arm.is_some() {
                self.error_current("`else` arm must be the last switch arm");
                return Err(());
            }

            arms.push(self.parse_switch_arm()?);
            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Stmt::Switch(SwitchStmt {
            span: self.span(start.span.start, close.span.end),
            expression,
            arms,
            else_arm,
        }))
    }

    fn parse_switch_arm(&mut self) -> ParseResult<SwitchArm> {
        let start = self.expect_keyword(Keyword::Is, "`is`")?;
        let pattern = self.parse_enum_pattern_after_is(start.span.start)?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(SwitchArm {
            span: self.span(start.span.start, end),
            enum_name: pattern.enum_name,
            enum_name_span: pattern.enum_name_span,
            variant_name: pattern.variant_name,
            variant_name_span: pattern.variant_name_span,
            payload: pattern.payload,
            body,
        })
    }

    fn parse_enum_pattern_after_is(&mut self, start: usize) -> ParseResult<ParsedEnumPattern> {
        let enum_name = self.expect_identifier("expected enum name after `is`")?;
        self.expect_punctuation(".", "`.`")?;
        let variant_name = self.expect_identifier("expected enum variant name after `.`")?;
        let mut end = variant_name.span.end;
        let payload = if self.match_punctuation("(").is_some() {
            let payload = self.expect_identifier("expected payload binding name")?;
            let close = self.expect_punctuation(")", "`)`")?;
            end = close.span.end;
            Some(SwitchPayloadBinding {
                span: payload.span,
                name: payload.value,
            })
        } else {
            None
        };

        Ok(ParsedEnumPattern {
            span: self.span(start, end),
            enum_name: enum_name.value,
            enum_name_span: enum_name.span,
            variant_name: variant_name.value,
            variant_name_span: variant_name.span,
            payload,
        })
    }

    fn parse_switch_else_arm(&mut self) -> ParseResult<SwitchElseArm> {
        let start = self.expect_keyword(Keyword::Else, "`else`")?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(SwitchElseArm {
            span: self.span(start.span.start, end),
            body,
        })
    }

    fn parse_for_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::For, "`for`")?;
        let name = self.expect_identifier("expected loop variable name after `for`")?;
        self.expect_keyword(Keyword::In, "`in`")?;
        let range_start = self.parse_expression()?;
        let range = self.expect_punctuation("..<", "`..<`")?;
        let range_end = self.parse_expression()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Stmt::ForRange(ForRangeStmt {
            span: self.span(start.span.start, end),
            name: name.value,
            name_span: name.span,
            start: range_start,
            range_span: range.span,
            end: range_end,
            body,
        }))
    }

    fn parse_while_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::While, "`while`")?;
        if self.at_keyword(Keyword::Let) || self.at_keyword(Keyword::Var) {
            return self.parse_while_let_statement(start.span.start);
        }

        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Stmt::While(WhileStmt {
            span: self.span(start.span.start, end),
            condition,
            body,
        }))
    }

    fn parse_while_let_statement(&mut self, start: usize) -> ParseResult<Stmt> {
        let binding = self.bump();
        let kind = match binding.kind {
            TokenKind::Keyword(Keyword::Let) => BindingKind::Let,
            TokenKind::Keyword(Keyword::Var) => BindingKind::Var,
            _ => unreachable!("parse_while_let_statement starts at let or var"),
        };
        let name = self.expect_identifier("expected optional loop binding name")?;
        self.expect_punctuation("=", "`=`")?;
        let initializer = self.parse_expression()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Stmt::WhileLet(WhileLetStmt {
            span: self.span(start, end),
            kind,
            name: name.value,
            name_span: name.span,
            initializer,
            body,
        }))
    }

    fn parse_loop_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::Loop, "`loop`")?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Stmt::Loop(LoopStmt {
            span: self.span(start.span.start, end),
            body,
        }))
    }

    fn parse_break_statement(&mut self) -> ParseResult<Stmt> {
        let token = self.expect_keyword(Keyword::Break, "`break`")?;
        self.expect_statement_end("`break` does not take a value")?;
        Ok(Stmt::Break(BreakStmt {
            span: self.span(token.span.start, token.span.end),
        }))
    }

    fn parse_continue_statement(&mut self) -> ParseResult<Stmt> {
        let token = self.expect_keyword(Keyword::Continue, "`continue`")?;
        self.expect_statement_end("`continue` does not take a value")?;
        Ok(Stmt::Continue(ContinueStmt {
            span: self.span(token.span.start, token.span.end),
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
        let left = self.parse_logical_or_expression()?;

        if let Some(operator) = self.match_punctuation("??") {
            let right = self.parse_optional_default_expression()?;
            return Ok(Expr::OptionalDefault(OptionalDefaultExpr {
                span: self.span(left.span().start, right.span().end),
                operator_span: operator.span,
                value: Box::new(left),
                default: Box::new(right),
            }));
        }

        Ok(left)
    }

    fn parse_logical_or_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_logical_and_expression()?;

        while let Some(operator) = self.match_logical_or_operator() {
            let right = self.parse_logical_and_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_logical_and_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_equality_expression()?;

        while let Some(operator) = self.match_logical_and_operator() {
            let right = self.parse_equality_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_equality_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_ordering_expression()?;

        while let Some(operator) = self.match_equality_operator() {
            let right = self.parse_ordering_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_ordering_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_shift_expression()?;

        while let Some(operator) = self.match_ordering_operator() {
            let right = self.parse_shift_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_shift_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_additive_expression()?;

        while let Some(operator) = self.match_shift_operator() {
            let right = self.parse_additive_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_additive_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_multiplicative_expression()?;

        while let Some(operator) = self.match_additive_operator() {
            let right = self.parse_multiplicative_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_multiplicative_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_prefix_expression()?;

        while let Some(operator) = self.match_multiplicative_operator() {
            let right = self.parse_prefix_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_prefix_expression(&mut self) -> ParseResult<Expr> {
        if let Some(operator) = self.match_unary_operator() {
            let operand = self.parse_prefix_expression()?;
            return Ok(Expr::Unary(UnaryExpr {
                span: self.span(operator.span.start, operand.span().end),
                operator: operator.value,
                operator_span: operator.span,
                operand: Box::new(operand),
            }));
        }

        self.parse_postfix_expression()
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

            if self.at_punctuation("[") {
                expression = self.finish_index_expression(expression)?;
                continue;
            }

            if let Some(question) = self.match_punctuation("?") {
                expression = Expr::Propagate(PropagationExpr {
                    span: self.span(expression.span().start, question.span.end),
                    operator_span: question.span,
                    expression: Box::new(expression),
                });
                continue;
            }

            if let Some(bang) = self.match_punctuation("!") {
                expression = Expr::Force(ForceExpr {
                    span: self.span(expression.span().start, bang.span.end),
                    operator_span: bang.span,
                    expression: Box::new(expression),
                });
                continue;
            }

            if let Some(catch) = self.match_keyword(Keyword::Catch) {
                let error = self.expect_identifier("expected catch binding name")?;
                let catch_block = self.parse_block()?;
                let end = catch_block.span.end;
                expression = Expr::Catch(CatchExpr {
                    span: self.span(expression.span().start, end),
                    catch_span: catch.span,
                    expression: Box::new(expression),
                    error_name: error.value,
                    error_span: error.span,
                    catch_block,
                });
                continue;
            }

            if let Some(as_token) = self.match_keyword(Keyword::As) {
                let ty = self.parse_type()?;
                expression = Expr::TypeConversion(TypeConversionExpr {
                    span: self.span(expression.span().start, ty.span().end),
                    expression: Box::new(expression),
                    as_span: as_token.span,
                    ty,
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

    fn finish_index_expression(&mut self, object: Expr) -> ParseResult<Expr> {
        let start = object.span().start;
        let open = self.expect_punctuation("[", "`[`")?;
        self.skip_newlines();
        let index = self.parse_expression()?;
        self.skip_newlines();
        let close = self.expect_punctuation("]", "`]`")?;
        Ok(Expr::Index(IndexExpr {
            span: self.span(start, close.span.end),
            object: Box::new(object),
            index_span: self.span(open.span.start, close.span.end),
            index: Box::new(index),
        }))
    }

    fn finish_struct_literal_expression(&mut self, ty: TypeExpr) -> ParseResult<Expr> {
        let start = ty.span().start;
        let open = self.expect_punctuation("{", "`{`")?;
        let mut fields = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_current("expected `}` to close struct literal");
                return Err(());
            }

            let name = self.expect_identifier("expected struct literal field name")?;
            self.expect_punctuation(":", "`:`")?;
            let value = self.parse_expression()?;
            fields.push(StructLiteralField {
                span: self.span(name.span.start, value.span().end),
                name: name.value,
                name_span: name.span,
                value,
            });

            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Expr::StructLiteral(StructLiteralExpr {
            span: self.span(start, close.span.end),
            ty,
            fields_span: self.span(open.span.start, close.span.end),
            fields,
        }))
    }

    fn parse_primary_expression(&mut self) -> ParseResult<Expr> {
        match self.current().kind {
            TokenKind::Identifier => {
                let token = self.bump();
                let name = self.lexeme(&token);
                if self.looks_like_struct_literal_body() {
                    return self.finish_struct_literal_expression(TypeExpr::Reference(
                        TypeReference {
                            span: token.span,
                            name,
                        },
                    ));
                }

                Ok(Expr::Identifier(IdentifierExpr {
                    span: token.span,
                    name,
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
            TokenKind::Keyword(Keyword::True) | TokenKind::Keyword(Keyword::False) => {
                let token = self.bump();
                Ok(Expr::BoolLiteral(LiteralExpr {
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
            TokenKind::Punctuation("[") => self.parse_array_literal_expression(),
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

    fn parse_array_literal_expression(&mut self) -> ParseResult<Expr> {
        let open = self.expect_punctuation("[", "`[`")?;
        let mut elements = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("]") {
            if self.at_eof() {
                self.error_current("expected `]` to close array literal");
                return Err(());
            }

            elements.push(self.parse_expression()?);
            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }

        let close = self.expect_punctuation("]", "`]`")?;
        Ok(Expr::ArrayLiteral(ArrayLiteralExpr {
            span: self.span(open.span.start, close.span.end),
            elements_span: self.span(open.span.start, close.span.end),
            elements,
        }))
    }
}

#[cfg(test)]
mod tests;
