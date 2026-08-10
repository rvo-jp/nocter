use super::*;

impl Parser<'_> {
    pub(super) fn parse_type_alias_decl(
        &mut self,
        visibility: Visibility,
        target_directive: Option<TargetDirective>,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Type, "`type`")?;
        let name = self.expect_name_identifier("expected type alias name after `type`")?;
        let generics = self.parse_generic_param_list()?;
        self.expect_punctuation("=", "`=`")?;
        let target = self.parse_type()?;
        let requirements = self.parse_where_clause()?;
        let end = requirements
            .as_ref()
            .map_or(target.span().end, |clause| clause.span.end);

        Ok(Item::TypeAlias(TypeAliasDecl {
            span: self.span(
                target_directive
                    .as_ref()
                    .map_or(start.span.start, |target| target.span.start),
                end,
            ),
            visibility,
            target_directive,
            name: name.value,
            name_span: name.span,
            generics,
            target,
            requirements,
        }))
    }

    pub(super) fn parse_struct_decl(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
        is_copy: bool,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Struct, "`struct`")?;
        let name = self.expect_name_identifier("expected struct name after `struct`")?;
        let generics = self.parse_generic_param_list()?;
        let requirements = self.parse_where_clause()?;
        let fields = self.parse_struct_fields()?;
        let end = fields.0.end;

        Ok(Item::Struct(StructDecl {
            span: self.span(
                target
                    .as_ref()
                    .map_or(start.span.start, |target| target.span.start),
                end,
            ),
            visibility,
            target,
            is_copy,
            name: name.value,
            name_span: name.span,
            generics,
            requirements,
            fields: fields.1,
        }))
    }

    pub(super) fn parse_struct_fields(&mut self) -> ParseResult<(ByteSpan, Vec<StructField>)> {
        let start = self.expect_punctuation("{", "`{`")?;
        let mut fields = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_current("expected `}` to close struct declaration");
                return Err(());
            }

            let visibility = self.parse_visibility()?;
            if self.at_ellipsis() {
                self.error_at(
                    self.ellipsis_span(),
                    "embedding declarations are not supported",
                );
                return Err(());
            }
            let name = self.expect_name_identifier("expected struct field name")?;
            self.expect_punctuation(":", "`:`")?;
            let ty = self.parse_type()?;
            fields.push(StructField {
                span: self.span(name.span.start, ty.span().end),
                visibility,
                name: name.value,
                name_span: name.span,
                ty,
            });

            self.skip_newlines();
            _ = self.match_punctuation(",");
            self.skip_newlines();
        }

        let end = self.expect_punctuation("}", "`}`")?;
        Ok((self.span(start.span.start, end.span.end), fields))
    }

    pub(super) fn parse_enum_decl(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Enum, "`enum`")?;
        let name = self.expect_name_identifier("expected enum name after `enum`")?;
        let generics = self.parse_generic_param_list()?;
        let requirements = self.parse_where_clause()?;
        let variants = self.parse_enum_variants()?;
        let end = variants.0.end;

        Ok(Item::Enum(EnumDecl {
            span: self.span(
                target
                    .as_ref()
                    .map_or(start.span.start, |target| target.span.start),
                end,
            ),
            visibility,
            target,
            name: name.value,
            name_span: name.span,
            generics,
            requirements,
            variants: variants.1,
        }))
    }

    pub(super) fn parse_enum_variants(&mut self) -> ParseResult<(ByteSpan, Vec<EnumVariant>)> {
        let start = self.expect_punctuation("{", "`{`")?;
        let mut variants = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_current("expected `}` to close enum declaration");
                return Err(());
            }

            let name = self.expect_name_identifier("expected enum variant name")?;
            let payload = if self.at_punctuation("(") {
                self.parse_parameter_list()?.parameters
            } else {
                Vec::new()
            };
            let end = payload
                .last()
                .map_or(name.span.end, |parameter| parameter.span.end);
            variants.push(EnumVariant {
                span: self.span(name.span.start, end),
                name: name.value,
                name_span: name.span,
                payload,
            });

            self.skip_newlines();
            _ = self.match_punctuation(",");
            self.skip_newlines();
        }

        let end = self.expect_punctuation("}", "`}`")?;
        Ok((self.span(start.span.start, end.span.end), variants))
    }
}
