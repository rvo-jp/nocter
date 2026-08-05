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
            if item.visibility != Visibility::Public {
                return None;
            }

            let reexport = item
                .names
                .iter()
                .find(|imported| imported.local_name() == name)?;
            let (imported_ast, _) = self.module_index.import_ast(item, self.import_sources)?;
            let mut branch_visited = visited.clone();
            let imported = self.find_importable_symbol_with_visited(
                imported_ast,
                &reexport.name,
                &mut branch_visited,
            )?;
            (imported.visibility == Visibility::Public)
                .then(|| qualify_imported_symbol(imported, &item.path.value, &reexport.name))
        })
    }

    pub(super) fn direct_importable_symbol(
        &self,
        ast: &AstFile,
        name: &str,
    ) -> Option<ImportableSymbol> {
        ast.items.iter().find_map(|item| match item {
            Item::Import(import)
                if import.visibility == Visibility::Public && import.alias.name == name =>
            {
                let source = self
                    .module_index
                    .import_ast_for_span(import.path.span, self.import_sources)
                    .map(|(_, source)| source);
                Some(ImportableSymbol {
                    declaration_span: import.alias.span,
                    declaration_name_span: import.alias.span,
                    visibility: Visibility::Public,
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
                    kind: SymbolKind::Function(function_signature(function)),
                    local_type_names: type_decl_names(ast),
                    imported_type_names: self.imported_type_names(ast),
                })
            }
            Item::Primitive(primitive) if primitive.name == name => Some(ImportableSymbol {
                declaration_span: primitive.name_span,
                declaration_name_span: primitive.name_span,
                visibility: primitive.visibility,
                kind: SymbolKind::Primitive(primitive_signature(primitive)),
                local_type_names: type_decl_names(ast),
                imported_type_names: self.imported_type_names(ast),
            }),
            Item::TypeAlias(alias) if alias.name == name => {
                let symbol = type_alias_symbol_with_impl_members(ast, alias);
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
                attach_inherent_impl_members_to_symbol(&mut symbol, ast, &struct_.name);
                attach_literal_definitions_to_symbol(&mut symbol, ast, &struct_.name);
                super::super::constructions::attach_construction_surfaces_to_symbol(
                    &mut symbol,
                    ast,
                    &struct_.name,
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
                attach_inherent_impl_members_to_symbol(&mut symbol, ast, &enum_.name);
                attach_literal_definitions_to_symbol(&mut symbol, ast, &enum_.name);
                super::super::constructions::attach_construction_surfaces_to_symbol(
                    &mut symbol,
                    ast,
                    &enum_.name,
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
        imported_type_names
    }
}
