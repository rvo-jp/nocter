use super::{ParseResult, Parser};
use crate::ast::{
    Expr, LiteralCapture, LiteralContextOverride, LiteralDecl, LiteralShape, ParameterList,
    TypeExpr, TypedSequenceLiteralExpr, TypedStringLiteralExpr, UnaryExpr, UnaryOperator,
    Visibility,
};
use crate::lexer::{Keyword, TokenKind};

impl Parser<'_> {
    pub(super) fn parse_literal_decl_data(
        &mut self,
        visibility: Visibility,
        target: Option<TypeExpr>,
    ) -> ParseResult<LiteralDecl> {
        let keyword = self.expect_keyword(Keyword::Literal, "`literal`")?;
        let target = match target {
            Some(target) => target,
            None => self.parse_type()?,
        };

        let (shape, shape_span, parameters, capture) = if self.at_punctuation("[") {
            let open = self.bump();
            let close = self.expect_punctuation("]", "`]` sequence shape marker")?;
            let shape_span = self.span(open.span.start, close.span.end);
            let (parameters, capture) = self.parse_literal_sequence_parameters()?;
            (
                LiteralShape::Sequence,
                shape_span,
                parameters,
                Some(capture),
            )
        } else if self.current().kind == TokenKind::StringLiteral {
            let marker = self.bump();
            if self.lexeme(&marker) != "\"\"" {
                self.error_at(
                    marker.span,
                    "string literal shape marker must be empty `\"\"`",
                );
                return Err(());
            }
            let parameters = self.parse_parameter_list()?;
            if parameters.parameters.len() != 1 {
                self.error_at(
                    parameters.span,
                    "string literal definition requires exactly one parameter",
                );
                return Err(());
            }
            (LiteralShape::String, marker.span, parameters, None)
        } else {
            self.error_current("expected `[]` or `\"\"` literal shape marker");
            return Err(());
        };

        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let result_provenance = self.parse_result_provenance_clause()?;
        let requirements = self.parse_where_clause()?;
        let body = if self.at_punctuation("{") {
            let previous_capture = self.literal_pack_capture.clone();
            self.literal_pack_capture = capture.as_ref().map(|capture| capture.name.clone());
            let body = self.parse_block();
            self.literal_pack_capture = previous_capture;
            Some(body?)
        } else {
            None
        };
        let end = body.as_ref().map_or_else(
            || {
                requirements.as_ref().map_or_else(
                    || {
                        result_provenance
                            .as_ref()
                            .map_or(return_type.span().end, |clause| clause.span.end)
                    },
                    |clause| clause.span.end,
                )
            },
            |body| body.span.end,
        );
        Ok(LiteralDecl {
            span: self.span(keyword.span.start, end),
            visibility,
            keyword_span: keyword.span,
            target,
            shape,
            shape_span,
            parameters,
            capture,
            return_type,
            result_provenance,
            requirements,
            body,
        })
    }

    fn parse_literal_sequence_parameters(
        &mut self,
    ) -> ParseResult<(ParameterList, LiteralCapture)> {
        let open = self.expect_punctuation("(", "`(`")?;
        self.skip_newlines();
        if !self.at_ellipsis() {
            self.error_current("sequence literal definition requires `...items: T` capture");
            return Err(());
        }
        let ellipsis_span = self.ellipsis_span();
        self.bump();
        self.bump();
        self.bump();
        let name = self.expect_name_identifier("expected literal capture name after `...`")?;
        self.expect_punctuation(":", "`:`")?;
        let element_type = self.parse_type()?;
        let capture = LiteralCapture {
            span: self.span(ellipsis_span.start, element_type.span().end),
            ellipsis_span,
            name: name.value,
            name_span: name.span,
            element_type,
        };
        self.skip_newlines();
        if self.at_punctuation(",") {
            self.error_current("sequence literal capture must be the only Phase 1 parameter");
            return Err(());
        }
        let close = self.expect_punctuation(")", "`)`")?;
        Ok((
            ParameterList {
                span: self.span(open.span.start, close.span.end),
                parameters: Vec::new(),
            },
            capture,
        ))
    }

    pub(super) fn finish_typed_sequence_literal(&mut self, target: TypeExpr) -> ParseResult<Expr> {
        let start = target.span().start;
        let open = self.expect_punctuation("[", "`[`")?;
        let mut elements = Vec::new();
        self.skip_newlines();
        while !self.at_punctuation("]") {
            if self.at_eof() {
                self.error_current("expected `]` to close typed sequence literal");
                return Err(());
            }
            if self.at_ellipsis() {
                let ellipsis_span = self.ellipsis_span();
                self.bump();
                self.bump();
                self.bump();
                let source = self.parse_expression()?;
                elements.push(Expr::Unary(UnaryExpr {
                    span: self.span(ellipsis_span.start, source.span().end),
                    operator: UnaryOperator::Spread,
                    operator_span: ellipsis_span,
                    operand: Box::new(source),
                }));
            } else {
                elements.push(self.parse_expression()?);
            }
            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }
        let close = self.expect_punctuation("]", "`]`")?;
        let using = self.parse_optional_literal_context_override()?;
        let end = using
            .as_ref()
            .map_or(close.span.end, |using| using.span.end);
        Ok(Expr::TypedSequenceLiteral(TypedSequenceLiteralExpr {
            span: self.span(start, end),
            target,
            elements_span: self.span(open.span.start, close.span.end),
            elements,
            using,
        }))
    }

    pub(super) fn finish_typed_string_literal(&mut self, target: TypeExpr) -> ParseResult<Expr> {
        let start = target.span().start;
        let token = self.bump();
        let text = match self.parse_string_literal_expression(token)? {
            Expr::StringLiteral(text) => text,
            Expr::InterpolatedString(interpolated) => {
                self.error_at(
                    interpolated.span,
                    "interpolation is not a typed string literal in v0.3.0 Phase 1",
                );
                return Err(());
            }
            _ => unreachable!("string literal parser returned a non-string expression"),
        };
        let text_end = text.span.end;
        let using = self.parse_optional_literal_context_override()?;
        let end = using.as_ref().map_or(text_end, |using| using.span.end);
        Ok(Expr::TypedStringLiteral(TypedStringLiteralExpr {
            span: self.span(start, end),
            target,
            text,
            using,
        }))
    }

    fn parse_optional_literal_context_override(
        &mut self,
    ) -> ParseResult<Option<LiteralContextOverride>> {
        let Some(using) = self.match_keyword(Keyword::Using) else {
            return Ok(None);
        };
        let allocator = self.parse_prefix_expression()?;
        Ok(Some(LiteralContextOverride {
            span: self.span(using.span.start, allocator.span().end),
            using_span: using.span,
            allocator: Box::new(allocator),
        }))
    }
}
