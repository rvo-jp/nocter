use super::*;

impl Resolver<'_> {
    pub(in crate::resolve) fn collect_synthetic_prelude_symbols(&mut self, ast: &AstFile) {
        let Some(prelude_source) = self.prelude_sources.get(&ast.span.source) else {
            return;
        };
        let Some(prelude_ast) = self.module_index.ast_for_source(prelude_source.source) else {
            return;
        };

        let was_collecting = self.collecting_synthetic_prelude;
        self.collecting_synthetic_prelude = true;
        self.collect_public_exports(prelude_ast, prelude_source.access, "std/prelude");
        self.collecting_synthetic_prelude = was_collecting;
    }

    pub(in crate::resolve) fn collect_import_namespace_symbol(&mut self, item: &ImportItem) {
        let import_source = self
            .module_index
            .import_ast_for_span(item.path.span, self.import_sources)
            .map(|(_, source)| source);

        if import_source.is_none() && is_relative_module_path(&item.path.value) {
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
                source: import_source.map(|source| source.source),
                access: import_source.map(|source| source.access),
                kind: ImportedSymbolKind::Namespace,
            }),
        );
    }

    pub(in crate::resolve) fn resolve_namespace_member_symbol(
        &mut self,
        namespace: &ImportedSymbol,
        member_name: &str,
        member_span: ByteSpan,
    ) -> Option<SymbolId> {
        if namespace.kind != ImportedSymbolKind::Namespace {
            return None;
        }

        let source = namespace.source?;
        let access = namespace.access.unwrap_or(ImportAccess::Public);
        let imported_ast = self.module_index.ast_for_source(source)?;

        match self.find_importable_symbol(imported_ast, member_name) {
            Some(imported) if imported.is_visible_to(access) => {
                let imported = filter_importable_symbol_for_access(imported, access);
                let dependency_type_names = imported.local_type_names.clone();
                let dependency_imported_type_names = imported.imported_type_names.clone();
                let imported = qualify_imported_symbol(imported, &namespace.path, member_name);
                let id = self.output.symbols.define_hidden(
                    member_name.to_string(),
                    member_span,
                    imported.declaration_span,
                    imported.kind,
                );
                self.collect_hidden_imported_type_symbols(
                    imported_ast,
                    &namespace.path,
                    access,
                    &dependency_type_names,
                );
                self.collect_hidden_imported_type_dependencies(&dependency_imported_type_names);
                Some(id)
            }
            Some(imported) => {
                self.output.diagnostics.push(restricted_import_diagnostic(
                    self.sources,
                    member_name,
                    &namespace.path,
                    imported.visibility,
                    member_span,
                    imported.declaration_span,
                ));
                None
            }
            None => {
                self.output.diagnostics.push(missing_import_diagnostic(
                    self.sources,
                    member_name,
                    &namespace.path,
                    member_span,
                ));
                None
            }
        }
    }

    pub(in crate::resolve) fn collect_imported_symbols(&mut self, item: &FromImportItem) {
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
                    source: None,
                    access: None,
                    kind: ImportedSymbolKind::UnloadedName,
                }),
            );
        }
    }

    pub(in crate::resolve) fn collect_scoped_import_namespace_symbol(
        &mut self,
        item: &ImportItem,
        scope: &mut Scope,
    ) {
        let import_source = self
            .module_index
            .import_ast_for_span(item.path.span, self.import_sources)
            .map(|(_, source)| source);

        if import_source.is_none() && is_relative_module_path(&item.path.value) {
            self.output.diagnostics.push(unloaded_import_diagnostic(
                self.sources,
                &item.path.value,
                item.alias.span,
            ));
            return;
        }

        self.define_scoped_symbol(
            item.alias.name.clone(),
            item.alias.span,
            item.path.span,
            SymbolKind::Imported(ImportedSymbol {
                path: item.path.value.clone(),
                source: import_source.map(|source| source.source),
                access: import_source.map(|source| source.access),
                kind: ImportedSymbolKind::Namespace,
            }),
            scope,
        );
    }

    pub(in crate::resolve) fn collect_scoped_imported_symbols(
        &mut self,
        item: &FromImportItem,
        scope: &mut Scope,
    ) {
        if let Some((imported_ast, import_source)) =
            self.module_index.import_ast(item, self.import_sources)
        {
            self.collect_loaded_scoped_imported_symbols(
                item,
                imported_ast,
                import_source.access,
                scope,
            );
            return;
        }

        if is_relative_module_path(&item.path.value) {
            self.report_unloaded_imported_symbols(item);
            return;
        }

        for name in &item.names {
            self.define_scoped_symbol(
                name.local_name().to_string(),
                name.local_span(),
                item.span,
                SymbolKind::Imported(ImportedSymbol {
                    path: item.path.value.clone(),
                    source: None,
                    access: None,
                    kind: ImportedSymbolKind::UnloadedName,
                }),
                scope,
            );
        }
    }

    fn collect_loaded_scoped_imported_symbols(
        &mut self,
        item: &FromImportItem,
        imported_ast: &AstFile,
        access: ImportAccess,
        scope: &mut Scope,
    ) {
        for name in &item.names {
            match self.find_importable_symbol(imported_ast, &name.name) {
                Some(imported) if imported.is_visible_to(access) => {
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let dependency_imported_type_names = imported.imported_type_names.clone();
                    let imported = qualify_imported_symbol(imported, &item.path.value, &name.name);
                    let dependency_type_names = imported.local_type_names.clone();
                    self.define_scoped_symbol(
                        name.local_name().to_string(),
                        name.local_span(),
                        imported.declaration_span,
                        imported.kind,
                        scope,
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

    fn define_scoped_symbol(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        kind: SymbolKind,
        scope: &mut Scope,
    ) {
        if is_builtin_type_name(&name) {
            self.output.diagnostics.push(builtin_name_reuse_diagnostic(
                self.sources,
                &name,
                name_span,
            ));
            return;
        }

        if let Some(first_span) = scope.get(&name) {
            let diagnostic = self.duplicate_visible_symbol_diagnostic(&name, first_span, name_span);
            self.output.diagnostics.push(diagnostic);
            return;
        }

        if let Some(symbol) = self.output.symbols.symbol_by_name(&name) {
            let diagnostic =
                self.duplicate_visible_symbol_diagnostic(&name, symbol.name_span, name_span);
            self.output.diagnostics.push(diagnostic);
            return;
        }

        let id = self
            .output
            .symbols
            .define_hidden(name.clone(), name_span, declaration_span, kind);
        scope.define_symbol(name, name_span, id);
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
                        declaration_name_span: function.name_span,
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
                        declaration_name_span: primitive.name_span,
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
                        alias.name_span,
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
                        struct_.name_span,
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
                        enum_.name_span,
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
                        interface.name_span,
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
                | Item::Import(_)
                | Item::FromImport(_)
                | Item::Impl(_)
                | Item::Literal(_) => {}
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
            self.output.symbols.ensure_hidden_resolvable(
                canonical_name,
                imported.declaration_name_span,
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
            if !self
                .collected_hidden_type_dependencies
                .insert((imported_ast.span.source, type_name.imported_name.clone()))
            {
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
            self.output.symbols.ensure_hidden_resolvable(
                canonical_name,
                imported.declaration_name_span,
                imported.declaration_span,
                imported.kind,
            );
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
}
