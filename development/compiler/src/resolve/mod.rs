//! Import resolution, visibility, and name lookup.

mod body;
mod builtins;
mod collector;
mod diagnostics;
mod imports;
mod module_index;
mod signatures;
mod symbols;

#[cfg(test)]
mod tests;

pub use symbols::{
    AssociatedFunctionSignature, DropSignature, EnumVariantSignature, FunctionSignature,
    ImportAccess, ImportSource, ImportSourceMap, ImportedSymbol, ImportedSymbolKind, LocalSymbol,
    LocalSymbolId, LocalSymbolKind, MethodSignature, ParameterSignature, PreludeSourceMap,
    ResolveOutput, StructFieldSignature, Symbol, SymbolId, SymbolKind, SymbolTable, TypeSymbol,
    TypeSymbolKind,
};

use module_index::ModuleIndex;

use crate::ast::AstFile;
use crate::source::SourceMap;

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
    let mut resolver = Resolver {
        sources,
        module_index,
        import_sources,
        prelude_sources,
        output: ResolveOutput::new(),
    };

    resolver.collect_top_level_symbols(root);
    resolver.resolve_callable_bodies(root);
    resolver.output
}

struct Resolver<'a> {
    sources: &'a SourceMap,
    module_index: ModuleIndex<'a>,
    import_sources: &'a ImportSourceMap,
    prelude_sources: &'a PreludeSourceMap,
    output: ResolveOutput,
}
