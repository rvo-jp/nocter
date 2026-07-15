use super::diagnostics::{
    missing_import_diagnostic, restricted_import_diagnostic, unloaded_import_diagnostic,
};
use super::module_index::is_relative_module_path;
use super::signatures::{
    alias_type_symbol, attach_inherent_impl_members_to_symbol, enum_type_symbol,
    function_signature, nominal_type_symbol, primitive_signature, struct_type_symbol,
};
use super::{ImportAccess, ImportedSymbol, Resolver, SymbolKind, TypeSymbol, TypeSymbolKind};
use crate::ast::{AstFile, FromImportItem, ImportItem, Item, TypeAliasDecl, UseItem, Visibility};
use crate::source::ByteSpan;

impl Resolver<'_> {
    pub(super) fn collect_use_symbols(&mut self, item: &UseItem) {
        let Some((imported_ast, import_source)) = self
            .module_index
            .import_ast_for_span(item.path.span, self.import_sources)
        else {
            return;
        };

        self.collect_public_exports(imported_ast, import_source.access, &item.path.value);
    }

    pub(super) fn collect_import_namespace_symbol(&mut self, item: &ImportItem) {
        if self
            .module_index
            .import_ast_for_span(item.path.span, self.import_sources)
            .is_none()
            && is_relative_module_path(&item.path.value)
        {
            self.output.diagnostics.push(unloaded_import_diagnostic(
                self.sources,
                &item.path.value,
                item.alias.span,
            ));
            return;
        }

        self.define_symbol(
            item.alias.name.clone(),
            item.alias.span,
            item.path.span,
            SymbolKind::Imported(ImportedSymbol {
                path: item.path.value.clone(),
            }),
        );
    }

    pub(super) fn collect_imported_symbols(&mut self, item: &FromImportItem) {
        if let Some((imported_ast, import_source)) =
            self.module_index.import_ast(item, self.import_sources)
        {
            self.collect_loaded_imported_symbols(item, imported_ast, import_source.access);
            return;
        }

        if is_relative_module_path(&item.path.value) {
            self.report_unloaded_imported_symbols(item);
            return;
        }

        for name in &item.names {
            self.define_symbol(
                name.local_name().to_string(),
                name.local_span(),
                item.span,
                SymbolKind::Imported(ImportedSymbol {
                    path: item.path.value.clone(),
                }),
            );
        }
    }

    fn collect_loaded_imported_symbols(
        &mut self,
        item: &FromImportItem,
        imported_ast: &AstFile,
        access: ImportAccess,
    ) {
        for name in &item.names {
            match self.find_importable_symbol(imported_ast, &name.name) {
                Some(imported) if imported.is_visible_to(access) => {
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, &item.path.value, &name.name);
                    self.define_symbol(
                        name.local_name().to_string(),
                        name.local_span(),
                        imported.declaration_span,
                        imported.kind,
                    );
                }
                Some(imported) => {
                    self.output.diagnostics.push(restricted_import_diagnostic(
                        self.sources,
                        &name.name,
                        &item.path.value,
                        imported.visibility,
                        name.name_span,
                        imported.declaration_span,
                    ));
                }
                None => {
                    self.output.diagnostics.push(missing_import_diagnostic(
                        self.sources,
                        &name.name,
                        &item.path.value,
                        name.name_span,
                    ));
                }
            }
        }
    }

    fn collect_public_exports(&mut self, ast: &AstFile, access: ImportAccess, module_path: &str) {
        for item in &ast.items {
            match item {
                Item::Function(function) if function.owner.is_none() => {
                    let imported = ImportableSymbol {
                        declaration_span: function.name_span,
                        visibility: function.visibility,
                        kind: SymbolKind::Function(function_signature(function)),
                    };
                    self.collect_public_export(
                        function.name.clone(),
                        function.name_span,
                        imported,
                        access,
                    );
                }
                Item::Primitive(primitive) => {
                    let imported = ImportableSymbol {
                        declaration_span: primitive.name_span,
                        visibility: primitive.visibility,
                        kind: SymbolKind::Primitive(primitive_signature(primitive)),
                    };
                    self.collect_public_export(
                        primitive.name.clone(),
                        primitive.name_span,
                        imported,
                        access,
                    );
                }
                Item::TypeAlias(alias) => {
                    let symbol = type_alias_symbol_with_impl_members(ast, alias);
                    let imported = type_importable_symbol(alias.span, alias.visibility, symbol);
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, module_path, &alias.name);
                    self.collect_public_export(
                        alias.name.clone(),
                        alias.name_span,
                        imported,
                        access,
                    );
                }
                Item::Struct(struct_) => {
                    let mut symbol =
                        struct_type_symbol(struct_.name.clone(), struct_.is_copy, &struct_.fields);
                    attach_inherent_impl_members_to_symbol(&mut symbol, ast, &struct_.name);
                    let imported = type_importable_symbol(struct_.span, struct_.visibility, symbol);
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, module_path, &struct_.name);
                    self.collect_public_export(
                        struct_.name.clone(),
                        struct_.name_span,
                        imported,
                        access,
                    );
                }
                Item::Enum(enum_) => {
                    let mut symbol = enum_type_symbol(enum_.name.clone(), &enum_.variants);
                    attach_inherent_impl_members_to_symbol(&mut symbol, ast, &enum_.name);
                    let imported = type_importable_symbol(enum_.span, enum_.visibility, symbol);
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, module_path, &enum_.name);
                    self.collect_public_export(
                        enum_.name.clone(),
                        enum_.name_span,
                        imported,
                        access,
                    );
                }
                Item::Trait(trait_) => {
                    let imported = type_importable_symbol(
                        trait_.span,
                        trait_.visibility,
                        nominal_type_symbol(trait_.name.clone(), TypeSymbolKind::Trait),
                    );
                    let imported = qualify_imported_symbol(imported, module_path, &trait_.name);
                    self.collect_public_export(
                        trait_.name.clone(),
                        trait_.name_span,
                        imported,
                        access,
                    );
                }
                Item::FromImport(item) if item.visibility == Visibility::Public => {
                    self.collect_public_reexports(item, access);
                }
                Item::Function(_)
                | Item::Use(_)
                | Item::Import(_)
                | Item::FromImport(_)
                | Item::Impl(_) => {}
            }
        }
    }

    fn collect_public_export(
        &mut self,
        name: String,
        name_span: ByteSpan,
        imported: ImportableSymbol,
        access: ImportAccess,
    ) {
        if imported.is_visible_to(access) {
            self.define_symbol(name, name_span, imported.declaration_span, imported.kind);
        }
    }

    fn collect_public_reexports(&mut self, item: &FromImportItem, access: ImportAccess) {
        let Some((imported_ast, _)) = self.module_index.import_ast(item, self.import_sources)
        else {
            return;
        };

        for name in &item.names {
            match self.find_importable_symbol(imported_ast, &name.name) {
                Some(imported)
                    if imported.visibility == Visibility::Public
                        && imported.is_visible_to(access) =>
                {
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, &item.path.value, &name.name);
                    self.define_symbol(
                        name.local_name().to_string(),
                        name.local_span(),
                        imported.declaration_span,
                        imported.kind,
                    );
                }
                Some(imported) => {
                    self.output.diagnostics.push(restricted_import_diagnostic(
                        self.sources,
                        &name.name,
                        &item.path.value,
                        imported.visibility,
                        name.name_span,
                        imported.declaration_span,
                    ));
                }
                None => {
                    self.output.diagnostics.push(missing_import_diagnostic(
                        self.sources,
                        &name.name,
                        &item.path.value,
                        name.name_span,
                    ));
                }
            }
        }
    }

    fn report_unloaded_imported_symbols(&mut self, item: &FromImportItem) {
        for name in &item.names {
            self.output.diagnostics.push(unloaded_import_diagnostic(
                self.sources,
                &item.path.value,
                name.local_span(),
            ));
        }
    }

    fn find_importable_symbol(&self, ast: &AstFile, name: &str) -> Option<ImportableSymbol> {
        direct_importable_symbol(ast, name).or_else(|| self.find_reexported_symbol(ast, name))
    }

    fn find_reexported_symbol(&self, ast: &AstFile, name: &str) -> Option<ImportableSymbol> {
        ast.items.iter().find_map(|item| {
            let Item::FromImport(item) = item else {
                return None;
            };
            if item.visibility != Visibility::Public {
                return None;
            }

            let reexport = item
                .names
                .iter()
                .find(|imported| imported.local_name() == name)?;
            let (imported_ast, _) = self.module_index.import_ast(item, self.import_sources)?;
            let imported = direct_importable_symbol(imported_ast, &reexport.name)?;
            (imported.visibility == Visibility::Public)
                .then(|| qualify_imported_symbol(imported, &item.path.value, &reexport.name))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportableSymbol {
    declaration_span: ByteSpan,
    visibility: Visibility,
    kind: SymbolKind,
}

impl ImportableSymbol {
    fn is_visible_to(&self, access: ImportAccess) -> bool {
        match self.visibility {
            Visibility::Public => true,
            Visibility::Nocter => access == ImportAccess::Nocter,
            Visibility::Private => false,
        }
    }
}

fn direct_importable_symbol(ast: &AstFile, name: &str) -> Option<ImportableSymbol> {
    ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.owner.is_none() && function.name == name => {
            Some(ImportableSymbol {
                declaration_span: function.name_span,
                visibility: function.visibility,
                kind: SymbolKind::Function(function_signature(function)),
            })
        }
        Item::Primitive(primitive) if primitive.name == name => Some(ImportableSymbol {
            declaration_span: primitive.name_span,
            visibility: primitive.visibility,
            kind: SymbolKind::Primitive(primitive_signature(primitive)),
        }),
        Item::TypeAlias(alias) if alias.name == name => {
            let symbol = type_alias_symbol_with_impl_members(ast, alias);
            Some(type_importable_symbol(alias.span, alias.visibility, symbol))
        }
        Item::Struct(struct_) if struct_.name == name => {
            let mut symbol =
                struct_type_symbol(struct_.name.clone(), struct_.is_copy, &struct_.fields);
            attach_inherent_impl_members_to_symbol(&mut symbol, ast, &struct_.name);
            Some(type_importable_symbol(
                struct_.span,
                struct_.visibility,
                symbol,
            ))
        }
        Item::Enum(enum_) if enum_.name == name => {
            let mut symbol = enum_type_symbol(enum_.name.clone(), &enum_.variants);
            attach_inherent_impl_members_to_symbol(&mut symbol, ast, &enum_.name);
            Some(type_importable_symbol(enum_.span, enum_.visibility, symbol))
        }
        Item::Trait(trait_) if trait_.name == name => Some(type_importable_symbol(
            trait_.span,
            trait_.visibility,
            nominal_type_symbol(trait_.name.clone(), TypeSymbolKind::Trait),
        )),
        _ => None,
    })
}

fn type_alias_symbol_with_impl_members(ast: &AstFile, alias: &TypeAliasDecl) -> TypeSymbol {
    let mut symbol = alias_type_symbol(alias.name.clone(), alias.target.clone());
    attach_inherent_impl_members_to_symbol(&mut symbol, ast, &alias.name);
    symbol
}

fn type_importable_symbol(
    declaration_span: ByteSpan,
    visibility: Visibility,
    symbol: TypeSymbol,
) -> ImportableSymbol {
    ImportableSymbol {
        declaration_span,
        visibility,
        kind: SymbolKind::Type(symbol),
    }
}

fn filter_importable_symbol_for_access(
    mut imported: ImportableSymbol,
    access: ImportAccess,
) -> ImportableSymbol {
    if let SymbolKind::Type(symbol) = &mut imported.kind {
        for field in &mut symbol.fields {
            field.is_accessible =
                field.is_accessible && visibility_is_visible_to(field.visibility, access);
        }

        for function in &mut symbol.associated_functions {
            function.is_accessible =
                function.is_accessible && visibility_is_visible_to(function.visibility, access);
        }

        for method in &mut symbol.methods {
            method.is_accessible =
                method.is_accessible && visibility_is_visible_to(method.visibility, access);
        }
    }

    imported
}

fn visibility_is_visible_to(visibility: Visibility, access: ImportAccess) -> bool {
    match visibility {
        Visibility::Public => true,
        Visibility::Nocter => access == ImportAccess::Nocter,
        Visibility::Private => false,
    }
}

fn qualify_imported_symbol(
    mut imported: ImportableSymbol,
    import_path: &str,
    imported_name: &str,
) -> ImportableSymbol {
    if let SymbolKind::Type(symbol) = &mut imported.kind
        && symbol.canonical_name == imported_name
    {
        symbol.canonical_name = format!("{import_path}.{imported_name}");
    }

    imported
}
