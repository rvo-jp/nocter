use super::*;

pub(super) fn type_alias_symbol_with_impl_members(
    ast: &AstFile,
    alias: &TypeAliasDecl,
) -> TypeSymbol {
    let mut symbol = alias_type_symbol(alias);
    attach_inherent_impl_members_to_symbol(&mut symbol, ast, &alias.name);
    symbol
}

pub(super) fn type_importable_symbol(
    declaration_span: ByteSpan,
    declaration_name_span: ByteSpan,
    visibility: Visibility,
    symbol: TypeSymbol,
    local_type_names: Vec<String>,
    imported_type_names: Vec<ImportedTypeName>,
) -> ImportableSymbol {
    ImportableSymbol {
        declaration_span,
        declaration_name_span,
        visibility,
        visibility_source: declaration_span.source,
        kind: SymbolKind::Type(symbol),
        local_type_names,
        imported_type_names,
    }
}

pub(super) fn type_decl_names(ast: &AstFile) -> Vec<String> {
    ast.items
        .iter()
        .filter_map(|item| match item {
            Item::TypeAlias(alias) => Some(alias.name.clone()),
            Item::Struct(struct_) => Some(struct_.name.clone()),
            Item::Enum(enum_) => Some(enum_.name.clone()),
            Item::Interface(interface) => Some(interface.name.clone()),
            Item::Import(_)
            | Item::FromImport(_)
            | Item::Function(_)
            | Item::Test(_)
            | Item::Primitive(_)
            | Item::Impl(_) => None,
            Item::Construct(_) | Item::Coerce(_) => None,
        })
        .collect()
}
