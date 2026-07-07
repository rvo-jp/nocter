use super::support::with_type_span;
use super::{ParseResult, Parser};
use crate::ast::{
    ArrayLength, ArrayType, BorrowType, FallibleType, GenericParam, GenericParamList, GenericType,
    OptionalType, PointerType, TypeExpr, TypeReference, ViewType,
};
use crate::lexer::Keyword;
use crate::source::ByteSpan;

impl Parser<'_> {
    pub(super) fn parse_type(&mut self) -> ParseResult<TypeExpr> {
        let mut ty = self.parse_type_atom()?;

        loop {
            if let Some(question) = self.match_punctuation("?") {
                ty = TypeExpr::Optional(OptionalType {
                    span: self.span(ty.span().start, question.span.end),
                    inner: Box::new(ty),
                });
                continue;
            }

            if let Some(bang) = self.match_punctuation("!") {
                let error = TypeExpr::Reference(TypeReference {
                    span: bang.span,
                    name: "error".to_string(),
                });
                ty = TypeExpr::Fallible(FallibleType {
                    span: self.span(ty.span().start, bang.span.end),
                    success: Box::new(ty),
                    error: Box::new(error),
                });
                continue;
            }

            break;
        }

        Ok(ty)
    }

    pub(super) fn parse_type_atom(&mut self) -> ParseResult<TypeExpr> {
        if let Some(star) = self.match_punctuation("*") {
            let inner = self.parse_type_atom()?;
            return Ok(TypeExpr::Pointer(PointerType {
                span: self.span(star.span.start, inner.span().end),
                inner: Box::new(inner),
            }));
        }

        if let Some(borrow) = self.match_punctuation("&+") {
            let inner = self.parse_type_atom()?;
            return Ok(TypeExpr::Borrow(BorrowType {
                span: self.span(borrow.span.start, inner.span().end),
                is_readwrite: true,
                inner: Box::new(inner),
            }));
        }

        if let Some(borrow) = self.match_punctuation("&") {
            let inner = self.parse_type_atom()?;
            return Ok(TypeExpr::Borrow(BorrowType {
                span: self.span(borrow.span.start, inner.span().end),
                is_readwrite: false,
                inner: Box::new(inner),
            }));
        }

        if let Some(open) = self.match_punctuation("[") {
            let plus = self.match_punctuation("+");
            let element = self.parse_type()?;
            if self.match_punctuation(";").is_some() {
                if let Some(plus) = plus {
                    self.error_at(plus.span, "`+` is not valid in a fixed-size array type");
                    return Err(());
                }

                let length = self.expect_integer_literal("expected array length after `;`")?;
                let close = self.expect_punctuation("]", "`]`")?;
                return Ok(TypeExpr::Array(ArrayType {
                    span: self.span(open.span.start, close.span.end),
                    element: Box::new(element),
                    length: ArrayLength {
                        span: length.span,
                        value: self.lexeme(&length),
                    },
                }));
            }

            let close = self.expect_punctuation("]", "`]`")?;
            return Ok(TypeExpr::View(ViewType {
                span: self.span(open.span.start, close.span.end),
                is_readwrite: plus.is_some(),
                element: Box::new(element),
            }));
        }

        if let Some(open) = self.match_punctuation("(") {
            let inner = self.parse_type()?;
            let close = self.expect_punctuation(")", "`)`")?;
            return Ok(with_type_span(
                inner,
                self.span(open.span.start, close.span.end),
            ));
        }

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
        if self.at_punctuation("<") {
            let (arguments, arguments_span) = self.parse_type_argument_list()?;
            return Ok(TypeExpr::Generic(GenericType {
                span: self.span(name.span.start, arguments_span.end),
                name: name.value,
                name_span: name.span,
                arguments,
            }));
        }

        Ok(TypeExpr::Reference(TypeReference {
            span: name.span,
            name: name.value,
        }))
    }

    pub(super) fn parse_type_argument_list(&mut self) -> ParseResult<(Vec<TypeExpr>, ByteSpan)> {
        let start = self.expect_punctuation("<", "`<`")?;
        let mut arguments = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation(">") {
            if self.at_eof() {
                self.error_current("expected `>` to close type argument list");
                return Err(());
            }

            arguments.push(self.parse_type()?);
            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }

        let end = self.expect_punctuation(">", "`>`")?;
        Ok((arguments, self.span(start.span.start, end.span.end)))
    }

    pub(super) fn parse_generic_param_list(&mut self) -> ParseResult<GenericParamList> {
        let Some(start) = self.match_punctuation("<") else {
            return Ok(GenericParamList::empty());
        };
        let mut parameters = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation(">") {
            if self.at_eof() {
                self.error_current("expected `>` to close generic parameter list");
                return Err(());
            }

            let parameter = self.expect_identifier("expected generic parameter name")?;
            let bound = if self.match_punctuation(":").is_some() {
                Some(self.parse_type()?)
            } else {
                None
            };
            let end = bound
                .as_ref()
                .map_or(parameter.span.end, |bound| bound.span().end);
            parameters.push(GenericParam {
                span: self.span(parameter.span.start, end),
                name: parameter.value,
                bound,
            });

            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }

        let end = self.expect_punctuation(">", "`>`")?;
        Ok(GenericParamList {
            span: Some(self.span(start.span.start, end.span.end)),
            parameters,
        })
    }
}
