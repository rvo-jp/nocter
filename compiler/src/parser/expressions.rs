use super::{ParseResult, Parser};
use crate::ast::{
    ArrayLiteralExpr, BinaryExpr, CallExpr, CatchExpr, Expr, ForceExpr, GroupExpr, IdentifierExpr,
    IndexExpr, LiteralExpr, MemberExpr, OptionalDefaultExpr, PropagationExpr, StructLiteralExpr,
    StructLiteralField, TypeConversionExpr, TypeExpr, TypeReference, UnaryExpr,
};
use crate::lexer::{Keyword, TokenKind};

impl Parser<'_> {
    pub(super) fn parse_expression(&mut self) -> ParseResult<Expr> {
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
