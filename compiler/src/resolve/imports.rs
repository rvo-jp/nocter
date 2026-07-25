use super::diagnostics::{
    missing_import_diagnostic, restricted_import_diagnostic, unloaded_import_diagnostic,
};
use super::module_index::is_relative_module_path;
use super::signatures::{
    alias_type_symbol, attach_inherent_impl_members_to_symbol, enum_type_symbol,
    function_signature, interface_type_symbol, primitive_signature, struct_type_symbol,
};
use super::{
    FunctionSignature, ImportAccess, ImportedSymbol, ParameterSignature, Resolver, SymbolKind,
    TypeSymbol,
};
use crate::ast::{
    AstFile, FromImportItem, ImportItem, Item, TypeAliasDecl, TypeExpr, UseItem, Visibility,
};
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
                    let dependency_imported_type_names = imported.imported_type_names.clone();
                    let imported = qualify_imported_symbol(imported, &item.path.value, &name.name);
                    let dependency_type_names = imported.local_type_names.clone();
                    self.define_symbol(
                        name.local_name().to_string(),
                        name.local_span(),
                        imported.declaration_span,
                        imported.kind,
                    );
                    self.collect_hidden_imported_type_symbols(
                        imported_ast,
                        &item.path.value,
                        access,
                        &dependency_type_names,
                    );
                    self.collect_hidden_imported_type_dependencies(&dependency_imported_type_names);
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
                        local_type_names: type_decl_names(ast),
                        imported_type_names: self.imported_type_names(ast),
                    };
                    self.collect_public_export(
                        function.name.clone(),
                        function.name_span,
                        imported,
                        access,
                        ast,
                        module_path,
                    );
                }
                Item::Primitive(primitive) => {
                    let imported = ImportableSymbol {
                        declaration_span: primitive.name_span,
                        visibility: primitive.visibility,
                        kind: SymbolKind::Primitive(primitive_signature(primitive)),
                        local_type_names: type_decl_names(ast),
                        imported_type_names: self.imported_type_names(ast),
                    };
                    self.collect_public_export(
                        primitive.name.clone(),
                        primitive.name_span,
                        imported,
                        access,
                        ast,
                        module_path,
                    );
                }
                Item::TypeAlias(alias) => {
                    let symbol = type_alias_symbol_with_impl_members(ast, alias);
                    let imported = type_importable_symbol(
                        alias.span,
                        alias.visibility,
                        symbol,
                        type_decl_names(ast),
                        self.imported_type_names(ast),
                    );
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, module_path, &alias.name);
                    self.collect_public_export(
                        alias.name.clone(),
                        alias.name_span,
                        imported,
                        access,
                        ast,
                        module_path,
                    );
                }
                Item::Struct(struct_) => {
                    let mut symbol = struct_type_symbol(struct_, struct_.is_copy, &struct_.fields);
                    attach_inherent_impl_members_to_symbol(&mut symbol, ast, &struct_.name);
                    let imported = type_importable_symbol(
                        struct_.span,
                        struct_.visibility,
                        symbol,
                        type_decl_names(ast),
                        self.imported_type_names(ast),
                    );
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, module_path, &struct_.name);
                    self.collect_public_export(
                        struct_.name.clone(),
                        struct_.name_span,
                        imported,
                        access,
                        ast,
                        module_path,
                    );
                }
                Item::Enum(enum_) => {
                    let mut symbol = enum_type_symbol(enum_);
                    attach_inherent_impl_members_to_symbol(&mut symbol, ast, &enum_.name);
                    let imported = type_importable_symbol(
                        enum_.span,
                        enum_.visibility,
                        symbol,
                        type_decl_names(ast),
                        self.imported_type_names(ast),
                    );
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, module_path, &enum_.name);
                    self.collect_public_export(
                        enum_.name.clone(),
                        enum_.name_span,
                        imported,
                        access,
                        ast,
                        module_path,
                    );
                }
                Item::Interface(interface) => {
                    let imported = type_importable_symbol(
                        interface.span,
                        interface.visibility,
                        interface_type_symbol(interface),
                        type_decl_names(ast),
                        self.imported_type_names(ast),
                    );
                    let imported = qualify_imported_symbol(imported, module_path, &interface.name);
                    self.collect_public_export(
                        interface.name.clone(),
                        interface.name_span,
                        imported,
                        access,
                        ast,
                        module_path,
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
        imported_ast: &AstFile,
        module_path: &str,
    ) {
        if imported.is_visible_to(access) {
            let dependency_type_names = imported.local_type_names.clone();
            let dependency_imported_type_names = imported.imported_type_names.clone();
            self.define_symbol(name, name_span, imported.declaration_span, imported.kind);
            self.collect_hidden_imported_type_symbols(
                imported_ast,
                module_path,
                access,
                &dependency_type_names,
            );
            self.collect_hidden_imported_type_dependencies(&dependency_imported_type_names);
        }
    }

    fn collect_public_reexports(&mut self, item: &FromImportItem, access: ImportAccess) {
        let Some((imported_ast, import_source)) =
            self.module_index.import_ast(item, self.import_sources)
        else {
            return;
        };

        for name in &item.names {
            match self.find_importable_symbol(imported_ast, &name.name) {
                Some(imported)
                    if imported.visibility == Visibility::Public
                        && imported.is_visible_to(import_source.access)
                        && imported.is_visible_to(access) =>
                {
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let dependency_type_names = imported.local_type_names.clone();
                    let dependency_imported_type_names = imported.imported_type_names.clone();
                    let imported = qualify_imported_symbol(imported, &item.path.value, &name.name);
                    self.define_symbol(
                        name.local_name().to_string(),
                        name.local_span(),
                        imported.declaration_span,
                        imported.kind,
                    );
                    self.collect_hidden_imported_type_symbols(
                        imported_ast,
                        &item.path.value,
                        access,
                        &dependency_type_names,
                    );
                    self.collect_hidden_imported_type_dependencies(&dependency_imported_type_names);
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

    fn collect_hidden_imported_type_symbols(
        &mut self,
        imported_ast: &AstFile,
        import_path: &str,
        access: ImportAccess,
        type_names: &[String],
    ) {
        for type_name in type_names {
            let Some(imported) = self.direct_importable_symbol(imported_ast, type_name) else {
                continue;
            };
            if !imported.is_visible_to(access) {
                continue;
            }
            let imported = filter_importable_symbol_for_access(imported, access);
            let imported = qualify_imported_symbol(imported, import_path, type_name);
            let SymbolKind::Type(symbol) = &imported.kind else {
                continue;
            };
            let canonical_name = symbol.canonical_name.clone();
            if self
                .output
                .symbols
                .symbol_by_name(&canonical_name)
                .is_some()
            {
                continue;
            }
            self.define_symbol(
                canonical_name,
                imported.declaration_span,
                imported.declaration_span,
                imported.kind,
            );
        }
    }

    fn collect_hidden_imported_type_dependencies(&mut self, type_names: &[ImportedTypeName]) {
        for type_name in type_names {
            let Some((imported_ast, import_source)) = self
                .module_index
                .import_ast_for_span(type_name.path_span, self.import_sources)
            else {
                continue;
            };
            let Some(imported) =
                self.find_importable_symbol(imported_ast, &type_name.imported_name)
            else {
                continue;
            };
            if !imported.is_visible_to(import_source.access) {
                continue;
            }
            let imported = filter_importable_symbol_for_access(imported, import_source.access);
            let nested_local_type_names = imported.local_type_names.clone();
            let nested_imported_type_names = imported.imported_type_names.clone();
            let imported =
                qualify_imported_symbol(imported, &type_name.import_path, &type_name.imported_name);
            let SymbolKind::Type(symbol) = &imported.kind else {
                continue;
            };
            let canonical_name = symbol.canonical_name.clone();
            if self
                .output
                .symbols
                .symbol_by_name(&canonical_name)
                .is_none()
            {
                self.define_symbol(
                    canonical_name,
                    imported.declaration_span,
                    imported.declaration_span,
                    imported.kind,
                );
            }
            self.collect_hidden_imported_type_symbols(
                imported_ast,
                &type_name.import_path,
                import_source.access,
                &nested_local_type_names,
            );
            self.collect_hidden_imported_type_dependencies(&nested_imported_type_names);
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
        self.direct_importable_symbol(ast, name)
            .or_else(|| self.find_reexported_symbol(ast, name))
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
            let imported = self.direct_importable_symbol(imported_ast, &reexport.name)?;
            (imported.visibility == Visibility::Public)
                .then(|| qualify_imported_symbol(imported, &item.path.value, &reexport.name))
        })
    }

    fn direct_importable_symbol(&self, ast: &AstFile, name: &str) -> Option<ImportableSymbol> {
        ast.items.iter().find_map(|item| match item {
            Item::Function(function) if function.owner.is_none() && function.name == name => {
                Some(ImportableSymbol {
                    declaration_span: function.name_span,
                    visibility: function.visibility,
                    kind: SymbolKind::Function(function_signature(function)),
                    local_type_names: type_decl_names(ast),
                    imported_type_names: self.imported_type_names(ast),
                })
            }
            Item::Primitive(primitive) if primitive.name == name => Some(ImportableSymbol {
                declaration_span: primitive.name_span,
                visibility: primitive.visibility,
                kind: SymbolKind::Primitive(primitive_signature(primitive)),
                local_type_names: type_decl_names(ast),
                imported_type_names: self.imported_type_names(ast),
            }),
            Item::TypeAlias(alias) if alias.name == name => {
                let symbol = type_alias_symbol_with_impl_members(ast, alias);
                Some(type_importable_symbol(
                    alias.span,
                    alias.visibility,
                    symbol,
                    type_decl_names(ast),
                    self.imported_type_names(ast),
                ))
            }
            Item::Struct(struct_) if struct_.name == name => {
                let mut symbol = struct_type_symbol(struct_, struct_.is_copy, &struct_.fields);
                attach_inherent_impl_members_to_symbol(&mut symbol, ast, &struct_.name);
                Some(type_importable_symbol(
                    struct_.span,
                    struct_.visibility,
                    symbol,
                    type_decl_names(ast),
                    self.imported_type_names(ast),
                ))
            }
            Item::Enum(enum_) if enum_.name == name => {
                let mut symbol = enum_type_symbol(enum_);
                attach_inherent_impl_members_to_symbol(&mut symbol, ast, &enum_.name);
                Some(type_importable_symbol(
                    enum_.span,
                    enum_.visibility,
                    symbol,
                    type_decl_names(ast),
                    self.imported_type_names(ast),
                ))
            }
            Item::Interface(interface) if interface.name == name => Some(type_importable_symbol(
                interface.span,
                interface.visibility,
                interface_type_symbol(interface),
                type_decl_names(ast),
                self.imported_type_names(ast),
            )),
            _ => None,
        })
    }

    fn imported_type_names(&self, ast: &AstFile) -> Vec<ImportedTypeName> {
        let mut imported_type_names = Vec::new();
        for item in &ast.items {
            let Item::FromImport(import) = item else {
                continue;
            };
            let Some((imported_ast, import_source)) =
                self.module_index.import_ast(import, self.import_sources)
            else {
                continue;
            };
            for name in &import.names {
                let Some(imported) = self.find_importable_symbol(imported_ast, &name.name) else {
                    continue;
                };
                if !imported.is_visible_to(import_source.access) {
                    continue;
                }
                if !matches!(imported.kind, SymbolKind::Type(_)) {
                    continue;
                }
                imported_type_names.push(ImportedTypeName {
                    local_name: name.local_name().to_string(),
                    import_path: import.path.value.clone(),
                    imported_name: name.name.clone(),
                    path_span: import.path.span,
                });
            }
        }
        imported_type_names
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportableSymbol {
    declaration_span: ByteSpan,
    visibility: Visibility,
    kind: SymbolKind,
    local_type_names: Vec<String>,
    imported_type_names: Vec<ImportedTypeName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportedTypeName {
    local_name: String,
    import_path: String,
    imported_name: String,
    path_span: ByteSpan,
}

impl ImportedTypeName {
    fn qualified_name(&self) -> String {
        format!("{}.{}", self.import_path, self.imported_name)
    }
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

fn type_alias_symbol_with_impl_members(ast: &AstFile, alias: &TypeAliasDecl) -> TypeSymbol {
    let mut symbol = alias_type_symbol(alias);
    attach_inherent_impl_members_to_symbol(&mut symbol, ast, &alias.name);
    symbol
}

fn type_importable_symbol(
    declaration_span: ByteSpan,
    visibility: Visibility,
    symbol: TypeSymbol,
    local_type_names: Vec<String>,
    imported_type_names: Vec<ImportedTypeName>,
) -> ImportableSymbol {
    ImportableSymbol {
        declaration_span,
        visibility,
        kind: SymbolKind::Type(symbol),
        local_type_names,
        imported_type_names,
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
    match &mut imported.kind {
        SymbolKind::Function(signature) | SymbolKind::Primitive(signature) => {
            qualify_function_signature(
                signature,
                import_path,
                &imported.local_type_names,
                &imported.imported_type_names,
            );
        }
        SymbolKind::Type(symbol) => {
            if symbol.canonical_name == imported_name {
                symbol.canonical_name = format!("{import_path}.{imported_name}");
            }
            qualify_type_symbol(
                symbol,
                import_path,
                &imported.local_type_names,
                &imported.imported_type_names,
            );
        }
        SymbolKind::Imported(_) => {}
    }

    imported
}

fn qualify_type_symbol(
    symbol: &mut TypeSymbol,
    import_path: &str,
    local_type_names: &[String],
    imported_type_names: &[ImportedTypeName],
) {
    if let Some(target) = &mut symbol.alias_target {
        qualify_type_expr(target, import_path, local_type_names, imported_type_names);
    }
    for field in &mut symbol.fields {
        qualify_type_expr(
            &mut field.ty,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
    for variant in &mut symbol.variants {
        for parameter in &mut variant.payload {
            qualify_parameter_signature(
                parameter,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
    }
    for function in &mut symbol.associated_functions {
        qualify_function_signature(
            &mut function.signature,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
    for method in &mut symbol.methods {
        if let Some(impl_target_ty) = &mut method.impl_target_ty {
            qualify_type_expr(
                impl_target_ty,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        qualify_parameter_signature(
            &mut method.receiver,
            import_path,
            local_type_names,
            imported_type_names,
        );
        qualify_function_signature(
            &mut method.signature,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
    if let Some(drop_member) = &mut symbol.drop_member {
        qualify_parameter_signature(
            &mut drop_member.binding,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
}

fn qualify_function_signature(
    signature: &mut FunctionSignature,
    import_path: &str,
    local_type_names: &[String],
    imported_type_names: &[ImportedTypeName],
) {
    for parameter in &mut signature.parameters {
        qualify_parameter_signature(
            parameter,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
    qualify_type_expr(
        &mut signature.return_type,
        import_path,
        local_type_names,
        imported_type_names,
    );
}

fn qualify_parameter_signature(
    parameter: &mut ParameterSignature,
    import_path: &str,
    local_type_names: &[String],
    imported_type_names: &[ImportedTypeName],
) {
    qualify_type_expr(
        &mut parameter.ty,
        import_path,
        local_type_names,
        imported_type_names,
    );
}

fn qualify_type_expr(
    ty: &mut TypeExpr,
    import_path: &str,
    local_type_names: &[String],
    imported_type_names: &[ImportedTypeName],
) {
    match ty {
        TypeExpr::Reference(reference) => {
            if local_type_names.iter().any(|name| name == &reference.name) {
                reference.name = format!("{import_path}.{}", reference.name);
            } else if let Some(imported) = imported_type_names
                .iter()
                .find(|imported| imported.local_name == reference.name)
            {
                reference.name = imported.qualified_name();
            }
        }
        TypeExpr::Generic(generic) => {
            if local_type_names.iter().any(|name| name == &generic.name) {
                generic.name = format!("{import_path}.{}", generic.name);
            } else if let Some(imported) = imported_type_names
                .iter()
                .find(|imported| imported.local_name == generic.name)
            {
                generic.name = imported.qualified_name();
            }
            for argument in &mut generic.arguments {
                qualify_type_expr(argument, import_path, local_type_names, imported_type_names);
            }
        }
        TypeExpr::Pointer(pointer) => {
            qualify_type_expr(
                &mut pointer.inner,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        TypeExpr::Borrow(borrow) => {
            qualify_type_expr(
                &mut borrow.inner,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        TypeExpr::View(view) => {
            qualify_type_expr(
                &mut view.element,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        TypeExpr::Array(array) => {
            qualify_type_expr(
                &mut array.element,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        TypeExpr::Optional(optional) => {
            qualify_type_expr(
                &mut optional.inner,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        TypeExpr::Fallible(fallible) => {
            qualify_type_expr(
                &mut fallible.success,
                import_path,
                local_type_names,
                imported_type_names,
            );
            qualify_type_expr(
                &mut fallible.error,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
    }
}

fn type_decl_names(ast: &AstFile) -> Vec<String> {
    ast.items
        .iter()
        .filter_map(|item| match item {
            Item::TypeAlias(alias) => Some(alias.name.clone()),
            Item::Struct(struct_) => Some(struct_.name.clone()),
            Item::Enum(enum_) => Some(enum_.name.clone()),
            Item::Interface(interface) => Some(interface.name.clone()),
            Item::Use(_)
            | Item::Import(_)
            | Item::FromImport(_)
            | Item::Function(_)
            | Item::Primitive(_)
            | Item::Impl(_) => None,
        })
        .collect()
}
