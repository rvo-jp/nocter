use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ImportableSymbol {
    pub(super) declaration_span: ByteSpan,
    pub(super) declaration_name_span: ByteSpan,
    pub(super) visibility: Visibility,
    /// The module against which a relative visibility boundary is interpreted.
    ///
    /// This differs from `declaration_span.source` after a re-export: diagnostics
    /// and navigation still point at the original declaration, while access to
    /// the exported name is governed by the re-exporting module.
    pub(super) visibility_source: SourceId,
    pub(super) kind: SymbolKind,
    pub(super) local_type_names: Vec<String>,
    pub(super) imported_type_names: Vec<ImportedTypeName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::resolve) struct ImportedTypeName {
    pub(super) local_name: String,
    pub(super) import_path: String,
    pub(super) imported_name: String,
    pub(super) canonical_name: String,
    pub(super) path_span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ReexportLookup {
    pub(super) source: SourceId,
    pub(super) name: String,
}

impl ImportedTypeName {
    pub(super) fn qualified_name(&self) -> String {
        self.canonical_name.clone()
    }
}

impl ImportableSymbol {
    pub(super) fn is_visible_to(&self, access: ImportAccess) -> bool {
        access.allows(self.visibility)
    }
}
