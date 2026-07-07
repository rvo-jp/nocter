use super::support::ParsedEnumPattern;
use super::{ParseResult, Parser};
use crate::ast::{
    BindingKind, BindingStmt, Block, BreakStmt, ContinueStmt, Expr, ExpressionStmt, FailStmt,
    ForRangeStmt, IfIsStmt, IfLetStmt, IfStmt, LoopStmt, ReturnStmt, Stmt, SwitchArm,
    SwitchElseArm, SwitchPayloadBinding, SwitchStmt, WhileLetStmt, WhileStmt,
};
use crate::lexer::{Keyword, TokenKind};

impl Parser<'_> {
    pub(super) fn parse_block(&mut self) -> ParseResult<Block> {
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

    pub(super) fn parse_statement(&mut self) -> ParseResult<Stmt> {
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

    pub(super) fn parse_return_statement(&mut self) -> ParseResult<Stmt> {
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

    pub(super) fn parse_fail_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::Fail, "`fail`")?;
        let expression = self.parse_expression()?;
        let end = expression.span().end;
        Ok(Stmt::Fail(FailStmt {
            span: self.span(start.span.start, end),
            expression,
        }))
    }

    pub(super) fn parse_binding_statement(&mut self) -> ParseResult<Stmt> {
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

    pub(super) fn parse_if_statement(&mut self) -> ParseResult<Stmt> {
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

    pub(super) fn parse_if_is_statement(
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

    pub(super) fn parse_if_let_statement(&mut self, start: usize) -> ParseResult<Stmt> {
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

    pub(super) fn parse_else_block(&mut self) -> ParseResult<Option<Block>> {
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

    pub(super) fn parse_switch_statement(&mut self) -> ParseResult<Stmt> {
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

    pub(super) fn parse_switch_arm(&mut self) -> ParseResult<SwitchArm> {
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

    pub(super) fn parse_enum_pattern_after_is(
        &mut self,
        start: usize,
    ) -> ParseResult<ParsedEnumPattern> {
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

    pub(super) fn parse_switch_else_arm(&mut self) -> ParseResult<SwitchElseArm> {
        let start = self.expect_keyword(Keyword::Else, "`else`")?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(SwitchElseArm {
            span: self.span(start.span.start, end),
            body,
        })
    }

    pub(super) fn parse_for_statement(&mut self) -> ParseResult<Stmt> {
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

    pub(super) fn parse_while_statement(&mut self) -> ParseResult<Stmt> {
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

    pub(super) fn parse_while_let_statement(&mut self, start: usize) -> ParseResult<Stmt> {
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

    pub(super) fn parse_loop_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::Loop, "`loop`")?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Stmt::Loop(LoopStmt {
            span: self.span(start.span.start, end),
            body,
        }))
    }

    pub(super) fn parse_break_statement(&mut self) -> ParseResult<Stmt> {
        let token = self.expect_keyword(Keyword::Break, "`break`")?;
        self.expect_statement_end("`break` does not take a value")?;
        Ok(Stmt::Break(BreakStmt {
            span: self.span(token.span.start, token.span.end),
        }))
    }

    pub(super) fn parse_continue_statement(&mut self) -> ParseResult<Stmt> {
        let token = self.expect_keyword(Keyword::Continue, "`continue`")?;
        self.expect_statement_end("`continue` does not take a value")?;
        Ok(Stmt::Continue(ContinueStmt {
            span: self.span(token.span.start, token.span.end),
        }))
    }

    pub(super) fn parse_expression_statement(&mut self) -> ParseResult<Stmt> {
        let expression = self.parse_expression()?;
        let span = expression.span();
        Ok(Stmt::Expression(ExpressionStmt { span, expression }))
    }
}
