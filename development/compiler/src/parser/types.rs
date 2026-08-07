use super::support::with_type_span;
use super::{ParseResult, Parser};
use crate::ast::{
    ArrayLength, ArrayType, BorrowType, CallableCapability, CallableTypeExpr,
    CallableTypeParameter, FallibleType, GenericParam, GenericParamList, GenericType, OptionalType,
    PointerType, TypeExpr, TypeReference, ViewType,
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
        let result_allocation = if self.at_identifier_text("alloc")
            && (self.token_at_offset_is_keyword(1, Keyword::Func)
                || ((self.punctuation_at_offset(1, "&") || self.punctuation_at_offset(1, "&+"))
                    && self.token_at_offset_is_keyword(2, Keyword::Func)))
        {
            self.parse_optional_result_allocation_modifier()
        } else {
            None
        };

        if self.at_keyword(Keyword::Func) {
            return self.parse_callable_type(
                None,
                CallableCapability::Consuming,
                result_allocation,
            );
        }

        if self.at_punctuation("&+") && self.next_is_keyword(Keyword::Func) {
            let prefix = self.bump();
            return self.parse_callable_type(
                Some(prefix.span),
                CallableCapability::Readwrite,
                result_allocation,
            );
        }

        if self.at_punctuation("&") && self.next_is_keyword(Keyword::Func) {
            let prefix = self.bump();
            return self.parse_callable_type(
                Some(prefix.span),
                CallableCapability::Readonly,
                result_allocation,
            );
        }

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

            if let Some(plus) = plus {
                self.error_at(
                    plus.span,
                    "`+` is not valid inside an unsized array data type; use `&+[T]` for a mutable array slice",
                );
                return Err(());
            }

            let close = self.expect_punctuation("]", "`]`")?;
            return Ok(TypeExpr::View(ViewType {
                span: self.span(open.span.start, close.span.end),
                is_readwrite: false,
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

    fn parse_callable_type(
        &mut self,
        prefix_span: Option<ByteSpan>,
        capability: CallableCapability,
        result_allocation: Option<crate::ast::ResultAllocationModifier>,
    ) -> ParseResult<TypeExpr> {
        let func = self.expect_keyword(Keyword::Func, "`func`")?;
        let open = self.expect_punctuation("(", "`(`")?;
        let mut parameters = Vec::new();
        self.skip_newlines();
        while !self.at_punctuation(")") {
            if self.at_eof() {
                self.error_current("expected `)` to close callable parameter list");
                return Err(());
            }
            let start = self.current().span.start;
            let (name, name_span) = if self.current().kind == crate::lexer::TokenKind::Identifier
                && self.next_is_punctuation(":")
            {
                let name = self.bump();
                self.expect_punctuation(":", "`:`")?;
                (Some(self.lexeme(&name)), Some(name.span))
            } else {
                (None, None)
            };
            let ty = self.parse_type()?;
            parameters.push(CallableTypeParameter {
                span: self.span(start, ty.span().end),
                name,
                name_span,
                ty,
            });
            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }
        let close = self.expect_punctuation(")", "`)`")?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let result_provenance = self.parse_result_provenance_clause()?;
        let start = result_allocation.map_or_else(
            || prefix_span.map_or(func.span.start, |span| span.start),
            |modifier| modifier.span.start,
        );
        let end = result_provenance
            .as_ref()
            .map_or(return_type.span().end, |clause| clause.span.end);
        Ok(TypeExpr::Callable(CallableTypeExpr {
            span: self.span(start, end),
            func_span: func.span,
            result_allocation,
            capability,
            parameters_span: self.span(open.span.start, close.span.end),
            parameters,
            return_type: Box::new(return_type),
            result_provenance,
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

            let parameter = self.expect_name_identifier("expected generic parameter name")?;
            let mut bounds = Vec::new();
            if self.match_punctuation(":").is_some() {
                loop {
                    bounds.push(self.parse_type()?);
                    if self.match_punctuation("+").is_none() {
                        break;
                    }
                }
            }
            let end = bounds
                .last()
                .map_or(parameter.span.end, |bound| bound.span().end);
            parameters.push(GenericParam {
                span: self.span(parameter.span.start, end),
                name: parameter.value,
                name_span: parameter.span,
                bounds,
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
