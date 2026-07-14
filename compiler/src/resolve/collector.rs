use super::builtins::is_reserved_type_declaration_name;
use super::diagnostics::{
    builtin_type_declaration_name_reuse_diagnostic, duplicate_visible_name_diagnostic,
};
use super::signatures::{
    alias_type_symbol, associated_function_signatures, drop_signature,
    duplicate_inherent_drop_diagnostics, duplicate_inherent_member_name_diagnostics,
    enum_type_symbol, function_signature, impl_target_type_name, method_signatures,
    nominal_type_symbol, primitive_signature, struct_type_symbol,
    type_symbol_accepts_inherent_impl,
};
use super::{Resolver, SymbolKind, TypeSymbol, TypeSymbolKind};
use crate::ast::{AstFile, FunctionDecl, ImplDecl, Item, PrimitiveDecl};
use crate::source::ByteSpan;

impl Resolver<'_> {
    pub(super) fn collect_top_level_symbols(&mut self, ast: &AstFile) {
        for item in &ast.items {
            match item {
                Item::Use(item) => self.collect_use_symbols(item),
                Item::Import(item) => self.collect_import_namespace_symbol(item),
                Item::FromImport(item) => self.collect_imported_symbols(item),
                Item::Function(function) => self.collect_function_symbol(function),
                Item::Primitive(primitive) => self.collect_primitive_symbol(primitive),
                Item::TypeAlias(alias) => self.collect_type_symbol(
                    alias.name.clone(),
                    alias.name_span,
                    alias.span,
                    alias_type_symbol(alias.name.clone(), alias.target.clone()),
                ),
                Item::Struct(struct_) => self.collect_type_symbol(
                    struct_.name.clone(),
                    struct_.name_span,
                    struct_.span,
                    struct_type_symbol(struct_.name.clone(), struct_.is_copy, &struct_.fields),
                ),
                Item::Enum(enum_) => self.collect_type_symbol(
                    enum_.name.clone(),
                    enum_.name_span,
                    enum_.span,
                    enum_type_symbol(enum_.name.clone(), &enum_.variants),
                ),
                Item::Trait(trait_) => self.collect_type_symbol(
                    trait_.name.clone(),
                    trait_.name_span,
                    trait_.span,
                    nominal_type_symbol(trait_.name.clone(), TypeSymbolKind::Trait),
                ),
                Item::Impl(_) => {}
            }
        }

        for item in &ast.items {
            if let Item::Impl(impl_) = item {
                self.collect_inherent_impl_members(impl_);
            }
        }
    }

    pub(super) fn define_symbol(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        kind: SymbolKind,
    ) {
        if let Err(first_id) =
            self.output
                .symbols
                .define(name.clone(), name_span, declaration_span, kind)
            && let Some(first) = self.output.symbols.get(first_id)
        {
            self.output
                .diagnostics
                .push(duplicate_visible_name_diagnostic(
                    self.sources,
                    &name,
                    first.name_span,
                    name_span,
                ));
        }
    }

    fn collect_function_symbol(&mut self, function: &FunctionDecl) {
        self.define_symbol(
            function.name.clone(),
            function.name_span,
            function.name_span,
            SymbolKind::Function(function_signature(function)),
        );
    }

    fn collect_primitive_symbol(&mut self, primitive: &PrimitiveDecl) {
        self.define_symbol(
            primitive.name.clone(),
            primitive.name_span,
            primitive.name_span,
            SymbolKind::Primitive(primitive_signature(primitive)),
        );
    }

    fn collect_type_symbol(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        symbol: TypeSymbol,
    ) {
        if is_reserved_type_declaration_name(&name) {
            self.output
                .diagnostics
                .push(builtin_type_declaration_name_reuse_diagnostic(
                    self.sources,
                    &name,
                    name_span,
                ));
            return;
        }

        self.define_symbol(name, name_span, declaration_span, SymbolKind::Type(symbol));
    }

    fn collect_inherent_impl_members(&mut self, impl_: &ImplDecl) {
        if impl_.trait_ty.is_some() {
            return;
        }

        let Some(target_name) = impl_target_type_name(&impl_.target_ty) else {
            return;
        };
        let Some(symbol_id) = self.output.symbols.by_name.get(target_name).copied() else {
            return;
        };
        let Some(symbol) = self
            .output
            .symbols
            .symbols
            .get_mut(symbol_id.raw() as usize)
        else {
            return;
        };
        let diagnostics = {
            let SymbolKind::Type(type_symbol) = &mut symbol.kind else {
                return;
            };

            if !type_symbol_accepts_inherent_impl(type_symbol) {
                return;
            }

            let mut associated_functions =
                associated_function_signatures(impl_).collect::<Vec<_>>();
            let mut methods = method_signatures(impl_).collect::<Vec<_>>();
            let mut diagnostics = duplicate_inherent_member_name_diagnostics(
                self.sources,
                target_name,
                type_symbol,
                impl_,
            );
            diagnostics.extend(duplicate_inherent_drop_diagnostics(
                self.sources,
                target_name,
                type_symbol,
                impl_,
            ));
            type_symbol
                .associated_functions
                .append(&mut associated_functions);
            type_symbol.methods.append(&mut methods);
            if type_symbol.drop_member.is_none() {
                type_symbol.drop_member = drop_signature(impl_);
            }
            diagnostics
        };
        self.output.diagnostics.extend(diagnostics);
    }
}
