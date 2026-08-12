use super::*;

impl Resolver<'_> {
    pub(super) fn find_importable_symbol(
        &self,
        ast: &AstFile,
        name: &str,
    ) -> Option<ImportableSymbol> {
        let mut visited = HashSet::new();
        self.find_importable_symbol_with_visited(ast, name, &mut visited)
    }

    fn find_importable_symbol_with_visited(
        &self,
        ast: &AstFile,
        name: &str,
        visited: &mut HashSet<ReexportLookup>,
    ) -> Option<ImportableSymbol> {
        self.direct_importable_symbol(ast, name)
            .or_else(|| self.find_reexported_symbol(ast, name, visited))
    }

    fn find_reexported_symbol(
        &self,
        ast: &AstFile,
        name: &str,
        visited: &mut HashSet<ReexportLookup>,
    ) -> Option<ImportableSymbol> {
        if !visited.insert(ReexportLookup {
            source: ast.span.source,
            name: name.to_string(),
        }) {
            return None;
        }

        ast.items.iter().find_map(|item| {
            let Item::FromImport(item) = item else {
                return None;
            };
            if self
                .import_sources
                .get(&item.path.span)
                .is_some_and(|source| source.kind == crate::resolve::ImportKind::Source)
            {
                return None;
            }
            if item.visibility == Visibility::Private {
                return None;
            }

            let reexport = item
                .names
                .iter()
                .find(|imported| imported.local_name() == name)?;
            let (imported_ast, import_source) =
                self.module_index.import_ast(item, self.import_sources)?;
            let mut branch_visited = visited.clone();
            let mut imported = self.find_importable_symbol_with_visited(
                imported_ast,
                &reexport.name,
                &mut branch_visited,
            )?;
            if !imported.is_visible_to(import_source.access)
                || !self.output.reexport_does_not_widen(
                    imported.visibility,
                    imported.visibility_source,
                    item.visibility,
                    ast.span.source,
                )
            {
                return None;
            }
            imported.visibility = item.visibility;
            imported.visibility_source = ast.span.source;
            Some(qualify_imported_symbol(
                imported,
                &item.path.value,
                &reexport.name,
            ))
        })
    }

    pub(super) fn direct_importable_symbol(
        &self,
        ast: &AstFile,
        name: &str,
    ) -> Option<ImportableSymbol> {
        ast.items.iter().find_map(|item| match item {
            Item::Import(import)
                if import.visibility != Visibility::Private && import.alias.name == name =>
            {
                let source = self
                    .module_index
                    .import_ast_for_span(import.path.span, self.import_sources)
                    .map(|(_, source)| source);
                Some(ImportableSymbol {
                    declaration_span: import.alias.span,
                    declaration_name_span: import.alias.span,
                    visibility: import.visibility,
                    visibility_source: ast.span.source,
                    kind: SymbolKind::Imported(ImportedSymbol {
                        path: import.path.value.clone(),
                        source: source.map(|source| source.source),
                        access: source.map(|source| source.access),
                        kind: ImportedSymbolKind::Namespace,
                    }),
                    local_type_names: Vec::new(),
                    imported_type_names: Vec::new(),
                })
            }
            Item::Function(function) if function.owner.is_none() && function.name == name => {
                Some(ImportableSymbol {
                    declaration_span: function.name_span,
                    declaration_name_span: function.name_span,
                    visibility: function.visibility,
                    visibility_source: ast.span.source,
                    kind: SymbolKind::Function(function_signature(function)),
                    local_type_names: type_decl_names(ast),
                    imported_type_names: self.imported_type_names(ast),
                })
            }
            Item::Primitive(primitive) if primitive.name == name => Some(ImportableSymbol {
                declaration_span: primitive.name_span,
                declaration_name_span: primitive.name_span,
                visibility: primitive.visibility,
                visibility_source: ast.span.source,
                kind: SymbolKind::Primitive(primitive_signature(primitive)),
                local_type_names: type_decl_names(ast),
                imported_type_names: self.imported_type_names(ast),
            }),
            Item::TypeAlias(alias) if alias.name == name => {
                let symbol = type_alias_symbol_with_surfaces(ast, alias);
                Some(type_importable_symbol(
                    alias.span,
                    alias.name_span,
                    alias.visibility,
                    symbol,
                    type_decl_names(ast),
                    self.imported_type_names(ast),
                ))
            }
            Item::Struct(struct_) if struct_.name == name => {
                let mut symbol = struct_type_symbol(struct_, struct_.is_copy, &struct_.fields);
                super::super::type_surfaces::attach_nominal_type_surfaces(
                    &mut symbol,
                    ast,
                    &struct_.name,
                    &self.output.semantic_db,
                );
                Some(type_importable_symbol(
                    struct_.span,
                    struct_.name_span,
                    struct_.visibility,
                    symbol,
                    type_decl_names(ast),
                    self.imported_type_names(ast),
                ))
            }
            Item::Enum(enum_) if enum_.name == name => {
                let mut symbol = enum_type_symbol(enum_);
                super::super::type_surfaces::attach_nominal_type_surfaces(
                    &mut symbol,
                    ast,
                    &enum_.name,
                    &self.output.semantic_db,
                );
                Some(type_importable_symbol(
                    enum_.span,
                    enum_.name_span,
                    enum_.visibility,
                    symbol,
                    type_decl_names(ast),
                    self.imported_type_names(ast),
                ))
            }
            Item::Interface(interface) if interface.name == name => Some(type_importable_symbol(
                interface.span,
                interface.name_span,
                interface.visibility,
                interface_type_symbol(interface),
                type_decl_names(ast),
                self.imported_type_names(ast),
            )),
            _ => None,
        })
    }

    pub(super) fn imported_type_names(&self, ast: &AstFile) -> Vec<ImportedTypeName> {
        if let Some(imported) = self
            .imported_type_name_cache
            .borrow()
            .get(&ast.span.source)
            .cloned()
        {
            return imported;
        }
        let is_top_level_lookup = self.collecting_imported_type_names.borrow().is_empty();
        if !self
            .collecting_imported_type_names
            .borrow_mut()
            .insert(ast.span.source)
        {
            return Vec::new();
        }
        let mut imported_type_names = Vec::new();
        for item in &ast.items {
            let Item::FromImport(import) = item else {
                continue;
            };
            if self
                .import_sources
                .get(&import.path.span)
                .is_some_and(|source| source.kind == crate::resolve::ImportKind::Source)
            {
                continue;
            }
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
                let SymbolKind::Type(symbol) = &imported.kind else {
                    continue;
                };
                let canonical_name = if symbol.canonical_name.contains('.') {
                    symbol.canonical_name.clone()
                } else {
                    format!("{}.{}", import.path.value, name.name)
                };
                imported_type_names.push(ImportedTypeName {
                    local_name: name.local_name().to_string(),
                    import_path: import.path.value.clone(),
                    imported_name: name.name.clone(),
                    canonical_name,
                    path_span: import.path.span,
                });
            }
        }
        self.collecting_imported_type_names
            .borrow_mut()
            .remove(&ast.span.source);
        // A nested lookup may have crossed an import cycle and received an intentionally empty
        // recursion-guard result. Only a top-level lookup is a stable cache entry; nested sources
        // will be cached when a later surface asks for their environment directly.
        if is_top_level_lookup {
            self.imported_type_name_cache
                .borrow_mut()
                .insert(ast.span.source, imported_type_names.clone());
        }
        imported_type_names
    }
}
