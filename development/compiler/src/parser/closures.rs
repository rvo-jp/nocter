//! Closure expression parsing.

use super::{ParseResult, Parser};
use crate::ast::{ClosureCapture, ClosureCaptureMode, ClosureExpr, ClosureParameter, Expr};
use crate::lexer::Keyword;

impl Parser<'_> {
    pub(super) fn parse_closure_expression(&mut self) -> ParseResult<Expr> {
        let open = self.expect_punctuation("(", "`(`")?;
        let mut captures = Vec::new();
        let mut parameters = Vec::new();
        let mut capture_separator_span = None;
        self.skip_newlines();

        if self.closure_header_has_capture_separator() {
            while !self.at_punctuation(";") {
                captures.push(self.parse_closure_capture()?);
                self.skip_newlines();
                if self.match_punctuation(",").is_none() {
                    break;
                }
                self.skip_newlines();
            }
            let separator = self.expect_punctuation(";", "`;` after the closure capture list")?;
            capture_separator_span = Some(separator.span);
            self.skip_newlines();
        }

        while !self.at_punctuation(")") {
            parameters.push(self.parse_closure_parameter()?);
            self.skip_newlines();
            let Some(comma) = self.match_punctuation(",") else {
                break;
            };
            self.skip_newlines();
            if self.at_punctuation(")") {
                self.error_at(
                    comma.span,
                    "trailing commas in closure parameter lists are not supported",
                );
                return Err(());
            }
        }

        let close = self.expect_punctuation(")", "`)`")?;
        self.skip_newlines();
        let return_type = if self.match_punctuation(":").is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.skip_newlines();
        let body = self.parse_block()?;

        Ok(Expr::Closure(ClosureExpr {
            span: self.span(open.span.start, body.span.end),
            parameters_span: self.span(open.span.start, close.span.end),
            captures,
            capture_separator_span,
            parameters,
            return_type,
            body,
        }))
    }

    fn parse_closure_capture(&mut self) -> ParseResult<ClosureCapture> {
        let (mode, operator) = if let Some(operator) = self.match_punctuation("&+") {
            (ClosureCaptureMode::ReadwriteBorrow, operator)
        } else if let Some(operator) = self.match_punctuation("&") {
            (ClosureCaptureMode::ReadonlyBorrow, operator)
        } else if let Some(operator) = self.match_keyword(Keyword::Move) {
            (ClosureCaptureMode::Move, operator)
        } else {
            self.error_current("expected `&`, `&+`, or `move` closure capture");
            return Err(());
        };
        let name = self.expect_name_identifier("expected captured binding name")?;
        Ok(ClosureCapture {
            span: self.span(operator.span.start, name.span.end),
            mode,
            operator_span: operator.span,
            name: name.value,
            name_span: name.span,
        })
    }

    fn parse_closure_parameter(&mut self) -> ParseResult<ClosureParameter> {
        let name = self.expect_name_identifier("expected closure parameter name")?;
        let ty = if self.match_punctuation(":").is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };
        Ok(ClosureParameter {
            span: self.span(
                name.span.start,
                ty.as_ref().map_or(name.span.end, |ty| ty.span().end),
            ),
            name: name.value,
            name_span: name.span,
            ty,
        })
    }
}
