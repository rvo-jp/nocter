//! Import resolution, visibility, and name lookup.

mod body;
mod builtins;
mod collector;
mod conformance;
mod diagnostics;
mod imports;
mod literals;
mod module_index;
mod regions;
mod signatures;
mod symbols;

#[cfg(test)]
mod tests;

pub use symbols::{
    AssociatedFunctionSignature, DropSignature, EnumVariantSignature, FunctionSignature,
    ImportAccess, ImportSource, ImportSourceMap, ImportedSymbol, ImportedSymbolKind,
    InterfaceConformance, LiteralCaptureSignature, LiteralResolution, LiteralSignature,
    LocalSymbol, LocalSymbolId, LocalSymbolKind, MethodSignature, ParameterSignature,
    PreludeSourceMap, ResolveOutput, StructFieldSignature, Symbol, SymbolId, SymbolKind,
    SymbolTable, TypeSymbol, TypeSymbolKind,
};

use module_index::ModuleIndex;

use crate::ast::{AstFile, Item};
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashSet;

pub fn resolve(sources: &SourceMap, ast: &AstFile) -> ResolveOutput {
    resolve_compile_unit(
        sources,
        ast,
        std::slice::from_ref(ast),
        &ImportSourceMap::new(),
        &PreludeSourceMap::new(),
    )
}

pub fn resolve_compile_unit(
    sources: &SourceMap,
    root: &AstFile,
    files: &[AstFile],
    import_sources: &ImportSourceMap,
    prelude_sources: &PreludeSourceMap,
) -> ResolveOutput {
    let module_index = ModuleIndex::new(files);
    let access = root_access(root, import_sources, prelude_sources);
    let mut resolver = Resolver {
        sources,
        module_index,
        import_sources,
        prelude_sources,
        output: ResolveOutput::new(access),
        synthetic_prelude_symbol_spans: HashSet::new(),
        collecting_synthetic_prelude: false,
    };

    resolver.collect_top_level_symbols(root);
    resolver.resolve_callable_bodies(root);
    resolver.output
}

fn root_access(
    root: &AstFile,
    import_sources: &ImportSourceMap,
    prelude_sources: &PreludeSourceMap,
) -> ImportAccess {
    for item in &root.items {
        let path_span = match item {
            Item::Import(import) => import.path.span,
            Item::FromImport(import) => import.path.span,
            Item::Function(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_)
            | Item::Interface(_)
            | Item::Impl(_)
            | Item::Literal(_) => continue,
        };
        if let Some(import_source) = import_sources.get(&path_span) {
            return import_source.access;
        }
    }

    prelude_sources
        .get(&root.span.source)
        .map(|source| source.access)
        .unwrap_or(ImportAccess::Public)
}

struct Resolver<'a> {
    sources: &'a SourceMap,
    module_index: ModuleIndex<'a>,
    import_sources: &'a ImportSourceMap,
    prelude_sources: &'a PreludeSourceMap,
    output: ResolveOutput,
    synthetic_prelude_symbol_spans: HashSet<ByteSpan>,
    collecting_synthetic_prelude: bool,
}
