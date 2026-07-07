//! Parser for Nocter source syntax.

use crate::ast::{
    ArrayLength, ArrayLiteralExpr, ArrayType, AstFile, BinaryExpr, BinaryOperator, BindingKind,
    BindingStmt, Block, BorrowType, BreakStmt, CallExpr, CatchExpr, ContinueStmt, EnumDecl,
    EnumVariant, Expr, ExpressionStmt, FailStmt, FallibleType, ForRangeStmt, ForceExpr,
    FromImportItem, FunctionDecl, GenericParam, GenericParamList, GenericType, GroupExpr,
    IdentifierExpr, IfIsStmt, IfLetStmt, IfStmt, ImplDecl, ImplMember, ImportAlias, ImportItem,
    ImportedName, IndexExpr, Item, LiteralExpr, LoopStmt, MemberExpr, MethodDecl, ModulePath,
    OptionalDefaultExpr, OptionalType, Parameter, ParameterList, PointerType, PrimitiveDecl,
    ProgramDecl, PropagationExpr, ReturnStmt, Stmt, StructDecl, StructField, StructLiteralExpr,
    StructLiteralField, SwitchArm, SwitchElseArm, SwitchPayloadBinding, SwitchStmt, TraitDecl,
    TypeAliasDecl, TypeConversionExpr, TypeExpr, TypeReference, UnaryExpr, UnaryOperator, UseItem,
    ViewType, Visibility, WhileLetStmt, WhileStmt,
};
use crate::diagnostics::Diagnostic;
use crate::lexer::{Keyword, Token, TokenKind};
use crate::source::{ByteSpan, SourceId, SourceMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOutput {
    pub ast: Option<AstFile>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(sources: &SourceMap, source: SourceId, tokens: &[Token]) -> ParseOutput {
    if tokens.is_empty() {
        return ParseOutput {
            ast: None,
            diagnostics: vec![Diagnostic::error(
                "E0200",
                "parser requires a token stream ending in EOF",
            )],
        };
    }

    let mut parser = Parser {
        sources,
        source,
        tokens,
        index: 0,
        diagnostics: Vec::new(),
    };

    match parser.parse_source_file() {
        Ok(ast) if parser.diagnostics.is_empty() => ParseOutput {
            ast: Some(ast),
            diagnostics: parser.diagnostics,
        },
        Ok(_) | Err(()) => ParseOutput {
            ast: None,
            diagnostics: parser.diagnostics,
        },
    }
}

type ParseResult<T> = Result<T, ()>;

struct Parser<'a> {
    sources: &'a SourceMap,
    source: SourceId,
    tokens: &'a [Token],
    index: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn parse_source_file(&mut self) -> ParseResult<AstFile> {
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

    fn parse_item(&mut self) -> ParseResult<Item> {
        if self.at_keyword(Keyword::Use) {
            return self.parse_use_item();
        }

        if self.at_keyword(Keyword::From) {
            return self.parse_from_import_item(Visibility::Private);
        }

        if self.at_keyword(Keyword::Import) {
            return self.parse_import_item();
        }

        if self.at_keyword(Keyword::Program) {
            return self.parse_program_decl();
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

    fn parse_visibility(&mut self) -> ParseResult<Visibility> {
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

    fn parse_use_item(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Use, "`use`")?;
        let path = self.parse_module_path()?;
        let end = path.span.end;

        Ok(Item::Use(UseItem {
            span: self.span(start.span.start, end),
            path,
        }))
    }

    fn parse_import_item(&mut self) -> ParseResult<Item> {
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

    fn parse_from_import_item(&mut self, visibility: Visibility) -> ParseResult<Item> {
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

    fn parse_imported_name(&mut self, message: &str) -> ParseResult<ImportedName> {
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

    fn parse_program_decl(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Program, "`program`")?;
        self.expect_punctuation("(", "`(`")?;
        self.expect_punctuation(")", "`)`")?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Item::Program(ProgramDecl {
            span: self.span(start.span.start, end),
            return_type,
            body,
        }))
    }

    fn parse_function_decl(&mut self, visibility: Visibility) -> ParseResult<Item> {
        self.parse_function_decl_data(visibility)
            .map(Item::Function)
    }

    fn parse_function_decl_data(&mut self, visibility: Visibility) -> ParseResult<FunctionDecl> {
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

    fn parse_primitive_decl(&mut self, visibility: Visibility) -> ParseResult<Item> {
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

    fn parse_type_alias_decl(&mut self, visibility: Visibility) -> ParseResult<Item> {
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

    fn parse_struct_decl(&mut self, visibility: Visibility, is_copy: bool) -> ParseResult<Item> {
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

    fn parse_struct_fields(&mut self) -> ParseResult<(ByteSpan, Vec<StructField>)> {
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

    fn parse_enum_decl(&mut self, visibility: Visibility) -> ParseResult<Item> {
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

    fn parse_enum_variants(&mut self) -> ParseResult<(ByteSpan, Vec<EnumVariant>)> {
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

    fn parse_trait_decl(&mut self, visibility: Visibility) -> ParseResult<Item> {
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

    fn parse_impl_decl(&mut self) -> ParseResult<Item> {
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

    fn parse_method_decl(
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

    fn parse_method_receiver(&mut self) -> ParseResult<Parameter> {
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

    fn parse_parameter_list(&mut self) -> ParseResult<ParameterList> {
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

    fn parse_type(&mut self) -> ParseResult<TypeExpr> {
        let mut ty = self.parse_type_atom()?;

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

    fn parse_type_atom(&mut self) -> ParseResult<TypeExpr> {
        if let Some(star) = self.match_punctuation("*") {
            let inner = self.parse_type_atom()?;
            return Ok(TypeExpr::Pointer(PointerType {
                span: self.span(star.span.start, inner.span().end),
                inner: Box::new(inner),
            }));
        }

        if let Some(borrow) = self.match_punctuation("&+") {
            let inner = self.parse_type_atom()?;
            return Ok(TypeExpr::Borrow(BorrowType {
                span: self.span(borrow.span.start, inner.span().end),
                is_readwrite: true,
                inner: Box::new(inner),
            }));
        }

        if let Some(borrow) = self.match_punctuation("&") {
            let inner = self.parse_type_atom()?;
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

            let close = self.expect_punctuation("]", "`]`")?;
            return Ok(TypeExpr::View(ViewType {
                span: self.span(open.span.start, close.span.end),
                is_readwrite: plus.is_some(),
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

    fn parse_type_argument_list(&mut self) -> ParseResult<(Vec<TypeExpr>, ByteSpan)> {
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

    fn parse_generic_param_list(&mut self) -> ParseResult<GenericParamList> {
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

            let parameter = self.expect_identifier("expected generic parameter name")?;
            let bound = if self.match_punctuation(":").is_some() {
                Some(self.parse_type()?)
            } else {
                None
            };
            let end = bound
                .as_ref()
                .map_or(parameter.span.end, |bound| bound.span().end);
            parameters.push(GenericParam {
                span: self.span(parameter.span.start, end),
                name: parameter.value,
                bound,
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

    fn parse_block(&mut self) -> ParseResult<Block> {
        let start = self.expect_punctuation("{", "`{`")?;
        let mut statements = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_current("expected `}` to close block");
                return Err(());
            }

            statements.push(self.parse_statement()?);
            self.skip_newlines();
        }

        let end = self.expect_punctuation("}", "`}`")?;
        Ok(Block {
            span: self.span(start.span.start, end.span.end),
            statements,
        })
    }

    fn parse_statement(&mut self) -> ParseResult<Stmt> {
        self.skip_newlines();

        if self.at_keyword(Keyword::Return) {
            return self.parse_return_statement();
        }

        if self.at_keyword(Keyword::Fail) {
            return self.parse_fail_statement();
        }

        if self.at_keyword(Keyword::If) {
            return self.parse_if_statement();
        }

        if self.at_keyword(Keyword::Switch) {
            return self.parse_switch_statement();
        }

        if self.at_keyword(Keyword::For) {
            return self.parse_for_statement();
        }

        if self.at_keyword(Keyword::While) {
            return self.parse_while_statement();
        }

        if self.at_keyword(Keyword::Loop) {
            return self.parse_loop_statement();
        }

        if self.at_keyword(Keyword::Break) {
            return self.parse_break_statement();
        }

        if self.at_keyword(Keyword::Continue) {
            return self.parse_continue_statement();
        }

        if self.at_keyword(Keyword::Let) || self.at_keyword(Keyword::Var) {
            return self.parse_binding_statement();
        }

        self.parse_expression_statement()
    }

    fn parse_return_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::Return, "`return`")?;
        if self.at_statement_end() {
            return Ok(Stmt::Return(ReturnStmt {
                span: self.span(start.span.start, start.span.end),
                expression: None,
            }));
        }

        let expression = self.parse_expression()?;
        let end = expression.span().end;
        Ok(Stmt::Return(ReturnStmt {
            span: self.span(start.span.start, end),
            expression: Some(expression),
        }))
    }

    fn parse_fail_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::Fail, "`fail`")?;
        let expression = self.parse_expression()?;
        let end = expression.span().end;
        Ok(Stmt::Fail(FailStmt {
            span: self.span(start.span.start, end),
            expression,
        }))
    }

    fn parse_binding_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.bump();
        let kind = match start.kind {
            TokenKind::Keyword(Keyword::Let) => BindingKind::Let,
            TokenKind::Keyword(Keyword::Var) => BindingKind::Var,
            _ => unreachable!("parse_binding_statement starts at let or var"),
        };
        let name = self.expect_identifier("expected binding name")?;
        let ty = if self.match_punctuation(":").is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect_punctuation("=", "`=`")?;
        let initializer = self.parse_expression()?;
        let else_block = if self.match_keyword(Keyword::Else).is_some() {
            Some(self.parse_block()?)
        } else {
            None
        };
        let end = else_block
            .as_ref()
            .map_or(initializer.span().end, |block| block.span.end);

        Ok(Stmt::Binding(BindingStmt {
            span: self.span(start.span.start, end),
            kind,
            name: name.value,
            name_span: name.span,
            ty,
            initializer,
            else_block,
        }))
    }

    fn parse_if_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::If, "`if`")?;
        if self.at_keyword(Keyword::Let) || self.at_keyword(Keyword::Var) {
            return self.parse_if_let_statement(start.span.start);
        }

        let condition = self.parse_expression()?;
        if let Some(is_token) = self.match_keyword(Keyword::Is) {
            return self.parse_if_is_statement(start.span.start, condition, is_token.span.start);
        }

        let then_block = self.parse_block()?;
        let else_block = self.parse_else_block()?;
        let end = else_block
            .as_ref()
            .map_or(then_block.span.end, |block| block.span.end);

        Ok(Stmt::If(IfStmt {
            span: self.span(start.span.start, end),
            condition,
            then_block,
            else_block,
        }))
    }

    fn parse_if_is_statement(
        &mut self,
        start: usize,
        expression: Expr,
        pattern_start: usize,
    ) -> ParseResult<Stmt> {
        let pattern = self.parse_enum_pattern_after_is(pattern_start)?;
        let then_block = self.parse_block()?;
        let else_block = self.parse_else_block()?;
        let end = else_block
            .as_ref()
            .map_or(then_block.span.end, |block| block.span.end);

        Ok(Stmt::IfIs(IfIsStmt {
            span: self.span(start, end),
            expression,
            pattern_span: pattern.span,
            enum_name: pattern.enum_name,
            enum_name_span: pattern.enum_name_span,
            variant_name: pattern.variant_name,
            variant_name_span: pattern.variant_name_span,
            payload: pattern.payload,
            then_block,
            else_block,
        }))
    }

    fn parse_if_let_statement(&mut self, start: usize) -> ParseResult<Stmt> {
        let binding = self.bump();
        let kind = match binding.kind {
            TokenKind::Keyword(Keyword::Let) => BindingKind::Let,
            TokenKind::Keyword(Keyword::Var) => BindingKind::Var,
            _ => unreachable!("parse_if_let_statement starts at let or var"),
        };
        let name = self.expect_identifier("expected optional binding name")?;
        self.expect_punctuation("=", "`=`")?;
        let initializer = self.parse_expression()?;
        let then_block = self.parse_block()?;
        let else_block = self.parse_else_block()?;
        let end = else_block
            .as_ref()
            .map_or(then_block.span.end, |block| block.span.end);

        Ok(Stmt::IfLet(IfLetStmt {
            span: self.span(start, end),
            kind,
            name: name.value,
            name_span: name.span,
            initializer,
            then_block,
            else_block,
        }))
    }

    fn parse_else_block(&mut self) -> ParseResult<Option<Block>> {
        let Some(else_token) = self.match_keyword(Keyword::Else) else {
            return Ok(None);
        };

        if self.at_keyword(Keyword::If) {
            let statement = self.parse_if_statement()?;
            return Ok(Some(Block {
                span: self.span(else_token.span.start, statement.span().end),
                statements: vec![statement],
            }));
        }

        Ok(Some(self.parse_block()?))
    }

    fn parse_switch_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::Switch, "`switch`")?;
        let expression = self.parse_expression()?;
        let open = self.expect_punctuation("{", "`{`")?;
        let mut arms = Vec::new();
        let mut else_arm = None;
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close switch statement");
                return Err(());
            }

            if self.at_keyword(Keyword::Else) {
                if else_arm.is_some() {
                    self.error_current("`switch` can have only one `else` arm");
                    return Err(());
                }

                else_arm = Some(self.parse_switch_else_arm()?);
                self.skip_newlines();
                continue;
            }

            if else_arm.is_some() {
                self.error_current("`else` arm must be the last switch arm");
                return Err(());
            }

            arms.push(self.parse_switch_arm()?);
            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Stmt::Switch(SwitchStmt {
            span: self.span(start.span.start, close.span.end),
            expression,
            arms,
            else_arm,
        }))
    }

    fn parse_switch_arm(&mut self) -> ParseResult<SwitchArm> {
        let start = self.expect_keyword(Keyword::Is, "`is`")?;
        let pattern = self.parse_enum_pattern_after_is(start.span.start)?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(SwitchArm {
            span: self.span(start.span.start, end),
            enum_name: pattern.enum_name,
            enum_name_span: pattern.enum_name_span,
            variant_name: pattern.variant_name,
            variant_name_span: pattern.variant_name_span,
            payload: pattern.payload,
            body,
        })
    }

    fn parse_enum_pattern_after_is(&mut self, start: usize) -> ParseResult<ParsedEnumPattern> {
        let enum_name = self.expect_identifier("expected enum name after `is`")?;
        self.expect_punctuation(".", "`.`")?;
        let variant_name = self.expect_identifier("expected enum variant name after `.`")?;
        let mut end = variant_name.span.end;
        let payload = if self.match_punctuation("(").is_some() {
            let payload = self.expect_identifier("expected payload binding name")?;
            let close = self.expect_punctuation(")", "`)`")?;
            end = close.span.end;
            Some(SwitchPayloadBinding {
                span: payload.span,
                name: payload.value,
            })
        } else {
            None
        };

        Ok(ParsedEnumPattern {
            span: self.span(start, end),
            enum_name: enum_name.value,
            enum_name_span: enum_name.span,
            variant_name: variant_name.value,
            variant_name_span: variant_name.span,
            payload,
        })
    }

    fn parse_switch_else_arm(&mut self) -> ParseResult<SwitchElseArm> {
        let start = self.expect_keyword(Keyword::Else, "`else`")?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(SwitchElseArm {
            span: self.span(start.span.start, end),
            body,
        })
    }

    fn parse_for_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::For, "`for`")?;
        let name = self.expect_identifier("expected loop variable name after `for`")?;
        self.expect_keyword(Keyword::In, "`in`")?;
        let range_start = self.parse_expression()?;
        let range = self.expect_punctuation("..<", "`..<`")?;
        let range_end = self.parse_expression()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Stmt::ForRange(ForRangeStmt {
            span: self.span(start.span.start, end),
            name: name.value,
            name_span: name.span,
            start: range_start,
            range_span: range.span,
            end: range_end,
            body,
        }))
    }

    fn parse_while_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::While, "`while`")?;
        if self.at_keyword(Keyword::Let) || self.at_keyword(Keyword::Var) {
            return self.parse_while_let_statement(start.span.start);
        }

        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Stmt::While(WhileStmt {
            span: self.span(start.span.start, end),
            condition,
            body,
        }))
    }

    fn parse_while_let_statement(&mut self, start: usize) -> ParseResult<Stmt> {
        let binding = self.bump();
        let kind = match binding.kind {
            TokenKind::Keyword(Keyword::Let) => BindingKind::Let,
            TokenKind::Keyword(Keyword::Var) => BindingKind::Var,
            _ => unreachable!("parse_while_let_statement starts at let or var"),
        };
        let name = self.expect_identifier("expected optional loop binding name")?;
        self.expect_punctuation("=", "`=`")?;
        let initializer = self.parse_expression()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Stmt::WhileLet(WhileLetStmt {
            span: self.span(start, end),
            kind,
            name: name.value,
            name_span: name.span,
            initializer,
            body,
        }))
    }

    fn parse_loop_statement(&mut self) -> ParseResult<Stmt> {
        let start = self.expect_keyword(Keyword::Loop, "`loop`")?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(Stmt::Loop(LoopStmt {
            span: self.span(start.span.start, end),
            body,
        }))
    }

    fn parse_break_statement(&mut self) -> ParseResult<Stmt> {
        let token = self.expect_keyword(Keyword::Break, "`break`")?;
        self.expect_statement_end("`break` does not take a value")?;
        Ok(Stmt::Break(BreakStmt {
            span: self.span(token.span.start, token.span.end),
        }))
    }

    fn parse_continue_statement(&mut self) -> ParseResult<Stmt> {
        let token = self.expect_keyword(Keyword::Continue, "`continue`")?;
        self.expect_statement_end("`continue` does not take a value")?;
        Ok(Stmt::Continue(ContinueStmt {
            span: self.span(token.span.start, token.span.end),
        }))
    }

    fn parse_expression_statement(&mut self) -> ParseResult<Stmt> {
        let expression = self.parse_expression()?;
        let span = expression.span();
        Ok(Stmt::Expression(ExpressionStmt { span, expression }))
    }

    fn parse_expression(&mut self) -> ParseResult<Expr> {
        self.parse_optional_default_expression()
    }

    fn parse_optional_default_expression(&mut self) -> ParseResult<Expr> {
        let left = self.parse_logical_or_expression()?;

        if let Some(operator) = self.match_punctuation("??") {
            let right = self.parse_optional_default_expression()?;
            return Ok(Expr::OptionalDefault(OptionalDefaultExpr {
                span: self.span(left.span().start, right.span().end),
                operator_span: operator.span,
                value: Box::new(left),
                default: Box::new(right),
            }));
        }

        Ok(left)
    }

    fn parse_logical_or_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_logical_and_expression()?;

        while let Some(operator) = self.match_logical_or_operator() {
            let right = self.parse_logical_and_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_logical_and_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_equality_expression()?;

        while let Some(operator) = self.match_logical_and_operator() {
            let right = self.parse_equality_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_equality_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_ordering_expression()?;

        while let Some(operator) = self.match_equality_operator() {
            let right = self.parse_ordering_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_ordering_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_shift_expression()?;

        while let Some(operator) = self.match_ordering_operator() {
            let right = self.parse_shift_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_shift_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_additive_expression()?;

        while let Some(operator) = self.match_shift_operator() {
            let right = self.parse_additive_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_additive_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_multiplicative_expression()?;

        while let Some(operator) = self.match_additive_operator() {
            let right = self.parse_multiplicative_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_multiplicative_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_prefix_expression()?;

        while let Some(operator) = self.match_multiplicative_operator() {
            let right = self.parse_prefix_expression()?;
            expression = Expr::Binary(BinaryExpr {
                span: self.span(expression.span().start, right.span().end),
                left: Box::new(expression),
                operator: operator.value,
                operator_span: operator.span,
                right: Box::new(right),
            });
        }

        Ok(expression)
    }

    fn parse_prefix_expression(&mut self) -> ParseResult<Expr> {
        if let Some(operator) = self.match_unary_operator() {
            let operand = self.parse_prefix_expression()?;
            return Ok(Expr::Unary(UnaryExpr {
                span: self.span(operator.span.start, operand.span().end),
                operator: operator.value,
                operator_span: operator.span,
                operand: Box::new(operand),
            }));
        }

        self.parse_postfix_expression()
    }

    fn parse_postfix_expression(&mut self) -> ParseResult<Expr> {
        let mut expression = self.parse_primary_expression()?;

        loop {
            if self.at_punctuation("(") {
                expression = self.finish_call_expression(expression)?;
                continue;
            }

            if self.match_punctuation(".").is_some() {
                let member = self.expect_identifier("expected member name after `.`")?;
                expression = Expr::Member(MemberExpr {
                    span: self.span(expression.span().start, member.span.end),
                    object: Box::new(expression),
                    member: member.value,
                    member_span: member.span,
                });
                continue;
            }

            if self.at_punctuation("[") {
                expression = self.finish_index_expression(expression)?;
                continue;
            }

            if let Some(question) = self.match_punctuation("?") {
                expression = Expr::Propagate(PropagationExpr {
                    span: self.span(expression.span().start, question.span.end),
                    operator_span: question.span,
                    expression: Box::new(expression),
                });
                continue;
            }

            if let Some(bang) = self.match_punctuation("!") {
                expression = Expr::Force(ForceExpr {
                    span: self.span(expression.span().start, bang.span.end),
                    operator_span: bang.span,
                    expression: Box::new(expression),
                });
                continue;
            }

            if let Some(catch) = self.match_keyword(Keyword::Catch) {
                let error = self.expect_identifier("expected catch binding name")?;
                let catch_block = self.parse_block()?;
                let end = catch_block.span.end;
                expression = Expr::Catch(CatchExpr {
                    span: self.span(expression.span().start, end),
                    catch_span: catch.span,
                    expression: Box::new(expression),
                    error_name: error.value,
                    error_span: error.span,
                    catch_block,
                });
                continue;
            }

            if let Some(as_token) = self.match_keyword(Keyword::As) {
                let ty = self.parse_type()?;
                expression = Expr::TypeConversion(TypeConversionExpr {
                    span: self.span(expression.span().start, ty.span().end),
                    expression: Box::new(expression),
                    as_span: as_token.span,
                    ty,
                });
                continue;
            }

            break;
        }

        Ok(expression)
    }

    fn finish_call_expression(&mut self, callee: Expr) -> ParseResult<Expr> {
        let start = callee.span().start;
        let open = self.expect_punctuation("(", "`(`")?;
        let mut arguments = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation(")") {
            if self.at_eof() {
                self.error_current("expected `)` to close argument list");
                return Err(());
            }

            arguments.push(self.parse_expression()?);
            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }

        let close = self.expect_punctuation(")", "`)`")?;
        Ok(Expr::Call(CallExpr {
            span: self.span(start, close.span.end),
            callee: Box::new(callee),
            arguments_span: self.span(open.span.start, close.span.end),
            arguments,
        }))
    }

    fn finish_index_expression(&mut self, object: Expr) -> ParseResult<Expr> {
        let start = object.span().start;
        let open = self.expect_punctuation("[", "`[`")?;
        self.skip_newlines();
        let index = self.parse_expression()?;
        self.skip_newlines();
        let close = self.expect_punctuation("]", "`]`")?;
        Ok(Expr::Index(IndexExpr {
            span: self.span(start, close.span.end),
            object: Box::new(object),
            index_span: self.span(open.span.start, close.span.end),
            index: Box::new(index),
        }))
    }

    fn finish_struct_literal_expression(&mut self, ty: TypeExpr) -> ParseResult<Expr> {
        let start = ty.span().start;
        let open = self.expect_punctuation("{", "`{`")?;
        let mut fields = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_current("expected `}` to close struct literal");
                return Err(());
            }

            let name = self.expect_identifier("expected struct literal field name")?;
            self.expect_punctuation(":", "`:`")?;
            let value = self.parse_expression()?;
            fields.push(StructLiteralField {
                span: self.span(name.span.start, value.span().end),
                name: name.value,
                name_span: name.span,
                value,
            });

            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Expr::StructLiteral(StructLiteralExpr {
            span: self.span(start, close.span.end),
            ty,
            fields_span: self.span(open.span.start, close.span.end),
            fields,
        }))
    }

    fn parse_primary_expression(&mut self) -> ParseResult<Expr> {
        match self.current().kind {
            TokenKind::Identifier => {
                let token = self.bump();
                let name = self.lexeme(&token);
                if self.looks_like_struct_literal_body() {
                    return self.finish_struct_literal_expression(TypeExpr::Reference(
                        TypeReference {
                            span: token.span,
                            name,
                        },
                    ));
                }

                Ok(Expr::Identifier(IdentifierExpr {
                    span: token.span,
                    name,
                }))
            }
            TokenKind::IntegerLiteral => {
                let token = self.bump();
                Ok(Expr::IntegerLiteral(LiteralExpr {
                    span: token.span,
                    value: self.lexeme(&token),
                }))
            }
            TokenKind::StringLiteral => {
                let token = self.bump();
                Ok(Expr::StringLiteral(LiteralExpr {
                    span: token.span,
                    value: self.lexeme(&token),
                }))
            }
            TokenKind::Keyword(Keyword::True) | TokenKind::Keyword(Keyword::False) => {
                let token = self.bump();
                Ok(Expr::BoolLiteral(LiteralExpr {
                    span: token.span,
                    value: self.lexeme(&token),
                }))
            }
            TokenKind::Keyword(Keyword::None) => {
                let token = self.bump();
                Ok(Expr::NoneLiteral(LiteralExpr {
                    span: token.span,
                    value: "none".to_string(),
                }))
            }
            TokenKind::Punctuation("[") => self.parse_array_literal_expression(),
            TokenKind::Punctuation("(") => {
                let start = self.bump();
                let expression = self.parse_expression()?;
                let end = self.expect_punctuation(")", "`)`")?;
                Ok(Expr::Group(GroupExpr {
                    span: self.span(start.span.start, end.span.end),
                    expression: Box::new(expression),
                }))
            }
            _ => {
                self.error_current("expected expression");
                Err(())
            }
        }
    }

    fn parse_array_literal_expression(&mut self) -> ParseResult<Expr> {
        let open = self.expect_punctuation("[", "`[`")?;
        let mut elements = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("]") {
            if self.at_eof() {
                self.error_current("expected `]` to close array literal");
                return Err(());
            }

            elements.push(self.parse_expression()?);
            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }

        let close = self.expect_punctuation("]", "`]`")?;
        Ok(Expr::ArrayLiteral(ArrayLiteralExpr {
            span: self.span(open.span.start, close.span.end),
            elements_span: self.span(open.span.start, close.span.end),
            elements,
        }))
    }

    fn parse_module_path(&mut self) -> ParseResult<ModulePath> {
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

    fn expect_identifier(&mut self, message: &str) -> ParseResult<ParsedIdentifier> {
        if self.current().kind == TokenKind::Identifier {
            let token = self.bump();
            return Ok(ParsedIdentifier {
                value: self.lexeme(&token),
                span: token.span,
            });
        }

        self.error_current(message);
        Err(())
    }

    fn expect_integer_literal(&mut self, message: &str) -> ParseResult<Token> {
        if self.current().kind == TokenKind::IntegerLiteral {
            return Ok(self.bump());
        }

        self.error_current(message);
        Err(())
    }

    fn expect_keyword(&mut self, keyword: Keyword, expected: &str) -> ParseResult<Token> {
        if self.at_keyword(keyword) {
            return Ok(self.bump());
        }

        self.error_current(format!("expected {expected}"));
        Err(())
    }

    fn expect_punctuation(&mut self, punctuation: &str, expected: &str) -> ParseResult<Token> {
        if self.at_punctuation(punctuation) {
            return Ok(self.bump());
        }

        self.error_current(format!("expected {expected}"));
        Err(())
    }

    fn expect_statement_end(&mut self, message: &str) -> ParseResult<()> {
        if self.at_statement_end() {
            return Ok(());
        }

        self.error_current(message);
        Err(())
    }

    fn match_keyword(&mut self, keyword: Keyword) -> Option<Token> {
        if self.at_keyword(keyword) {
            Some(self.bump())
        } else {
            None
        }
    }

    fn match_punctuation(&mut self, punctuation: &str) -> Option<Token> {
        if self.at_punctuation(punctuation) {
            Some(self.bump())
        } else {
            None
        }
    }

    fn match_logical_or_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("||") => BinaryOperator::LogicalOr,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    fn match_logical_and_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("&&") => BinaryOperator::LogicalAnd,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    fn match_equality_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("==") => BinaryOperator::Equal,
            TokenKind::Punctuation("!=") => BinaryOperator::NotEqual,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    fn match_ordering_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("<") => BinaryOperator::Less,
            TokenKind::Punctuation("<=") => BinaryOperator::LessEqual,
            TokenKind::Punctuation(">") => BinaryOperator::Greater,
            TokenKind::Punctuation(">=") => BinaryOperator::GreaterEqual,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    fn match_shift_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("<<") => BinaryOperator::ShiftLeft,
            TokenKind::Punctuation(">>") => BinaryOperator::ShiftRight,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    fn match_additive_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("+") => BinaryOperator::Add,
            TokenKind::Punctuation("-") => BinaryOperator::Subtract,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    fn match_multiplicative_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("*") => BinaryOperator::Multiply,
            TokenKind::Punctuation("/") => BinaryOperator::Divide,
            TokenKind::Punctuation("%") => BinaryOperator::Remainder,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    fn match_unary_operator(&mut self) -> Option<ParsedUnaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("!") => UnaryOperator::LogicalNot,
            TokenKind::Punctuation("-") => UnaryOperator::Negate,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedUnaryOperator {
            value,
            span: token.span,
        })
    }

    fn at_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current().kind, TokenKind::Keyword(actual) if actual == keyword)
    }

    fn at_punctuation(&self, punctuation: &str) -> bool {
        matches!(self.current().kind, TokenKind::Punctuation(actual) if actual == punctuation)
    }

    fn looks_like_struct_literal_body(&self) -> bool {
        if !self.at_punctuation("{") {
            return false;
        }

        let mut index = self.index + 1;
        while matches!(
            self.tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Newline)
        ) {
            index += 1;
        }

        if !matches!(
            self.tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Identifier)
        ) {
            return false;
        }

        index += 1;
        while matches!(
            self.tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Newline)
        ) {
            index += 1;
        }

        matches!(
            self.tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Punctuation(":"))
        )
    }

    fn at_statement_end(&self) -> bool {
        self.current().kind == TokenKind::Newline || self.at_punctuation("}") || self.at_eof()
    }

    fn at_eof(&self) -> bool {
        self.current().kind == TokenKind::Eof
    }

    fn skip_newlines(&mut self) {
        while self.current().kind == TokenKind::Newline {
            self.index += 1;
        }
    }

    fn current(&self) -> &Token {
        self.tokens
            .get(self.index)
            .unwrap_or_else(|| self.tokens.last().expect("parser requires an EOF token"))
    }

    fn bump(&mut self) -> Token {
        let token = self.current().clone();
        if !self.at_eof() {
            self.index += 1;
        }
        token
    }

    fn span(&self, start: usize, end: usize) -> ByteSpan {
        ByteSpan::new(self.source, start, end)
    }

    fn lexeme(&self, token: &Token) -> String {
        self.sources
            .get(token.span.source)
            .and_then(|file| file.text().get(token.span.start..token.span.end))
            .unwrap_or("")
            .to_string()
    }

    fn error_current(&mut self, message: impl Into<String>) {
        let token = self.current();
        self.error_at(token.span, message);
    }

    fn error_at(&mut self, span: ByteSpan, message: impl Into<String>) {
        let primary_span = self
            .sources
            .span_to_json(ByteSpan::new(span.source, span.start, span.end))
            .ok()
            .map(Box::new);
        let mut diagnostic = Diagnostic::error("E0200", message);
        diagnostic.primary_span = primary_span;
        self.diagnostics.push(diagnostic);
    }
}

#[derive(Debug, Clone)]
struct ParsedIdentifier {
    value: String,
    span: ByteSpan,
}

#[derive(Debug, Clone)]
struct ParsedEnumPattern {
    span: ByteSpan,
    enum_name: String,
    enum_name_span: ByteSpan,
    variant_name: String,
    variant_name_span: ByteSpan,
    payload: Option<SwitchPayloadBinding>,
}

#[derive(Debug, Clone)]
struct ParsedBinaryOperator {
    value: BinaryOperator,
    span: ByteSpan,
}

struct ParsedUnaryOperator {
    value: UnaryOperator,
    span: ByteSpan,
}

fn with_type_span(ty: TypeExpr, span: ByteSpan) -> TypeExpr {
    match ty {
        TypeExpr::Reference(mut ty) => {
            ty.span = span;
            TypeExpr::Reference(ty)
        }
        TypeExpr::Generic(mut ty) => {
            ty.span = span;
            TypeExpr::Generic(ty)
        }
        TypeExpr::Pointer(mut ty) => {
            ty.span = span;
            TypeExpr::Pointer(ty)
        }
        TypeExpr::Borrow(mut ty) => {
            ty.span = span;
            TypeExpr::Borrow(ty)
        }
        TypeExpr::View(mut ty) => {
            ty.span = span;
            TypeExpr::View(ty)
        }
        TypeExpr::Array(mut ty) => {
            ty.span = span;
            TypeExpr::Array(ty)
        }
        TypeExpr::Optional(mut ty) => {
            ty.span = span;
            TypeExpr::Optional(ty)
        }
        TypeExpr::Fallible(mut ty) => {
            ty.span = span;
            TypeExpr::Fallible(ty)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ast::{Expr, ImplMember, Item, JsonAstNode, Stmt, TypeExpr, Visibility};
    use crate::lexer::lex;
    use crate::source::SourceMap;

    fn parse_text(text: &str) -> ParseOutput {
        let (_, output) = parse_text_with_sources(text);
        output
    }

    fn parse_text_with_sources(text: &str) -> (SourceMap, ParseOutput) {
        let mut sources = SourceMap::new();
        let source = sources.add_source("app.nct", None, text);
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let output = parse(&sources, source, &lexed.tokens);
        (sources, output)
    }

    fn find_json_node<'a>(node: &'a JsonAstNode, kind: &str) -> Option<&'a JsonAstNode> {
        if node.kind == kind {
            return Some(node);
        }

        node.items
            .iter()
            .find_map(|child| find_json_node(child, kind))
    }

    #[test]
    fn parses_hello_program() {
        let output = parse_text(
            r#"use std/prelude

from std/io import print

program(): i32 {
    print("Hello") catch error {
        return 1
    }

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        assert_eq!(ast.items.len(), 3);
        assert!(matches!(ast.items[0], Item::Use(_)));
        assert!(matches!(ast.items[1], Item::FromImport(_)));
        assert!(matches!(ast.items[2], Item::Program(_)));
    }

    #[test]
    fn parses_import_aliases() {
        let output = parse_text(
            r#"import std/io as io
from std/io import File as StdFile, stdout
pub from std/string import String as StdString

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Import(import) = &ast.items[0] else {
            panic!("expected namespace import");
        };
        let Item::FromImport(from_import) = &ast.items[1] else {
            panic!("expected from import");
        };
        let Item::FromImport(reexport) = &ast.items[2] else {
            panic!("expected public re-export");
        };

        assert_eq!(import.path.value, "std/io");
        assert_eq!(import.alias.name, "io");
        assert_eq!(from_import.names[0].name, "File");
        assert_eq!(from_import.names[0].local_name(), "StdFile");
        assert_eq!(from_import.names[1].name, "stdout");
        assert_eq!(from_import.names[1].local_name(), "stdout");
        assert_eq!(reexport.visibility, Visibility::Public);
        assert_eq!(reexport.names[0].local_name(), "StdString");
    }

    #[test]
    fn parses_impl_trait_methods_and_generic_bounds() {
        let output = parse_text(
            r#"pub struct Counter {
    value: i32
}

impl Counter {
    pub func zero(): i32 {
        return 0
    }

    pub method (counter: &+Self).add(value: i32): void {
        return
    }
}

pub trait Writer {
    method (writer: &+Self).write(text: str): void!
}

impl Writer for Counter {
    method (counter: &+Self).write(text: str): void! {
        return
    }
}

func print<W: Writer>(writer: &+W): void! {
    return
}

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();

        let Item::Impl(inherent_impl) = &ast.items[1] else {
            panic!("expected inherent impl");
        };
        assert!(inherent_impl.trait_ty.is_none());
        assert!(matches!(
            &inherent_impl.target_ty,
            TypeExpr::Reference(reference) if reference.name == "Counter"
        ));
        assert!(matches!(
            &inherent_impl.members[0],
            ImplMember::Function(function) if function.name == "zero"
        ));
        let ImplMember::Method(method) = &inherent_impl.members[1] else {
            panic!("expected method");
        };
        assert_eq!(method.name, "add");
        assert!(method.body.is_some());
        assert!(matches!(&method.receiver.ty, TypeExpr::Borrow(_)));

        let Item::Trait(trait_) = &ast.items[2] else {
            panic!("expected trait");
        };
        assert_eq!(trait_.visibility, Visibility::Public);
        assert_eq!(trait_.name, "Writer");
        assert_eq!(trait_.methods.len(), 1);
        assert_eq!(trait_.methods[0].name, "write");
        assert!(trait_.methods[0].body.is_none());

        let Item::Impl(trait_impl) = &ast.items[3] else {
            panic!("expected trait impl");
        };
        assert!(trait_impl.trait_ty.is_some());
        assert!(matches!(
            &trait_impl.target_ty,
            TypeExpr::Reference(reference) if reference.name == "Counter"
        ));

        let Item::Function(function) = &ast.items[4] else {
            panic!("expected generic function");
        };
        assert_eq!(function.generics.parameters.len(), 1);
        assert_eq!(function.generics.parameters[0].name, "W");
        assert!(matches!(
            &function.generics.parameters[0].bound,
            Some(TypeExpr::Reference(reference)) if reference.name == "Writer"
        ));
    }

    #[test]
    fn parses_function_with_fallible_return_type() {
        let output = parse_text(
            r#"use std/prelude

from std/io import print

program(): i32 {
    run() catch error {
        return 1
    }

    return 0
}

func run(): void! {
    print("Hello")?
    return
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Some(Item::Function(function)) = ast.items.last() else {
            panic!("expected final item to be a function");
        };
        assert!(matches!(function.return_type, TypeExpr::Fallible(_)));
    }

    #[test]
    fn parses_grouped_optional_fallible_return_type() {
        let output = parse_text(
            r#"func env(name: str): (str?)! {
    return none
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Some(Item::Function(function)) = ast.items.first() else {
            panic!("expected function item");
        };
        let TypeExpr::Fallible(fallible) = &function.return_type else {
            panic!("expected fallible return type");
        };
        assert!(matches!(fallible.success.as_ref(), TypeExpr::Optional(_)));
    }

    #[test]
    fn parses_optional_default_expression() {
        let output = parse_text(
            r#"use std/prelude

program(): i32 {
    let user = (env("USER") catch error {
        return 1
    }) ?? "unknown"

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[1] else {
            panic!("expected program item");
        };
        let Stmt::Binding(binding) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        let Expr::OptionalDefault(expression) = &binding.initializer else {
            panic!("expected optional default expression");
        };
        assert_eq!(expression.operator_span.len(), 2);
        assert!(expression.span.start < expression.operator_span.start);
        assert!(expression.operator_span.end < expression.span.end);
    }

    #[test]
    fn parses_force_unwrap_expression() {
        let output = parse_text(
            r#"program(): i32 {
    return answer()!
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Return(statement) = &program.body.statements[0] else {
            panic!("expected return statement");
        };
        let Some(Expr::Force(expression)) = &statement.expression else {
            panic!("expected force unwrap expression");
        };
        assert_eq!(expression.operator_span.len(), 1);
        assert!(expression.span.start < expression.operator_span.start);
        assert_eq!(expression.span.end, expression.operator_span.end);
    }

    #[test]
    fn ast_json_includes_expression_operator_spans() {
        let (sources, output) = parse_text_with_sources(
            r#"program(): i32 {
    let value = maybe() ?? 0
    let handled = answer() catch error {
        return 1
    }
    return handled!
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let json = output.ast.unwrap().to_json(&sources);
        let optional_default = find_json_node(&json, "optional_default_expression")
            .expect("expected optional default expression");
        let optional_default_span = optional_default.operator_span.as_ref().unwrap();
        assert_eq!(
            optional_default_span.end_byte - optional_default_span.start_byte,
            2
        );

        let catch = find_json_node(&json, "fallible_catch_expression")
            .expect("expected fallible catch expression");
        let catch_span = catch.operator_span.as_ref().unwrap();
        assert_eq!(catch_span.end_byte - catch_span.start_byte, "catch".len());

        let force = find_json_node(&json, "force_unwrap_expression")
            .expect("expected force unwrap expression");
        let force_span = force.operator_span.as_ref().unwrap();
        assert_eq!(force_span.end_byte - force_span.start_byte, 1);
    }

    #[test]
    fn parses_optional_let_else_binding() {
        let output = parse_text(
            r#"program(): i32 {
    let home = lookup("HOME") else {
        return 1
    }

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Binding(binding) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };

        assert_eq!(binding.kind, BindingKind::Let);
        assert!(binding.else_block.is_some());
        assert!(matches!(binding.initializer, Expr::Call(_)));
    }

    #[test]
    fn parses_optional_var_else_binding() {
        let output = parse_text(
            r#"program(): i32 {
    var text = maybe_text else {
        return 1
    }

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Binding(binding) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };

        assert_eq!(binding.kind, BindingKind::Var);
        assert!(binding.else_block.is_some());
    }

    #[test]
    fn parses_optional_if_let_and_if_var_statements() {
        let output = parse_text(
            r#"program(): i32 {
    if let home = maybe_home {
        return 0
    } else {
        return 1
    }

    if var text = maybe_text {
        return 0
    }

    return 1
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::IfLet(first) = &program.body.statements[0] else {
            panic!("expected if let statement");
        };
        let Stmt::IfLet(second) = &program.body.statements[1] else {
            panic!("expected if var statement");
        };

        assert_eq!(first.kind, BindingKind::Let);
        assert_eq!(first.name, "home");
        assert!(first.else_block.is_some());
        assert_eq!(second.kind, BindingKind::Var);
        assert_eq!(second.name, "text");
        assert!(second.else_block.is_none());
    }

    #[test]
    fn parses_else_if_chains_as_nested_else_blocks() {
        let output = parse_text(
            r#"program(): i32 {
    if ready {
        return 0
    } else if let value = maybe_value {
        return value
    } else if var fallback = maybe_fallback {
        return fallback
    } else {
        return 3
    }
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::If(first) = &program.body.statements[0] else {
            panic!("expected if statement");
        };
        let first_else = first.else_block.as_ref().expect("expected else block");
        let Stmt::IfLet(second) = &first_else.statements[0] else {
            panic!("expected nested if let statement");
        };
        let second_else = second
            .else_block
            .as_ref()
            .expect("expected second else block");
        let Stmt::IfLet(third) = &second_else.statements[0] else {
            panic!("expected nested if var statement");
        };

        assert_eq!(second.kind, BindingKind::Let);
        assert_eq!(second.name, "value");
        assert_eq!(third.kind, BindingKind::Var);
        assert_eq!(third.name, "fallback");
        assert!(third.else_block.is_some());
    }

    #[test]
    fn parses_while_and_optional_while_statements() {
        let output = parse_text(
            r#"program(): i32 {
    while ready {
        tick()
    }

    while let value = next_value {
        use_value(value)
    }

    while var text = next_text {
        use_text(text)
    }

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::While(first) = &program.body.statements[0] else {
            panic!("expected while statement");
        };
        let Stmt::WhileLet(second) = &program.body.statements[1] else {
            panic!("expected while let statement");
        };
        let Stmt::WhileLet(third) = &program.body.statements[2] else {
            panic!("expected while var statement");
        };

        assert!(matches!(first.condition, Expr::Identifier(_)));
        assert_eq!(second.kind, BindingKind::Let);
        assert_eq!(second.name, "value");
        assert_eq!(third.kind, BindingKind::Var);
        assert_eq!(third.name, "text");
    }

    #[test]
    fn parses_break_and_continue_statements() {
        let output = parse_text(
            r#"program(): i32 {
    while ready {
        break
    }

    while waiting {
        continue
    }

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::While(first) = &program.body.statements[0] else {
            panic!("expected while statement");
        };
        let Stmt::While(second) = &program.body.statements[1] else {
            panic!("expected while statement");
        };

        assert!(matches!(first.body.statements[0], Stmt::Break(_)));
        assert!(matches!(second.body.statements[0], Stmt::Continue(_)));
    }

    #[test]
    fn parses_loop_statement() {
        let output = parse_text(
            r#"program(): i32 {
    loop {
        continue
    }

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Loop(statement) = &program.body.statements[0] else {
            panic!("expected loop statement");
        };

        assert!(matches!(statement.body.statements[0], Stmt::Continue(_)));
    }

    #[test]
    fn parses_fail_statement() {
        let output = parse_text(
            r#"program(): i32 {
    return 0
}

func run(error: error): i32! {
    fail error
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Function(function) = &ast.items[1] else {
            panic!("expected function item");
        };
        assert!(matches!(function.body.statements[0], Stmt::Fail(_)));
    }

    #[test]
    fn parses_switch_statement() {
        let output = parse_text(
            r#"enum AppError {
    missing_path
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is AppError.missing_path {
            return 1
        }

        is AppError.open_failed(path) {
            return 2
        }
    }

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Function(function) = &ast.items[2] else {
            panic!("expected function item");
        };
        let Stmt::Switch(statement) = &function.body.statements[0] else {
            panic!("expected switch statement");
        };

        assert_eq!(statement.arms.len(), 2);
        assert!(statement.arms[0].payload.is_none());
        assert_eq!(
            statement.arms[1]
                .payload
                .as_ref()
                .map(|payload| payload.name.as_str()),
            Some("path")
        );
        assert!(statement.else_arm.is_none());
    }

    #[test]
    fn parses_switch_else_arm() {
        let output = parse_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        is AppError.missing_path {
            return 1
        }

        else {
            return 0
        }
    }
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Function(function) = &ast.items[2] else {
            panic!("expected function item");
        };
        let Stmt::Switch(statement) = &function.body.statements[0] else {
            panic!("expected switch statement");
        };

        assert_eq!(statement.arms.len(), 1);
        assert!(statement.else_arm.is_some());
    }

    #[test]
    fn parses_if_is_statement() {
        let output = parse_text(
            r#"enum AppError {
    missing_path
    open_failed(path: str)
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    if error is AppError.open_failed(path) {
        return 1
    } else if error is AppError.missing_path {
        return 2
    } else {
        return 0
    }
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Function(function) = &ast.items[2] else {
            panic!("expected function item");
        };
        let Stmt::IfIs(statement) = &function.body.statements[0] else {
            panic!("expected if-is statement");
        };

        assert_eq!(statement.enum_name, "AppError");
        assert_eq!(statement.variant_name, "open_failed");
        assert_eq!(
            statement
                .payload
                .as_ref()
                .map(|payload| payload.name.as_str()),
            Some("path")
        );
        let Some(else_block) = &statement.else_block else {
            panic!("expected else block");
        };
        assert!(matches!(else_block.statements[0], Stmt::IfIs(_)));
    }

    #[test]
    fn rejects_switch_arm_after_else() {
        let output = parse_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        else {
            return 0
        }

        is AppError.missing_path {
            return 1
        }
    }
}
"#,
        );

        assert_eq!(output.diagnostics.len(), 1);
        assert!(output.diagnostics[0].message.contains("last"));
    }

    #[test]
    fn rejects_duplicate_switch_else_arm() {
        let output = parse_text(
            r#"enum AppError {
    missing_path
}

program(): i32 {
    return 0
}

func code(error: AppError): i32 {
    switch error {
        else {
            return 0
        }

        else {
            return 1
        }
    }
}
"#,
        );

        assert_eq!(output.diagnostics.len(), 1);
        assert!(output.diagnostics[0].message.contains("only one"));
    }

    #[test]
    fn parses_range_for_statement() {
        let output = parse_text(
            r#"program(): i32 {
    for i in 0..<4 {
        use_value(i)
    }

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::ForRange(statement) = &program.body.statements[0] else {
            panic!("expected range for statement");
        };

        assert_eq!(statement.name, "i");
        assert!(matches!(statement.start, Expr::IntegerLiteral(_)));
        assert!(matches!(statement.end, Expr::IntegerLiteral(_)));
        assert_eq!(statement.body.statements.len(), 1);
    }

    #[test]
    fn rejects_non_range_for_statement() {
        let output = parse_text(
            r#"program(): void {
    for item in items {
    }
}
"#,
        );

        assert_eq!(output.diagnostics.len(), 1);
        assert!(output.diagnostics[0].message.contains("..<"));
    }

    #[test]
    fn rejects_loop_control_with_values() {
        let output = parse_text(
            r#"program(): void {
    while ready {
        break 1
    }
}
"#,
        );

        assert_eq!(output.diagnostics.len(), 1);
        assert!(
            output.diagnostics[0]
                .message
                .contains("does not take a value")
        );
    }

    #[test]
    fn parses_builtin_view_and_array_types() {
        let output = parse_text(
            r#"pub func checksum(bytes: [u8], output: [+u8], header: [u8; 4]): str {
    return "ok"
}

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Function(function) = &ast.items[0] else {
            panic!("expected function declaration");
        };

        assert!(matches!(
            &function.parameters.parameters[0].ty,
            TypeExpr::View(view) if !view.is_readwrite
        ));
        assert!(matches!(
            &function.parameters.parameters[1].ty,
            TypeExpr::View(view) if view.is_readwrite
        ));
        assert!(matches!(
            &function.parameters.parameters[2].ty,
            TypeExpr::Array(array) if array.length.value == "4"
        ));
        assert!(
            matches!(&function.return_type, TypeExpr::Reference(reference) if reference.name == "str")
        );
    }

    #[test]
    fn parses_array_literal_expression() {
        let output = parse_text(
            r#"program(): i32 {
    let header = [
        0x7F,
        0x45,
        0x4C,
        0x46,
    ]
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Binding(binding) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        let Expr::ArrayLiteral(array) = &binding.initializer else {
            panic!("expected array literal");
        };
        assert_eq!(array.elements.len(), 4);
    }

    #[test]
    fn parses_struct_literal_expression() {
        let output = parse_text(
            r#"struct Point {
    x: i32
    label: str
}

program(): i32 {
    let point = Point{
        x: 1,
        label: "home",
    }
    return point.x
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[1] else {
            panic!("expected program item");
        };
        let Stmt::Binding(binding) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        let Expr::StructLiteral(literal) = &binding.initializer else {
            panic!("expected struct literal");
        };
        assert_eq!(literal.fields.len(), 2);
        assert_eq!(literal.fields[0].name, "x");
        assert_eq!(literal.fields[1].name, "label");
    }

    #[test]
    fn parses_index_expression() {
        let output = parse_text(
            r#"program(): i32 {
    let byte = header[0]
    let next = matrix[0][1]
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Binding(first) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        assert!(matches!(first.initializer, Expr::Index(_)));

        let Stmt::Binding(second) = &program.body.statements[1] else {
            panic!("expected binding statement");
        };
        let Expr::Index(outer) = &second.initializer else {
            panic!("expected outer index expression");
        };
        assert!(matches!(outer.object.as_ref(), Expr::Index(_)));
    }

    #[test]
    fn parses_if_else_statement_and_bool_literals() {
        let output = parse_text(
            r#"program(): i32 {
    if true {
        return 0
    } else {
        return 1
    }
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::If(statement) = &program.body.statements[0] else {
            panic!("expected if statement");
        };
        assert!(matches!(statement.condition, Expr::BoolLiteral(_)));
        assert!(statement.else_block.is_some());
    }

    #[test]
    fn parses_comparison_expressions() {
        let output = parse_text(
            r#"program(): i32 {
    let nonempty = count > 0
    let same = bytes[0] == 0
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Binding(first) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        assert!(matches!(first.initializer, Expr::Binary(_)));

        let Stmt::Binding(second) = &program.body.statements[1] else {
            panic!("expected binding statement");
        };
        let Expr::Binary(binary) = &second.initializer else {
            panic!("expected binary expression");
        };
        assert_eq!(binary.operator, BinaryOperator::Equal);
    }

    #[test]
    fn parses_arithmetic_expression_precedence() {
        let output = parse_text(
            r#"program(): i32 {
    let value = 1 + 2 * 3 - 4 / 2 % 2
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Binding(statement) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        let Expr::Binary(subtract_expression) = &statement.initializer else {
            panic!("expected top-level subtraction expression");
        };
        assert_eq!(subtract_expression.operator, BinaryOperator::Subtract);

        let Expr::Binary(add_expression) = subtract_expression.left.as_ref() else {
            panic!("expected addition on the left side of subtraction");
        };
        assert_eq!(add_expression.operator, BinaryOperator::Add);

        let Expr::Binary(multiply_expression) = add_expression.right.as_ref() else {
            panic!("expected multiplication on the right side of addition");
        };
        assert_eq!(multiply_expression.operator, BinaryOperator::Multiply);

        let Expr::Binary(remainder_expression) = subtract_expression.right.as_ref() else {
            panic!("expected remainder on the right side of subtraction");
        };
        assert_eq!(remainder_expression.operator, BinaryOperator::Remainder);

        let Expr::Binary(divide_expression) = remainder_expression.left.as_ref() else {
            panic!("expected division on the left side of remainder");
        };
        assert_eq!(divide_expression.operator, BinaryOperator::Divide);
    }

    #[test]
    fn parses_type_conversion_expression_precedence() {
        let output = parse_text(
            r#"program(): i32 {
    let value = byte as u64 + 1
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Binding(statement) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        let Expr::Binary(add_expression) = &statement.initializer else {
            panic!("expected top-level addition expression");
        };
        assert_eq!(add_expression.operator, BinaryOperator::Add);
        assert!(matches!(
            add_expression.left.as_ref(),
            Expr::TypeConversion(_)
        ));
    }

    #[test]
    fn parses_shift_expression_precedence() {
        let output = parse_text(
            r#"program(): i32 {
    let outside = value + 1 << count * 2 < limit
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Binding(statement) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        let Expr::Binary(ordering_expression) = &statement.initializer else {
            panic!("expected top-level ordering expression");
        };
        assert_eq!(ordering_expression.operator, BinaryOperator::Less);

        let Expr::Binary(shift_expression) = ordering_expression.left.as_ref() else {
            panic!("expected shift expression on the left side of ordering expression");
        };
        assert_eq!(shift_expression.operator, BinaryOperator::ShiftLeft);

        let Expr::Binary(add_expression) = shift_expression.left.as_ref() else {
            panic!("expected addition on the left side of shift expression");
        };
        assert_eq!(add_expression.operator, BinaryOperator::Add);

        let Expr::Binary(multiply_expression) = shift_expression.right.as_ref() else {
            panic!("expected multiplication on the right side of shift expression");
        };
        assert_eq!(multiply_expression.operator, BinaryOperator::Multiply);
    }

    #[test]
    fn parses_logical_expression_precedence() {
        let output = parse_text(
            r#"program(): i32 {
    let condition = count > 0 && ready || fallback
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Binding(statement) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        let Expr::Binary(or_expression) = &statement.initializer else {
            panic!("expected top-level logical or expression");
        };
        assert_eq!(or_expression.operator, BinaryOperator::LogicalOr);

        let Expr::Binary(and_expression) = or_expression.left.as_ref() else {
            panic!("expected logical and on the left side of logical or");
        };
        assert_eq!(and_expression.operator, BinaryOperator::LogicalAnd);

        let Expr::Binary(ordering_expression) = and_expression.left.as_ref() else {
            panic!("expected ordering expression on the left side of logical and");
        };
        assert_eq!(ordering_expression.operator, BinaryOperator::Greater);
    }

    #[test]
    fn parses_logical_not_expression_precedence() {
        let output = parse_text(
            r#"program(): i32 {
    let condition = !ready && fallback
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Binding(statement) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        let Expr::Binary(and_expression) = &statement.initializer else {
            panic!("expected logical and expression");
        };
        assert_eq!(and_expression.operator, BinaryOperator::LogicalAnd);

        let Expr::Unary(not_expression) = and_expression.left.as_ref() else {
            panic!("expected logical not on the left side of logical and");
        };
        assert_eq!(not_expression.operator, UnaryOperator::LogicalNot);
    }

    #[test]
    fn parses_numeric_negate_expression_precedence() {
        let output = parse_text(
            r#"program(): i32 {
    let smaller = -count < 0
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::Program(program) = &ast.items[0] else {
            panic!("expected program item");
        };
        let Stmt::Binding(statement) = &program.body.statements[0] else {
            panic!("expected binding statement");
        };
        let Expr::Binary(ordering_expression) = &statement.initializer else {
            panic!("expected ordering expression");
        };
        assert_eq!(ordering_expression.operator, BinaryOperator::Less);

        let Expr::Unary(negate_expression) = ordering_expression.left.as_ref() else {
            panic!("expected numeric negation on the left side of ordering expression");
        };
        assert_eq!(negate_expression.operator, UnaryOperator::Negate);
    }

    #[test]
    fn parses_relative_import_paths() {
        let output = parse_text(
            r#"from ./config import Config
from ../shared/path import Path

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::FromImport(config) = &ast.items[0] else {
            panic!("expected first item to be a relative import");
        };
        let Item::FromImport(path) = &ast.items[1] else {
            panic!("expected second item to be a relative import");
        };

        assert_eq!(config.path.value, "./config");
        assert_eq!(config.visibility, Visibility::Private);
        assert_eq!(path.path.value, "../shared/path");
        assert_eq!(path.visibility, Visibility::Private);
    }

    #[test]
    fn parses_public_reexports() {
        let output = parse_text(
            r#"pub from std/string import String

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        let Item::FromImport(import) = &ast.items[0] else {
            panic!("expected first item to be a public re-export");
        };

        assert_eq!(import.visibility, Visibility::Public);
        assert_eq!(import.names.len(), 1);
    }

    #[test]
    fn parses_top_level_type_and_primitive_declarations() {
        let output = parse_text(
            r#"pub type Bytes = [u8]

pub copy struct Layout {
    size: usize
    align: usize
}

pub enum IOError {
    not_found(path: str)
    denied
}

pub(nocter) primitive addr<T>(pointer: *T): usize

pub func write(file: &+File, text: str): void! {
    return
}

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let ast = output.ast.unwrap();
        assert_eq!(ast.items.len(), 6);

        let Item::TypeAlias(alias) = &ast.items[0] else {
            panic!("expected type alias");
        };
        assert_eq!(alias.visibility, Visibility::Public);
        assert!(matches!(&alias.target, TypeExpr::View(_)));

        let Item::Struct(struct_) = &ast.items[1] else {
            panic!("expected struct declaration");
        };
        assert!(struct_.is_copy);
        assert_eq!(struct_.fields.len(), 2);

        let Item::Enum(enum_) = &ast.items[2] else {
            panic!("expected enum declaration");
        };
        assert_eq!(enum_.variants.len(), 2);
        assert_eq!(enum_.variants[0].payload.len(), 1);

        let Item::Primitive(primitive) = &ast.items[3] else {
            panic!("expected primitive declaration");
        };
        assert_eq!(primitive.visibility, Visibility::Nocter);
        assert_eq!(primitive.generics.parameters.len(), 1);
        assert!(matches!(
            &primitive.parameters.parameters[0].ty,
            TypeExpr::Pointer(_)
        ));

        let Item::Function(function) = &ast.items[4] else {
            panic!("expected function declaration");
        };
        assert!(matches!(
            &function.parameters.parameters[0].ty,
            TypeExpr::Borrow(borrow) if borrow.is_readwrite
        ));
        assert!(matches!(&function.return_type, TypeExpr::Fallible(_)));
    }

    #[test]
    fn diagnoses_unknown_top_level_item() {
        let output = parse_text(
            r#"module app/main

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.ast.is_none());
        assert_eq!(output.diagnostics.len(), 1);
        assert!(output.diagnostics[0].message.contains("top-level item"));
    }
}
