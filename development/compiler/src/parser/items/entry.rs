use super::*;

impl Parser<'_> {
    pub(in crate::parser) fn parse_source_file(&mut self) -> ParseResult<AstFile> {
        self.skip_newlines();
        self.parse_module_body()
    }

    pub(in crate::parser) fn parse_module_body(&mut self) -> ParseResult<AstFile> {
        let mut items = Vec::new();
        let mut allow_use = true;
        while !self.at_eof() {
            if self.at_punctuation("#")
                && let Some(name) = self.identifier_text_at_offset(1)
                && name != "target"
            {
                self.error_current(format!(
                    "package directive `#{name}` is valid only in a package-root `nocter.nct`"
                ));
                return Err(());
            }
            if !allow_use && self.at_top_level_use_start() {
                self.error_current(
                    "top-level `use` declarations must appear before other declarations",
                );
                return Err(());
            }
            let item = self.parse_item()?;
            if !matches!(item, Item::Import(_) | Item::FromImport(_)) {
                allow_use = false;
            }
            items.push(item);
            self.skip_newlines();
        }

        let eof = self.current().clone();
        Ok(AstFile {
            span: self.span(0, eof.span.end),
            items,
        })
    }

    fn at_top_level_use_start(&self) -> bool {
        self.at_keyword(Keyword::Use)
            || (self.at_keyword(Keyword::Pub) && self.next_is_keyword(Keyword::Use))
    }

    pub(super) fn parse_item(&mut self) -> ParseResult<Item> {
        let target = self.parse_optional_target_directive()?;
        if target.is_some() {
            self.skip_newlines();
        }

        if self.at_keyword(Keyword::Use) {
            if target.is_some() {
                self.error_current(
                    "`#target` applies only to function, primitive, or type declarations",
                );
                return Err(());
            }
            return self.parse_use_item(Visibility::Private);
        }

        if self.at_identifier_text("from") || self.at_identifier_text("import") {
            self.error_current("`import` syntax has been removed; write a `use` declaration");
            return Err(());
        }

        if self.at_identifier_text("include") {
            self.error_current("textual include is not supported; write a `use` declaration");
            return Err(());
        }

        let visibility = self.parse_visibility()?;
        let is_copy = self.match_identifier_text("copy").is_some();
        let result_allocation = self.parse_optional_result_allocation_modifier();

        if self.at_keyword(Keyword::Use) {
            if result_allocation.is_some() {
                self.error_current("`alloc` applies only to callable declarations");
                return Err(());
            }
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations");
                return Err(());
            }
            if visibility == Visibility::Nocter {
                self.error_current("`pub(nocter) use` is not valid");
                return Err(());
            }
            return self.parse_use_item(visibility);
        }

        if self.at_identifier_text("from") || self.at_identifier_text("import") {
            self.error_current("`import` syntax has been removed; write a `use` declaration");
            return Err(());
        }

        if self.at_identifier_text("include") {
            self.error_current("textual include is not supported; write a `use` declaration");
            return Err(());
        }

        if self.at_keyword(Keyword::Func) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations");
                return Err(());
            }
            return self.parse_function_decl(visibility, target, result_allocation);
        }

        if self.at_keyword(Keyword::Test) {
            if target.is_some() {
                self.error_current("`#target` does not apply to test declarations");
                return Err(());
            }
            if is_copy || visibility != Visibility::Private {
                self.error_current("test declarations do not use top-level modifiers");
                return Err(());
            }
            return self.parse_test_decl();
        }

        if self.at_keyword(Keyword::Primitive) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations");
                return Err(());
            }
            return self.parse_primitive_decl(visibility, target, result_allocation);
        }

        if result_allocation.is_some() {
            self.error_current("`alloc` applies only to callable declarations");
            return Err(());
        }

        if self.at_keyword(Keyword::Literal) {
            if target.is_some() {
                self.error_current("`#target` does not apply to literal definitions");
                return Err(());
            }
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations");
                return Err(());
            }
            self.error_current(
                "top-level literal definitions have been removed; declare the literal inside `construct Type { ... }`",
            );
            return Err(());
        }

        if self.at_keyword(Keyword::Construct) {
            if target.is_some() {
                self.error_current("`#target` does not apply to construct declarations");
                return Err(());
            }
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations");
                return Err(());
            }
            if visibility != Visibility::Private {
                self.error_current("`construct` declarations do not use visibility modifiers");
                return Err(());
            }
            return self.parse_construct_decl();
        }

        if self.at_keyword(Keyword::Type) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations");
                return Err(());
            }
            return self.parse_type_alias_decl(visibility, target);
        }

        if self.at_keyword(Keyword::Struct) {
            return self.parse_struct_decl(visibility, target, is_copy);
        }

        if self.at_keyword(Keyword::Enum) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations");
                return Err(());
            }
            return self.parse_enum_decl(visibility, target);
        }

        if self.at_keyword(Keyword::Interface) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations");
                return Err(());
            }
            return self.parse_interface_decl(visibility, target);
        }

        if self.at_identifier_text("trait") {
            if target.is_some() {
                self.error_current(
                    "`#target` applies only to function, primitive, or type declarations",
                );
                return Err(());
            }
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations");
                return Err(());
            }
            self.error_current("`trait` has been removed; use `interface` for contracts");
            return Err(());
        }

        if self.at_identifier_text("literal") {
            if target.is_some() {
                self.error_current(
                    "`#target` applies only to function, primitive, or type declarations",
                );
                return Err(());
            }
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations");
                return Err(());
            }
            self.error_current("literal definitions require the reserved `literal` keyword");
            return Err(());
        }

        if self.at_keyword(Keyword::Impl) {
            if target.is_some() {
                self.error_current(
                    "`#target` applies only to function, primitive, or type declarations",
                );
                return Err(());
            }
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations");
                return Err(());
            }
            if visibility != Visibility::Private {
                self.error_current("`impl` blocks do not use visibility modifiers");
                return Err(());
            }
            return self.parse_impl_decl();
        }

        if visibility != Visibility::Private || is_copy {
            self.error_current("expected declaration after top-level modifier");
            return Err(());
        }

        if target.is_some() {
            self.error_current(
                "`#target` applies only to function, primitive, or type declarations",
            );
            return Err(());
        }

        self.error_current("expected a top-level item");
        Err(())
    }

    fn parse_optional_target_directive(&mut self) -> ParseResult<Option<TargetDirective>> {
        let Some(start) = self.match_punctuation("#") else {
            return Ok(None);
        };

        let name = self.expect_identifier("expected directive name after `#`")?;
        if name.value != "target" {
            self.error_at(name.span, "expected `target` directive");
            return Err(());
        }
        self.expect_punctuation(":", "`:` after `#target`")?;
        let target = self.expect_string_literal("expected target string literal")?;
        let target_text = self.lexeme(&target);
        let target_bytes = decode_string_literal_bytes(&target_text).map_err(|message| {
            self.error_at(
                target.span,
                format!("invalid target string literal: {message}"),
            );
        })?;
        let Ok(target_name) = String::from_utf8(target_bytes) else {
            self.error_at(target.span, "target string literal must be UTF-8");
            return Err(());
        };

        Ok(Some(TargetDirective {
            span: self.span(start.span.start, target.span.end),
            target_span: target.span,
            target: target_name,
        }))
    }

    pub(in crate::parser) fn parse_visibility(&mut self) -> ParseResult<Visibility> {
        if self.match_keyword(Keyword::Pub).is_none() {
            return Ok(Visibility::Private);
        }

        if self.match_punctuation("(").is_none() {
            return Ok(Visibility::Public);
        }

        let scope = self.expect_identifier("expected visibility scope after `pub(`")?;
        self.expect_punctuation(")", "`)`")?;
        if scope.value != "nocter" {
            self.error_at(scope.span, "expected `nocter` visibility scope");
            return Err(());
        }

        Ok(Visibility::Nocter)
    }
}
