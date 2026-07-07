use super::{ParseResult, Parser};
use crate::ast::{
    AstFile, EnumDecl, EnumVariant, FromImportItem, FunctionDecl, ImplDecl, ImplMember,
    ImportAlias, ImportItem, ImportedName, Item, MethodDecl, ModulePath, Parameter, ParameterList,
    PrimitiveDecl, StructDecl, StructField, TraitDecl, TypeAliasDecl, UseItem, Visibility,
};
use crate::lexer::Keyword;
use crate::source::ByteSpan;

impl Parser<'_> {
    pub(super) fn parse_source_file(&mut self) -> ParseResult<AstFile> {
        let mut items = Vec::new();

        self.skip_newlines();
        while !self.at_eof() {
            items.push(self.parse_item()?);
            self.skip_newlines();
        }

        let eof = self.current().clone();
        Ok(AstFile {
            span: self.span(0, eof.span.end),
            items,
        })
    }

    pub(super) fn parse_item(&mut self) -> ParseResult<Item> {
        if self.at_keyword(Keyword::Use) {
            return self.parse_use_item();
        }

        if self.at_keyword(Keyword::From) {
            return self.parse_from_import_item(Visibility::Private);
        }

        if self.at_keyword(Keyword::Import) {
            return self.parse_import_item();
        }

        let visibility = self.parse_visibility()?;
        let is_copy = self.match_keyword(Keyword::Copy).is_some();

        if self.at_keyword(Keyword::From) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            if visibility != Visibility::Public {
                self.error_current("`pub(nocter) from` is not valid in v0");
                return Err(());
            }
            return self.parse_from_import_item(visibility);
        }

        if self.at_keyword(Keyword::Import) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            self.error_current("`pub import` is not valid in v0");
            return Err(());
        }

        if self.at_keyword(Keyword::Func) {
            return self.parse_function_decl(visibility);
        }

        if self.at_keyword(Keyword::Primitive) {
            return self.parse_primitive_decl(visibility);
        }

        if self.at_keyword(Keyword::Type) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            return self.parse_type_alias_decl(visibility);
        }

        if self.at_keyword(Keyword::Struct) {
            return self.parse_struct_decl(visibility, is_copy);
        }

        if self.at_keyword(Keyword::Enum) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            return self.parse_enum_decl(visibility);
        }

        if self.at_keyword(Keyword::Trait) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            return self.parse_trait_decl(visibility);
        }

        if self.at_keyword(Keyword::Impl) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            if visibility != Visibility::Private {
                self.error_current("`impl` blocks do not use visibility modifiers in v0");
                return Err(());
            }
            return self.parse_impl_decl();
        }

        if visibility != Visibility::Private || is_copy {
            self.error_current("expected declaration after top-level modifier");
            return Err(());
        }

        self.error_current("expected a top-level item");
        Err(())
    }

    pub(super) fn parse_visibility(&mut self) -> ParseResult<Visibility> {
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

    pub(super) fn parse_use_item(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Use, "`use`")?;
        let path = self.parse_module_path()?;
        let end = path.span.end;

        Ok(Item::Use(UseItem {
            span: self.span(start.span.start, end),
            path,
        }))
    }

    pub(super) fn parse_import_item(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Import, "`import`")?;
        let path = self.parse_module_path()?;
        self.expect_keyword(Keyword::As, "`as`")?;
        let alias = self.expect_identifier("expected import alias after `as`")?;
        let end = alias.span.end;

        Ok(Item::Import(ImportItem {
            span: self.span(start.span.start, end),
            path,
            alias: ImportAlias {
                span: alias.span,
                name: alias.value,
            },
        }))
    }

    pub(super) fn parse_from_import_item(&mut self, visibility: Visibility) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::From, "`from`")?;
        let path = self.parse_module_path()?;
        self.expect_keyword(Keyword::Import, "`import`")?;

        let first = self.parse_imported_name("expected an imported name")?;
        let mut end = first.span.end;
        let mut names = vec![first];

        while self.match_punctuation(",").is_some() {
            self.skip_newlines();
            let name = self.parse_imported_name("expected an imported name after `,`")?;
            end = name.span.end;
            names.push(name);
        }

        Ok(Item::FromImport(FromImportItem {
            span: self.span(start.span.start, end),
            visibility,
            path,
            names,
        }))
    }

    pub(super) fn parse_imported_name(&mut self, message: &str) -> ParseResult<ImportedName> {
        let name = self.expect_identifier(message)?;
        let mut end = name.span.end;
        let alias = if self.match_keyword(Keyword::As).is_some() {
            let alias = self.expect_identifier("expected import alias after `as`")?;
            end = alias.span.end;
            Some(ImportAlias {
                span: alias.span,
                name: alias.value,
            })
        } else {
            None
        };

        Ok(ImportedName {
            span: self.span(name.span.start, end),
            name_span: name.span,
            name: name.value,
            alias,
        })
    }

    pub(super) fn parse_function_decl(&mut self, visibility: Visibility) -> ParseResult<Item> {
        self.parse_function_decl_data(visibility)
            .map(Item::Function)
    }

    pub(super) fn parse_function_decl_data(
        &mut self,
        visibility: Visibility,
    ) -> ParseResult<FunctionDecl> {
        let start = self.expect_keyword(Keyword::Func, "`func`")?;
        let name = self.expect_identifier("expected function name after `func`")?;
        let generics = self.parse_generic_param_list()?;
        let parameters = self.parse_parameter_list()?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(FunctionDecl {
            span: self.span(start.span.start, end),
            visibility,
            name: name.value,
            name_span: name.span,
            generics,
            parameters,
            return_type,
            body,
        })
    }

    pub(super) fn parse_primitive_decl(&mut self, visibility: Visibility) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Primitive, "`primitive`")?;
        let name = self.expect_identifier("expected primitive name after `primitive`")?;
        let generics = self.parse_generic_param_list()?;
        let parameters = self.parse_parameter_list()?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let end = return_type.span().end;

        Ok(Item::Primitive(PrimitiveDecl {
            span: self.span(start.span.start, end),
            visibility,
            name: name.value,
            name_span: name.span,
            generics,
            parameters,
            return_type,
        }))
    }

    pub(super) fn parse_type_alias_decl(&mut self, visibility: Visibility) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Type, "`type`")?;
        let name = self.expect_identifier("expected type alias name after `type`")?;
        let generics = self.parse_generic_param_list()?;
        self.expect_punctuation("=", "`=`")?;
        let target = self.parse_type()?;
        let end = target.span().end;

        Ok(Item::TypeAlias(TypeAliasDecl {
            span: self.span(start.span.start, end),
            visibility,
            name: name.value,
            name_span: name.span,
            generics,
            target,
        }))
    }

    pub(super) fn parse_struct_decl(
        &mut self,
        visibility: Visibility,
        is_copy: bool,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Struct, "`struct`")?;
        let name = self.expect_identifier("expected struct name after `struct`")?;
        let generics = self.parse_generic_param_list()?;
        let fields = self.parse_struct_fields()?;
        let end = fields.0.end;

        Ok(Item::Struct(StructDecl {
            span: self.span(start.span.start, end),
            visibility,
            is_copy,
            name: name.value,
            name_span: name.span,
            generics,
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
            let name = self.expect_identifier("expected struct field name")?;
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

    pub(super) fn parse_enum_decl(&mut self, visibility: Visibility) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Enum, "`enum`")?;
        let name = self.expect_identifier("expected enum name after `enum`")?;
        let generics = self.parse_generic_param_list()?;
        let variants = self.parse_enum_variants()?;
        let end = variants.0.end;

        Ok(Item::Enum(EnumDecl {
            span: self.span(start.span.start, end),
            visibility,
            name: name.value,
            name_span: name.span,
            generics,
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

            let name = self.expect_identifier("expected enum variant name")?;
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

    pub(super) fn parse_trait_decl(&mut self, visibility: Visibility) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Trait, "`trait`")?;
        let name = self.expect_identifier("expected trait name after `trait`")?;
        let generics = self.parse_generic_param_list()?;
        let open = self.expect_punctuation("{", "`{`")?;
        let mut methods = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close trait declaration");
                return Err(());
            }

            let method_visibility = self.parse_visibility()?;
            if !self.at_keyword(Keyword::Method) {
                self.error_current("expected `method` in trait declaration");
                return Err(());
            }
            methods.push(self.parse_method_decl(method_visibility, false)?);
            self.skip_newlines();
            _ = self.match_punctuation(",");
            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Item::Trait(TraitDecl {
            span: self.span(start.span.start, close.span.end),
            visibility,
            name: name.value,
            name_span: name.span,
            generics,
            methods,
        }))
    }

    pub(super) fn parse_impl_decl(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Impl, "`impl`")?;
        let first_ty = self.parse_type()?;
        let (trait_ty, target_ty) = if self.match_keyword(Keyword::For).is_some() {
            let target_ty = self.parse_type()?;
            (Some(first_ty), target_ty)
        } else {
            (None, first_ty)
        };
        let open = self.expect_punctuation("{", "`{`")?;
        let mut members = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close impl block");
                return Err(());
            }

            let visibility = self.parse_visibility()?;
            if self.at_keyword(Keyword::Func) {
                members.push(ImplMember::Function(
                    self.parse_function_decl_data(visibility)?,
                ));
            } else if self.at_keyword(Keyword::Method) {
                members.push(ImplMember::Method(
                    self.parse_method_decl(visibility, true)?,
                ));
            } else {
                self.error_current("expected `func` or `method` in impl block");
                return Err(());
            }

            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Item::Impl(ImplDecl {
            span: self.span(start.span.start, close.span.end),
            trait_ty,
            target_ty,
            members,
        }))
    }

    pub(super) fn parse_method_decl(
        &mut self,
        visibility: Visibility,
        require_body: bool,
    ) -> ParseResult<MethodDecl> {
        let start = self.expect_keyword(Keyword::Method, "`method`")?;
        let receiver = self.parse_method_receiver()?;
        self.expect_punctuation(".", "`.`")?;
        let name = self.expect_identifier("expected method name after `.`")?;
        let parameters = self.parse_parameter_list()?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let body = if require_body {
            Some(self.parse_block()?)
        } else if self.at_punctuation("{") {
            self.error_current("trait method signatures cannot have bodies in v0");
            return Err(());
        } else {
            None
        };
        let end = body
            .as_ref()
            .map_or(return_type.span().end, |body| body.span.end);

        Ok(MethodDecl {
            span: self.span(start.span.start, end),
            visibility,
            receiver,
            name: name.value,
            name_span: name.span,
            parameters,
            return_type,
            body,
        })
    }

    pub(super) fn parse_method_receiver(&mut self) -> ParseResult<Parameter> {
        let open = self.expect_punctuation("(", "`(`")?;
        let name = self.expect_identifier("expected receiver name")?;
        self.expect_punctuation(":", "`:`")?;
        let ty = self.parse_type()?;
        let close = self.expect_punctuation(")", "`)`")?;

        Ok(Parameter {
            span: self.span(open.span.start, close.span.end),
            name: name.value,
            name_span: name.span,
            ty,
        })
    }

    pub(super) fn parse_parameter_list(&mut self) -> ParseResult<ParameterList> {
        let start = self.expect_punctuation("(", "`(`")?;
        let mut parameters = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation(")") {
            if self.at_eof() {
                self.error_current("expected `)` to close parameter list");
                return Err(());
            }

            let name = self.expect_identifier("expected parameter name")?;
            self.expect_punctuation(":", "`:`")?;
            let ty = self.parse_type()?;
            let end = ty.span().end;
            parameters.push(Parameter {
                span: self.span(name.span.start, end),
                name: name.value,
                name_span: name.span,
                ty,
            });

            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }

        let end = self.expect_punctuation(")", "`)`")?;
        Ok(ParameterList {
            span: self.span(start.span.start, end.span.end),
            parameters,
        })
    }
    pub(super) fn parse_module_path(&mut self) -> ParseResult<ModulePath> {
        let mut value = String::new();
        let mut segments = Vec::new();
        let mut start = self.current().span.start;

        while self.at_punctuation(".") {
            let dot = self.bump();
            start = dot.span.start;

            if self.match_punctuation("/").is_some() {
                value.push_str("./");
                segments.push(".".to_string());
                break;
            }

            if self.match_punctuation(".").is_some() {
                self.expect_punctuation("/", "`/`")?;
                value.push_str("../");
                segments.push("..".to_string());
                continue;
            }

            self.error_current("expected `/` or `.` in relative module path");
            return Err(());
        }

        let first = self.expect_identifier("expected module path segment")?;
        let mut end = first.span.end;
        segments.push(first.value);

        while self.match_punctuation("/").is_some() {
            let segment = self.expect_identifier("expected module path segment after `/`")?;
            end = segment.span.end;
            segments.push(segment.value);
        }

        let path_segments = segments
            .iter()
            .filter(|segment| segment.as_str() != "." && segment.as_str() != "..")
            .cloned()
            .collect::<Vec<_>>();
        value.push_str(&path_segments.join("/"));

        Ok(ModulePath {
            span: self.span(start, end),
            value,
            segments,
        })
    }
}
