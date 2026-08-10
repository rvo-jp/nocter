use super::*;

impl Parser<'_> {
    pub(super) fn parse_instance_decl(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Instance, "`instance`")?;
        let generics = self.parse_generic_param_list()?;
        let target_ty = self.parse_type()?;
        let requirements = self.parse_where_clause()?;
        let open = self.expect_punctuation("{", "`{`")?;
        let mut members = Vec::new();
        let mut has_drop_member = false;
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close instance block");
                return Err(());
            }
            let visibility = self.parse_visibility()?;
            self.reject_removed_result_allocation_modifier()?;
            if self.at_keyword(Keyword::Func) {
                self.error_current(
                    "associated `func` declarations are written at top level as `func Type.name(...)`",
                );
                return Err(());
            } else if self.at_keyword(Keyword::Method) {
                members.push(InstanceMember::Method(
                    self.parse_method_decl(visibility, true)?,
                ));
            } else if self.at_identifier_text("drop") {
                if visibility != Visibility::Private {
                    self.error_current("drop member cannot be marked pub");
                    return Err(());
                }
                if has_drop_member {
                    self.error_current("instance block cannot define more than one drop member");
                    return Err(());
                }
                has_drop_member = true;
                members.push(InstanceMember::Drop(self.parse_drop_decl()?));
            } else {
                self.error_current("expected `method` or `drop` in instance block");
                return Err(());
            }
            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Item::Instance(InstanceDecl {
            span: self.span(start.span.start, close.span.end),
            generics,
            target_ty,
            requirements,
            members,
        }))
    }

    fn parse_drop_decl(&mut self) -> ParseResult<DropDecl> {
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
}

fn readwrite_self_borrow_type(span: ByteSpan) -> TypeExpr {
    TypeExpr::Borrow(BorrowType {
        span,
        is_readwrite: true,
        inner: Box::new(TypeExpr::Reference(TypeReference {
            span: ByteSpan::new(span.source, span.end - "self".len(), span.end),
            name: "Self".to_string(),
        })),
    })
}
