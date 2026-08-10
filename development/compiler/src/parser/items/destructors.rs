use super::*;

impl Parser<'_> {
    pub(super) fn parse_destruct_decl(&mut self) -> ParseResult<Item> {
        let keyword = self.expect_keyword(Keyword::Destruct, "`destruct`")?;
        self.reject_declaration_pattern_prefix("destruct")?;
        let target_ty = self.parse_type()?;
        let generics = self.declaration_pattern_parameters(&[&target_ty])?;
        self.expect_punctuation("(", "`(` before destructor receiver")?;
        let binding = self.parse_destruct_receiver()?;
        self.expect_punctuation(")", "`)` after destructor receiver")?;
        if self.at_identifier_text("where") {
            self.error_current("destruct declarations cannot have a `where` clause");
            return Err(());
        }
        let body = self.parse_block()?;
        Ok(Item::Destruct(DestructDecl {
            span: self.span(keyword.span.start, body.span.end),
            keyword_span: keyword.span,
            generics,
            target_ty,
            binding,
            body,
        }))
    }

    fn parse_destruct_receiver(&mut self) -> ParseResult<Parameter> {
        let borrow = self.expect_punctuation("&+", "`&+self`")?;
        let self_span =
            self.expect_self_identifier("expected `self` after `&+` in destruct declaration")?;
        let span = self.span(borrow.span.start, self_span.end);
        Ok(Parameter {
            span,
            name: "self".to_string(),
            name_span: self_span,
            ty: TypeExpr::Borrow(BorrowType {
                span,
                is_readwrite: true,
                inner: Box::new(TypeExpr::Reference(TypeReference {
                    span: self_span,
                    name: "Self".to_string(),
                })),
            }),
        })
    }
}
