use super::{ParseResult, Parser};
use crate::ast::{
    ArrayLiteralExpr, BinaryExpr, BorrowExpr, CallExpr, CatchExpr, Expr, ForceExpr, GenericType,
    GroupExpr, IdentifierExpr, IndexExpr, InterpolatedStringExpr, InterpolatedStringExpression,
    InterpolatedStringPart, InterpolatedStringText, LiteralExpr, MemberExpr, OtherwiseExpr,
    PropagationExpr, StructLiteralExpr, StructLiteralField, TypeConversionExpr, TypeExpr,
    TypeReference, UnaryExpr,
};
use crate::lexer::{Keyword, Token, TokenKind, lex_span};
use crate::literals::{StringLiteralPartSpan, decode_interpolated_text_part, string_literal_parts};
use crate::source::ByteSpan;

impl Parser<'_> {
    pub(super) fn parse_expression(&mut self) -> ParseResult<Expr> {
        self.parse_otherwise_expression()
    }

    fn parse_otherwise_expression(&mut self) -> ParseResult<Expr> {
        let left = self.parse_logical_or_expression()?;

        if self.at_punctuation("?") && self.next_is_punctuation("{") {
            self.error_current(
                "`?{}` pattern conditional expressions were removed; use a `match` expression",
            );
            return Err(());
        }

        if let Some(operator) = self.match_punctuation("??") {
            self.error_at(operator.span, "`??` was removed; use `otherwise { ... }`");
            return Err(());
        }

        if let Some(keyword) = self.match_keyword(Keyword::Otherwise) {
            let fallback = self.parse_block()?;
            return Ok(Expr::Otherwise(OtherwiseExpr {
                span: self.span(left.span().start, fallback.span.end),
                keyword_span: keyword.span,
                value: Box::new(left),
                fallback,
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

    pub(super) fn parse_prefix_expression(&mut self) -> ParseResult<Expr> {
        if let Some(operator) = self.match_punctuation("&+") {
            let expression = self.parse_prefix_expression()?;
            return Ok(Expr::Borrow(BorrowExpr {
                span: self.span(operator.span.start, expression.span().end),
                operator_span: operator.span,
                is_readwrite: true,
                expression: Box::new(expression),
            }));
        }

        if let Some(operator) = self.match_punctuation("&") {
            let expression = self.parse_prefix_expression()?;
            return Ok(Expr::Borrow(BorrowExpr {
                span: self.span(operator.span.start, expression.span().end),
                operator_span: operator.span,
                is_readwrite: false,
                expression: Box::new(expression),
            }));
        }

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

            if self.at_punctuation("?") && self.next_is_punctuation("{") {
                break;
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
                let error = self.expect_name_identifier("expected catch binding name")?;
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

            if self.at_ellipsis() {
                self.error_at(self.ellipsis_span(), "`...` spread is not part of v0");
                return Err(());
            }
            if self.current().kind == TokenKind::Identifier && self.next_is_punctuation(":") {
                self.error_current("named arguments are not part of v0");
                return Err(());
            }
            arguments.push(self.parse_expression()?);
            self.skip_newlines();
            let Some(comma) = self.match_punctuation(",") else {
                break;
            };
            if self.at_punctuation(")") {
                self.error_at(
                    comma.span,
                    "trailing commas in single-line argument lists are not part of v0",
                );
                return Err(());
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

            if self.at_ellipsis() {
                self.error_at(self.ellipsis_span(), "`...` spread is not part of v0");
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
                if self.looks_like_typed_sequence_literal_start() {
                    let target = self.parse_type()?;
                    return self.finish_typed_sequence_literal(target);
                }
                if self.looks_like_typed_string_literal_start() {
                    let target = self.parse_type()?;
                    return self.finish_typed_string_literal(target);
                }
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
                if self.looks_like_generic_struct_literal_body() {
                    let (arguments, arguments_span) = self.parse_type_argument_list()?;
                    return self.finish_struct_literal_expression(TypeExpr::Generic(GenericType {
                        span: self.span(token.span.start, arguments_span.end),
                        name,
                        name_span: token.span,
                        arguments,
                    }));
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
            TokenKind::ByteLiteral => {
                let token = self.bump();
                Ok(Expr::ByteLiteral(LiteralExpr {
                    span: token.span,
                    value: self.lexeme(&token),
                }))
            }
            TokenKind::StringLiteral => {
                let token = self.bump();
                self.parse_string_literal_expression(token)
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
            TokenKind::Keyword(Keyword::If) => self.parse_if_expression(),
            TokenKind::Keyword(Keyword::Match) => self.parse_match_expression(),
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
            TokenKind::Punctuation(".") if self.at_ellipsis() => {
                self.error_at(self.ellipsis_span(), "`...` spread is not part of v0");
                Err(())
            }
            TokenKind::Punctuation(".") if self.at_relative_module_path_expression() => {
                self.error_at(
                    self.current().span,
                    "module paths are only valid in `use` declarations; introduce the module namespace with `use` before using it",
                );
                Err(())
            }
            TokenKind::Punctuation("/") if self.at_absolute_module_path_expression() => {
                self.error_at(
                    self.current().span,
                    "module paths are only valid in `use` declarations; introduce the module namespace with `use` before using it",
                );
                Err(())
            }
            _ => {
                self.error_current("expected expression");
                Err(())
            }
        }
    }

    fn at_relative_module_path_expression(&self) -> bool {
        self.at_punctuation(".")
            && (self.current_touches_next_punctuation("/")
                || (self.current_touches_next_punctuation(".")
                    && self.punctuation_at_offset(2, "/")))
    }

    fn at_absolute_module_path_expression(&self) -> bool {
        self.at_punctuation("/") && self.current_touches_next_identifier()
    }

    pub(super) fn parse_string_literal_expression(&mut self, token: Token) -> ParseResult<Expr> {
        let value = self.lexeme(&token);
        let parts = match string_literal_parts(&value) {
            Ok(parts) => parts,
            Err(error) => {
                self.error_at(
                    self.span(token.span.start + error.start, token.span.start + error.end),
                    error.message,
                );
                return Err(());
            }
        };

        if parts
            .iter()
            .all(|part| matches!(part, StringLiteralPartSpan::Text { .. }))
        {
            return Ok(Expr::StringLiteral(LiteralExpr {
                span: token.span,
                value,
            }));
        }

        let mut ast_parts = Vec::with_capacity(parts.len());
        for part in parts {
            match part {
                StringLiteralPartSpan::Text { start, end } => {
                    let decoded = match decode_interpolated_text_part(&value, start, end) {
                        Ok(decoded) => decoded,
                        Err(message) => {
                            self.error_at(
                                self.span(token.span.start + start, token.span.start + end),
                                message,
                            );
                            return Err(());
                        }
                    };
                    ast_parts.push(InterpolatedStringPart::Text(InterpolatedStringText {
                        span: self.span(token.span.start + start, token.span.start + end),
                        value: decoded,
                    }));
                }
                StringLiteralPartSpan::Interpolation {
                    start,
                    expression_start,
                    expression_end,
                    end,
                } => {
                    let expression_span = self.span(
                        token.span.start + expression_start,
                        token.span.start + expression_end,
                    );
                    let expression = self.parse_interpolation_expression(expression_span)?;
                    ast_parts.push(InterpolatedStringPart::Expression(
                        InterpolatedStringExpression {
                            span: self.span(token.span.start + start, token.span.start + end),
                            expression_span,
                            expression: Box::new(expression),
                        },
                    ));
                }
            }
        }

        Ok(Expr::InterpolatedString(InterpolatedStringExpr {
            span: token.span,
            value,
            parts: ast_parts,
        }))
    }

    fn parse_interpolation_expression(&mut self, span: ByteSpan) -> ParseResult<Expr> {
        let lexed = lex_span(self.sources, span);
        if !lexed.diagnostics.is_empty() {
            self.diagnostics.extend(lexed.diagnostics);
            return Err(());
        }

        let mut parser = Parser {
            sources: self.sources,
            source: span.source,
            tokens: &lexed.tokens,
            index: 0,
            pending_token: None,
            diagnostics: Vec::new(),
            literal_pack_capture: None,
        };
        parser.skip_newlines();
        let expression = match parser.parse_expression() {
            Ok(expression) => expression,
            Err(()) => {
                self.diagnostics.extend(parser.diagnostics);
                return Err(());
            }
        };
        parser.skip_newlines();
        if !parser.at_eof() {
            parser.error_current("expected end of string interpolation expression");
            self.diagnostics.extend(parser.diagnostics);
            return Err(());
        }

        self.diagnostics.extend(parser.diagnostics);
        Ok(expression)
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

            if self.at_ellipsis() {
                self.error_at(self.ellipsis_span(), "`...` spread is not part of v0");
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
