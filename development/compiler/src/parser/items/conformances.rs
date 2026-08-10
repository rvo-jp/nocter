use super::*;

impl Parser<'_> {
    pub(super) fn parse_conformance_decl(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Conform, "`conform`")?;
        self.reject_declaration_pattern_prefix("conform")?;
        let interface_ty = self.parse_type()?;
        self.expect_keyword(Keyword::For, "`for` after interface type")?;
        let target_ty = self.parse_type()?;
        let generics = self.declaration_pattern_parameters(&[&interface_ty, &target_ty])?;
        let mut requirements = self.parse_where_clause()?;
        self.classify_declaration_pattern_refinements(&mut requirements, &generics);
        let (members, end) = self.parse_conformance_members()?;
        Ok(Item::Conformance(ConformanceDecl {
            span: self.span(start.span.start, end),
            generics,
            interface_ty,
            target_ty,
            requirements,
            members,
        }))
    }

    fn parse_conformance_members(&mut self) -> ParseResult<(Vec<ConformanceMember>, usize)> {
        let open = self.expect_punctuation("{", "`{` after conformance target")?;
        let mut members = Vec::new();
        self.skip_newlines();
        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close conform block");
                return Err(());
            }
            let visibility = self.parse_visibility()?;
            if visibility != Visibility::Private {
                self.error_current(
                    "conformance members inherit visibility and cannot be marked `pub`",
                );
                return Err(());
            }
            self.reject_removed_result_allocation_modifier()?;
            if self.at_keyword(Keyword::Type) {
                members.push(ConformanceMember::AssociatedType(
                    self.parse_associated_type_binding()?,
                ));
            } else if self.at_keyword(Keyword::Method) {
                members.push(ConformanceMember::Method(
                    self.parse_method_decl(Visibility::Private, true)?,
                ));
            } else {
                self.error_current("expected `type` or `method` in conform block");
                return Err(());
            }
            self.skip_newlines();
        }
        let close = self.expect_punctuation("}", "`}`")?;
        Ok((members, close.span.end))
    }

    fn parse_associated_type_binding(&mut self) -> ParseResult<AssociatedTypeBinding> {
        let start = self.expect_keyword(Keyword::Type, "`type`")?;
        let name = self.expect_name_identifier("expected associated type name after `type`")?;
        self.expect_punctuation("=", "`=` after associated type name")?;
        let value = self.parse_type()?;
        Ok(AssociatedTypeBinding {
            span: self.span(start.span.start, value.span().end),
            name: name.value,
            name_span: name.span,
            value,
        })
    }
}
