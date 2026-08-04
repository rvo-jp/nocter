use super::*;

impl Parser<'_> {
    pub(super) fn parse_impl_decl(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Impl, "`impl`")?;
        let generics = self.parse_generic_param_list()?;
        let first_ty = self.parse_type()?;
        if self.match_keyword(Keyword::For).is_some() {
            let target_ty = self.parse_type()?;
            let mut end = target_ty.span().end;
            if self.match_punctuation("{").is_some() {
                self.skip_newlines();
                if !self.at_punctuation("}") {
                    self.error_current(
                        "interface conformance impl cannot contain members; define methods in an inherent `impl Type` block",
                    );
                    return Err(());
                }
                let close = self.expect_punctuation("}", "`}`")?;
                end = close.span.end;
            }

            return Ok(Item::Impl(ImplDecl {
                span: self.span(start.span.start, end),
                generics,
                interface_ty: Some(first_ty),
                target_ty,
                members: Vec::new(),
            }));
        }
        let target_ty = first_ty;
        let open = self.expect_punctuation("{", "`{`")?;
        let mut members = Vec::new();
        let mut has_drop_member = false;
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close impl block");
                return Err(());
            }

            let visibility = self.parse_visibility()?;
            if self.at_keyword(Keyword::Func) {
                self.error_current(
                    "`func` declarations are written at top level as `func Type.name(...)` in v0",
                );
                return Err(());
            } else if self.at_keyword(Keyword::Method) {
                members.push(ImplMember::Method(
                    self.parse_method_decl(visibility, true)?,
                ));
            } else if self.at_identifier_text("drop") {
                if visibility != Visibility::Private {
                    self.error_current("drop member cannot be marked pub");
                    return Err(());
                }
                if has_drop_member {
                    self.error_current("impl block cannot define more than one drop member");
                    return Err(());
                }
                has_drop_member = true;
                members.push(ImplMember::Drop(self.parse_drop_decl()?));
            } else {
                self.error_current("expected `method` or `drop` in impl block");
                return Err(());
            }

            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Item::Impl(ImplDecl {
            span: self.span(start.span.start, close.span.end),
            generics,
            interface_ty: None,
            target_ty,
            members,
        }))
    }

    pub(super) fn parse_interface_decl(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Interface, "`interface`")?;
        let name = self.expect_name_identifier("expected interface name after `interface`")?;
        let generics = self.parse_generic_param_list()?;
        let open = self.expect_punctuation("{", "`{`")?;
        let mut methods = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close interface declaration");
                return Err(());
            }

            let method_visibility = self.parse_visibility()?;
            if method_visibility != Visibility::Public {
                self.error_current("interface members must be marked `pub`");
                return Err(());
            }
            if !self.at_keyword(Keyword::Method) {
                self.error_current("expected `pub method` in interface declaration");
                return Err(());
            }
            methods.push(self.parse_method_decl(method_visibility, false)?);

            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Item::Interface(InterfaceDecl {
            span: self.span(
                target
                    .as_ref()
                    .map_or(start.span.start, |target| target.span.start),
                close.span.end,
            ),
            visibility,
            target,
            name: name.value,
            name_span: name.span,
            generics,
            methods,
        }))
    }

    pub(super) fn parse_drop_decl(&mut self) -> ParseResult<DropDecl> {
        let start = self.bump();
        let binding = self.parse_drop_receiver()?;
        let body = self.parse_block()?;

        Ok(DropDecl {
            span: self.span(start.span.start, body.span.end),
            name_span: start.span,
            binding,
            body,
        })
    }

    pub(super) fn parse_method_decl(
        &mut self,
        visibility: Visibility,
        require_body: bool,
    ) -> ParseResult<MethodDecl> {
        let start = self.expect_keyword(Keyword::Method, "`method`")?;
        let receiver = self.parse_method_receiver()?;
        self.expect_punctuation(".", "`.`")?;
        let name = self.expect_name_identifier("expected method name after `.`")?;
        let generics = self.parse_generic_param_list()?;
        let parameters = self.parse_parameter_list()?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let result_provenance = self.parse_result_provenance_clause()?;
        let body = if require_body || self.at_punctuation("{") {
            Some(self.parse_block()?)
        } else {
            None
        };
        let end = body.as_ref().map_or_else(
            || {
                result_provenance
                    .as_ref()
                    .map_or(return_type.span().end, |clause| clause.span.end)
            },
            |body| body.span.end,
        );

        Ok(MethodDecl {
            span: self.span(start.span.start, end),
            visibility,
            receiver,
            name: name.value,
            name_span: name.span,
            generics,
            parameters,
            return_type,
            result_provenance,
            body,
        })
    }

    pub(super) fn parse_method_receiver(&mut self) -> ParseResult<MethodReceiver> {
        self.parse_self_receiver("expected `self`, `&self`, or `&+self` receiver after `method`")
    }

    fn parse_drop_receiver(&mut self) -> ParseResult<Parameter> {
        let borrow = self.expect_punctuation("&+", "`&+self`")?;
        let self_span = self.expect_self_identifier("expected `self` after `&+` in drop member")?;
        let ty = readwrite_self_borrow_type(self.span(borrow.span.start, self_span.end));

        Ok(Parameter {
            span: ty.span(),
            name: "self".to_string(),
            name_span: self_span,
            ty,
        })
    }

    fn parse_self_receiver(&mut self, message: &'static str) -> ParseResult<MethodReceiver> {
        let borrow = self
            .match_punctuation("&+")
            .map(|token| (token, true))
            .or_else(|| self.match_punctuation("&").map(|token| (token, false)));
        let self_span = self.expect_self_identifier(message)?;
        let (span, mode) = if let Some((borrow, is_readwrite)) = borrow {
            (
                self.span(borrow.span.start, self_span.end),
                if is_readwrite {
                    MethodReceiverMode::ReadwriteBorrow
                } else {
                    MethodReceiverMode::ReadonlyBorrow
                },
            )
        } else {
            (self_span, MethodReceiverMode::Owned)
        };

        Ok(MethodReceiver {
            span,
            name: "self".to_string(),
            name_span: self_span,
            mode,
        })
    }

    fn expect_self_identifier(&mut self, message: impl Into<String>) -> ParseResult<ByteSpan> {
        let message = message.into();
        let identifier = self.expect_identifier(&message)?;
        if identifier.value != "self" {
            self.error_at(identifier.span, "receiver name must be `self`");
            return Err(());
        }
        Ok(identifier.span)
    }
}

fn self_type(span: ByteSpan) -> TypeExpr {
    TypeExpr::Reference(TypeReference {
        span,
        name: "Self".to_string(),
    })
}

fn readwrite_self_borrow_type(span: ByteSpan) -> TypeExpr {
    TypeExpr::Borrow(BorrowType {
        span,
        is_readwrite: true,
        inner: Box::new(self_type(ByteSpan::new(
            span.source,
            span.end - "self".len(),
            span.end,
        ))),
    })
}
