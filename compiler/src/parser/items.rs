use super::support::ParsedIdentifier;
use super::{ParseResult, Parser};
use crate::ast::{
    AstFile, BorrowType, DropDecl, EnumDecl, EnumVariant, FromImportItem, FunctionDecl,
    FunctionOwner, ImplDecl, ImplMember, ImportAlias, ImportItem, ImportedName, InterfaceDecl,
    Item, MethodDecl, ModulePath, Parameter, ParameterList, PrimitiveDecl, StructDecl, StructField,
    TargetDirective, TypeAliasDecl, TypeExpr, TypeReference, Visibility,
};
use crate::lexer::Keyword;
use crate::literals::decode_string_literal_bytes;
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
        let target = self.parse_optional_target_directive()?;
        if target.is_some() {
            self.skip_newlines();
        }

        if self.at_keyword(Keyword::Use) {
            if target.is_some() {
                self.error_current(
                    "`#target` applies only to function, primitive, or type declarations in v0",
                );
                return Err(());
            }
            return self.parse_use_item(Visibility::Private);
        }

        if self.at_identifier_text("from") || self.at_identifier_text("import") {
            self.error_current("`import` syntax has been removed; use `use` imports");
            return Err(());
        }

        let visibility = self.parse_visibility()?;
        let is_copy = self.match_identifier_text("copy").is_some();

        if self.at_keyword(Keyword::Use) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            if visibility == Visibility::Nocter {
                self.error_current("`pub(nocter) use` is not valid in v0");
                return Err(());
            }
            return self.parse_use_item(visibility);
        }

        if self.at_identifier_text("from") || self.at_identifier_text("import") {
            self.error_current("`import` syntax has been removed; use `use` imports");
            return Err(());
        }

        if self.at_keyword(Keyword::Func) {
            return self.parse_function_decl(visibility, target);
        }

        if self.at_keyword(Keyword::Primitive) {
            return self.parse_primitive_decl(visibility, target);
        }

        if self.at_keyword(Keyword::Type) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            return self.parse_type_alias_decl(visibility, target);
        }

        if self.at_keyword(Keyword::Struct) {
            return self.parse_struct_decl(visibility, target, is_copy);
        }

        if self.at_keyword(Keyword::Enum) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            return self.parse_enum_decl(visibility, target);
        }

        if self.at_keyword(Keyword::Interface) {
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            return self.parse_interface_decl(visibility, target);
        }

        if self.at_identifier_text("trait") {
            if target.is_some() {
                self.error_current(
                    "`#target` applies only to function, primitive, or type declarations in v0",
                );
                return Err(());
            }
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            self.error_current("`trait` has been removed; use `interface` for contracts");
            return Err(());
        }

        if self.at_identifier_text("literal") {
            if target.is_some() {
                self.error_current(
                    "`#target` applies only to function, primitive, or type declarations in v0",
                );
                return Err(());
            }
            if is_copy {
                self.error_current("`copy` applies only to `struct` declarations in v0");
                return Err(());
            }
            self.error_current("literal definitions are not part of v0");
            return Err(());
        }

        if self.at_keyword(Keyword::Impl) {
            if target.is_some() {
                self.error_current(
                    "`#target` applies only to function, primitive, or type declarations in v0",
                );
                return Err(());
            }
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

        if target.is_some() {
            self.error_current(
                "`#target` applies only to function, primitive, or type declarations in v0",
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
        self.expect_punctuation("(", "`(`")?;
        let target = self.expect_string_literal("expected target string literal")?;
        let end = self.expect_punctuation(")", "`)`")?;
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
            span: self.span(start.span.start, end.span.end),
            target_span: target.span,
            target: target_name,
        }))
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

    pub(super) fn parse_use_item(&mut self, visibility: Visibility) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Use, "`use`")?;
        let path = self.parse_module_path()?;

        if path.value == "std/prelude" {
            self.error_at(
                path.span,
                "`std/prelude` is compiler-managed and cannot be imported in source",
            );
            return Err(());
        }

        if self.match_keyword(Keyword::As).is_some() {
            if visibility != Visibility::Private {
                self.error_current("namespace aliases cannot be re-exported in v0");
                return Err(());
            }
            let alias = self.expect_identifier("expected import alias after `as`")?;
            return Ok(Item::Import(ImportItem {
                span: self.span(start.span.start, alias.span.end),
                path,
                alias: ImportAlias {
                    span: alias.span,
                    name: alias.value,
                },
                alias_is_default: false,
            }));
        }

        if self.match_punctuation(".").is_some() {
            let (names, end) = if self.match_punctuation("{").is_some() {
                self.skip_newlines();
                let first = self.parse_imported_name("expected an imported name after `{`")?;
                let mut names = vec![first];

                loop {
                    self.skip_newlines();
                    if self.match_punctuation(",").is_none() {
                        break;
                    }
                    self.skip_newlines();
                    if self.at_punctuation("}") {
                        break;
                    }
                    let name = self.parse_imported_name("expected an imported name after `,`")?;
                    names.push(name);
                }

                self.skip_newlines();
                let close = self.expect_punctuation("}", "`}`")?;
                (names, close.span.end)
            } else {
                let name = self.parse_imported_name("expected an imported name after `.`")?;
                let end = name.span.end;
                (vec![name], end)
            };

            return Ok(Item::FromImport(FromImportItem {
                span: self.span(start.span.start, end),
                visibility,
                path,
                names,
            }));
        }

        if visibility != Visibility::Private {
            self.error_current("`pub use path` is not valid in v0; re-export explicit names");
            return Err(());
        }
        let end = path.span.end;
        let alias = default_namespace_alias(&path);

        Ok(Item::Import(ImportItem {
            span: self.span(start.span.start, end),
            path,
            alias,
            alias_is_default: true,
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

    pub(super) fn parse_function_decl(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
    ) -> ParseResult<Item> {
        self.parse_function_decl_data(visibility, target)
            .map(Item::Function)
    }

    pub(super) fn parse_function_decl_data(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
    ) -> ParseResult<FunctionDecl> {
        let start = self.expect_keyword(Keyword::Func, "`func`")?;
        let first_name = self.expect_identifier("expected function name after `func`")?;
        let (owner, name, name_span, member_name, member_name_span) =
            if self.match_punctuation(".").is_some() {
                let member =
                    self.expect_identifier("expected associated function name after `.`")?;
                (
                    Some(FunctionOwner {
                        name: first_name.value.clone(),
                        name_span: first_name.span,
                    }),
                    format!("{}.{}", first_name.value, member.value),
                    self.span(first_name.span.start, member.span.end),
                    member.value,
                    member.span,
                )
            } else {
                (
                    None,
                    first_name.value.clone(),
                    first_name.span,
                    first_name.value,
                    first_name.span,
                )
            };
        let generics = self.parse_generic_param_list()?;
        let parameters = self.parse_parameter_list()?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(FunctionDecl {
            span: self.span(
                target
                    .as_ref()
                    .map_or(start.span.start, |target| target.span.start),
                end,
            ),
            visibility,
            target,
            owner,
            name,
            name_span,
            member_name,
            member_name_span,
            generics,
            parameters,
            return_type,
            body,
        })
    }

    pub(super) fn parse_primitive_decl(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Primitive, "`primitive`")?;
        let name = self.expect_identifier("expected primitive name after `primitive`")?;
        let generics = self.parse_generic_param_list()?;
        let parameters = self.parse_parameter_list()?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let end = return_type.span().end;

        Ok(Item::Primitive(PrimitiveDecl {
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
            parameters,
            return_type,
        }))
    }

    pub(super) fn parse_type_alias_decl(
        &mut self,
        visibility: Visibility,
        target_directive: Option<TargetDirective>,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Type, "`type`")?;
        let name = self.expect_identifier("expected type alias name after `type`")?;
        let generics = self.parse_generic_param_list()?;
        self.expect_punctuation("=", "`=`")?;
        let target = self.parse_type()?;
        let end = target.span().end;

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
        }))
    }

    pub(super) fn parse_struct_decl(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
        is_copy: bool,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Struct, "`struct`")?;
        let name = self.expect_identifier("expected struct name after `struct`")?;
        let generics = self.parse_generic_param_list()?;
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
                    "embedding declarations are not part of v0",
                );
                return Err(());
            }
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

    pub(super) fn parse_enum_decl(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Enum, "`enum`")?;
        let name = self.expect_identifier("expected enum name after `enum`")?;
        let generics = self.parse_generic_param_list()?;
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
        let name = self.expect_identifier("expected interface name after `interface`")?;
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
        let name = self.expect_identifier("expected method name after `.`")?;
        let parameters = self.parse_parameter_list()?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let body = if require_body {
            Some(self.parse_block()?)
        } else if self.at_punctuation("{") {
            self.error_current("interface method signatures cannot have bodies");
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

    fn parse_self_receiver(&mut self, message: &'static str) -> ParseResult<Parameter> {
        let borrow = self
            .match_punctuation("&+")
            .map(|token| (token, true))
            .or_else(|| self.match_punctuation("&").map(|token| (token, false)));
        let self_span = self.expect_self_identifier(message)?;
        let ty = if let Some((borrow, is_readwrite)) = borrow {
            TypeExpr::Borrow(BorrowType {
                span: self.span(borrow.span.start, self_span.end),
                is_readwrite,
                inner: Box::new(self_type(self_span)),
            })
        } else {
            self_type(self_span)
        };

        Ok(Parameter {
            span: ty.span(),
            name: "self".to_string(),
            name_span: self_span,
            ty,
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

    pub(super) fn parse_parameter_list(&mut self) -> ParseResult<ParameterList> {
        let start = self.expect_punctuation("(", "`(`")?;
        let mut parameters = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation(")") {
            if self.at_eof() {
                self.error_current("expected `)` to close parameter list");
                return Err(());
            }

            if let Some(token) = self.match_keyword(Keyword::Var) {
                self.error_at(token.span, "`var` parameters are not part of v0");
                return Err(());
            }
            if self.at_ellipsis() {
                self.error_at(
                    self.ellipsis_span(),
                    "variadic parameters are not part of v0",
                );
                return Err(());
            }
            let name = self.expect_identifier("expected parameter name")?;
            self.expect_punctuation(":", "`:`")?;
            let ty = self.parse_type()?;
            if self.at_punctuation("=") {
                self.error_current("default parameters are not part of v0");
                return Err(());
            }
            if self.at_ellipsis() {
                self.error_at(
                    self.ellipsis_span(),
                    "variadic parameters are not part of v0",
                );
                return Err(());
            }
            let end = ty.span().end;
            parameters.push(Parameter {
                span: self.span(name.span.start, end),
                name: name.value,
                name_span: name.span,
                ty,
            });

            self.skip_newlines();
            let Some(comma) = self.match_punctuation(",") else {
                break;
            };
            if self.at_punctuation(")") {
                self.error_at(
                    comma.span,
                    "trailing commas in single-line parameter lists are not part of v0",
                );
                return Err(());
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
        let mut segment_spans = Vec::new();
        let mut start = self.current().span.start;

        if let Some(slash) = self.match_punctuation("/") {
            start = slash.span.start;
            value.push('/');
        }

        while self.at_punctuation(".") {
            let dot = self.bump();
            if value.is_empty() {
                start = dot.span.start;
            }

            if self.match_punctuation("/").is_some() {
                value.push_str("./");
                segments.push(".".to_string());
                segment_spans.push(dot.span);
                break;
            }

            if let Some(second_dot) = self.match_punctuation(".") {
                self.expect_punctuation("/", "`/`")?;
                value.push_str("../");
                segments.push("..".to_string());
                segment_spans.push(self.span(dot.span.start, second_dot.span.end));
                continue;
            }

            self.error_current("expected `/` or `.` in relative module path");
            return Err(());
        }

        let first = self.expect_module_path_segment("expected module path segment")?;
        let mut end = first.span.end;
        segments.push(first.value);
        segment_spans.push(first.span);

        while self.match_punctuation("/").is_some() {
            let segment =
                self.expect_module_path_segment("expected module path segment after `/`")?;
            end = segment.span.end;
            segments.push(segment.value);
            segment_spans.push(segment.span);
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
            segment_spans,
        })
    }
}

const MODULE_PATH_SEGMENT_DIAGNOSTIC: &str = "module path segments must be snake_case identifiers";

impl Parser<'_> {
    fn expect_module_path_segment(&mut self, message: &str) -> ParseResult<ParsedIdentifier> {
        let segment = self.expect_identifier(message)?;
        if is_valid_module_path_segment(&segment.value) {
            return Ok(segment);
        }

        self.error_at(segment.span, MODULE_PATH_SEGMENT_DIAGNOSTIC);
        Err(())
    }
}

fn is_valid_module_path_segment(segment: &str) -> bool {
    if segment == "_" {
        return false;
    }

    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !matches!(first, 'a'..='z' | '_') {
        return false;
    }

    chars.all(|char| matches!(char, 'a'..='z' | '0'..='9' | '_'))
}

fn default_namespace_alias(path: &ModulePath) -> ImportAlias {
    for (segment, span) in path.segments.iter().zip(path.segment_spans.iter()).rev() {
        if segment != "." && segment != ".." {
            return ImportAlias {
                span: *span,
                name: segment.clone(),
            };
        }
    }

    ImportAlias {
        span: path.span,
        name: path.value.clone(),
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
