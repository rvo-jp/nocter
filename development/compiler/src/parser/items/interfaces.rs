use super::*;

impl Parser<'_> {
    pub(super) fn parse_interface_decl(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Interface, "`interface`")?;
        let name = self.expect_name_identifier("expected interface name after `interface`")?;
        let generics = self.parse_generic_param_list()?;
        let requirements = self.parse_where_clause()?;
        let open = self.expect_punctuation("{", "`{`")?;
        let mut methods = Vec::new();
        let mut associated_types = Vec::new();
        self.skip_newlines();
        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close interface declaration");
                return Err(());
            }
            let member_visibility = self.parse_visibility()?;
            if member_visibility != Visibility::Public {
                self.error_current("interface members must be marked `pub`");
                return Err(());
            }
            if self.at_keyword(Keyword::Type) {
                associated_types.push(self.parse_associated_type_decl()?);
            } else {
                self.reject_removed_result_allocation_modifier()?;
                if !self.at_keyword(Keyword::Method) {
                    self.error_current(
                        "expected `pub type` or `pub method` in interface declaration",
                    );
                    return Err(());
                }
                methods.push(self.parse_method_decl(member_visibility, false)?);
            }
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
            requirements,
            associated_types,
            methods,
        }))
    }

    fn parse_associated_type_decl(&mut self) -> ParseResult<AssociatedTypeDecl> {
        let start = self.expect_keyword(Keyword::Type, "`type`")?;
        let name = self.expect_name_identifier("expected associated type name after `type`")?;
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
            .map_or(name.span.end, |bound| bound.span().end);
        Ok(AssociatedTypeDecl {
            span: self.span(start.span.start, end),
            name: name.value,
            name_span: name.span,
            bounds,
        })
    }
}
