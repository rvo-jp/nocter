use super::support::with_type_span;
use super::{ParseResult, Parser};
use crate::ast::{
    ArrayLength, ArrayType, BorrowType, CallableCapability, CallableTypeExpr,
    CallableTypeParameter, FallibleType, GenericParam, GenericParamList, GenericType,
    OpaqueAssociatedTypeBinding, OpaqueType, OptionalType, PointerType, ProjectedType, TypeExpr,
    TypeReference, ViewType,
};
use crate::lexer::Keyword;
use crate::source::ByteSpan;

impl Parser<'_> {
    pub(super) fn parse_type(&mut self) -> ParseResult<TypeExpr> {
        let mut ty = self.parse_type_atom_with_projections()?;

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

    fn parse_type_atom_with_projections(&mut self) -> ParseResult<TypeExpr> {
        let mut ty = self.parse_type_atom()?;
        while self.at_punctuation(".") && self.next_is_identifier() {
            self.bump();
            let name = self.expect_name_identifier("expected associated type name after `.`")?;
            ty = TypeExpr::Projection(ProjectedType {
                span: self.span(ty.span().start, name.span.end),
                base: Box::new(ty),
                name: name.value,
                name_span: name.span,
            });
        }
        Ok(ty)
    }

    pub(super) fn parse_type_atom(&mut self) -> ParseResult<TypeExpr> {
        self.reject_removed_result_allocation_modifier()?;

        if self.at_identifier_text("some") {
            return self.parse_opaque_type();
        }

        if self.at_keyword(Keyword::Func) {
            return self.parse_callable_type(None, CallableCapability::Consuming);
        }

        if self.at_punctuation("&+") && self.next_is_keyword(Keyword::Func) {
            let prefix = self.bump();
            return self.parse_callable_type(Some(prefix.span), CallableCapability::Readwrite);
        }

        if self.at_punctuation("&") && self.next_is_keyword(Keyword::Func) {
            let prefix = self.bump();
            return self.parse_callable_type(Some(prefix.span), CallableCapability::Readonly);
        }

        if let Some(star) = self.match_punctuation("*") {
            let inner = self.parse_type_atom_with_projections()?;
            return Ok(TypeExpr::Pointer(PointerType {
                span: self.span(star.span.start, inner.span().end),
                inner: Box::new(inner),
            }));
        }

        if let Some(borrow) = self.match_punctuation("&+") {
            let inner = self.parse_type_atom_with_projections()?;
            return Ok(TypeExpr::Borrow(BorrowType {
                span: self.span(borrow.span.start, inner.span().end),
                is_readwrite: true,
                inner: Box::new(inner),
            }));
        }

        if let Some(borrow) = self.match_punctuation("&") {
            let inner = self.parse_type_atom_with_projections()?;
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

    fn parse_opaque_type(&mut self) -> ParseResult<TypeExpr> {
        let some = self.bump();
        let name = self.expect_identifier("expected interface name after `some`")?;
        let mut arguments = Vec::new();
        let mut associated_bindings = Vec::new();
        let mut end = name.span.end;

        if self.match_punctuation("<").is_some() {
            if self.at_punctuation(">") {
                self.error_at(
                    self.current().span,
                    "opaque interface arguments cannot be empty",
                );
                return Err(());
            }
            loop {
                if self.current().kind == crate::lexer::TokenKind::Identifier
                    && self.next_is_punctuation("=")
                {
                    let binding_name =
                        self.expect_identifier("expected associated type binding name")?;
                    self.expect_punctuation("=", "`=`")?;
                    let value = self.parse_type()?;
                    associated_bindings.push(OpaqueAssociatedTypeBinding {
                        span: self.span(binding_name.span.start, value.span().end),
                        name: binding_name.value,
                        name_span: binding_name.span,
                        value,
                    });
                } else {
                    if !associated_bindings.is_empty() {
                        self.error_at(
                            self.current().span,
                            "interface type arguments must precede associated type bindings",
                        );
                        return Err(());
                    }
                    arguments.push(self.parse_type()?);
                }

                if self.match_punctuation(",").is_some() {
                    if self.at_punctuation(">") {
                        break;
                    }
                    continue;
                }
                break;
            }
            let close = self.expect_punctuation(">", "`>`")?;
            end = close.span.end;
        }

        let interface = if arguments.is_empty() {
            TypeExpr::Reference(TypeReference {
                span: name.span,
                name: name.value,
            })
        } else {
            TypeExpr::Generic(GenericType {
                span: self.span(name.span.start, end),
                name: name.value,
                name_span: name.span,
                arguments,
            })
        };

        Ok(TypeExpr::Opaque(OpaqueType {
            span: self.span(some.span.start, end),
            some_span: some.span,
            interface: Box::new(interface),
            associated_bindings,
            witness: None,
        }))
    }

    fn parse_callable_type(
        &mut self,
        prefix_span: Option<ByteSpan>,
        capability: CallableCapability,
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
        let start = prefix_span.map_or(func.span.start, |span| span.start);
        let end = result_provenance
            .as_ref()
            .map_or(return_type.span().end, |clause| clause.span.end);
        Ok(TypeExpr::Callable(CallableTypeExpr {
            span: self.span(start, end),
            func_span: func.span,
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

            if self.at_identifier_text("copy") {
                self.error_current(
                    "inline `copy` generic constraints were removed; declare the parameter name and write `where copy T`",
                );
                return Err(());
            }
            let parameter = self.expect_name_identifier("expected generic parameter name")?;
            if let Some(colon) = self.match_punctuation(":") {
                self.error_at(
                    colon.span,
                    "inline generic constraints were removed; declare names in `<...>` and write constraints in a `where` clause",
                );
                return Err(());
            }
            parameters.push(GenericParam {
                span: parameter.span,
                name: parameter.value,
                name_span: parameter.span,
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
