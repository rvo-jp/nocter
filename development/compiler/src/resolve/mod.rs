//! Import resolution, visibility, and name lookup.

mod body;
mod builtin_impls;
mod builtins;
mod closures;
mod coercions;
mod collector;
mod conformance;
mod constructions;
mod diagnostics;
mod generic_requirements;
mod imports;
mod literals;
mod module_index;
mod regions;
mod signatures;
mod symbols;
mod type_surfaces;

#[cfg(test)]
mod tests;

pub use generic_requirements::{GenericRequirement, GenericRequirements};
pub use symbols::{
    AssociatedFunctionSignature, AssociatedTypeBindingSignature, AssociatedTypeSignature,
    CoercionSignature, ConstructionEntry, ConstructionEntryKind, ConstructionSurface,
    DropSignature, EnumVariantSignature, FunctionSignature, ImportAccess, ImportKind, ImportSource,
    ImportSourceMap, ImportedSymbol, ImportedSymbolKind, InterfaceConformance,
    LiteralCaptureSignature, LiteralResolution, LiteralSignature, LocalSymbol, LocalSymbolId,
    LocalSymbolKind, MethodSignature, ParameterSignature, PreludeSourceMap, ResolveOutput,
    StructFieldSignature, Symbol, SymbolId, SymbolKind, SymbolTable, TypeSymbol, TypeSymbolKind,
};

use module_index::{MergedModules, ModuleIndex};

use crate::ast::{AstFile, Item};
use crate::callable_bodies::CallableBodyIndex;
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::source_modules::SourceModuleMap;
use crate::source_scopes::SourceScopeMap;
use std::cell::RefCell;
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
    resolve_compile_unit_with_callable_bodies(
        sources,
        root,
        files,
        import_sources,
        prelude_sources,
        &CallableBodyIndex::default(),
        &SourceScopeMap::default(),
    )
}

pub(crate) fn resolve_compile_unit_with_callable_bodies(
    sources: &SourceMap,
    root: &AstFile,
    files: &[AstFile],
    import_sources: &ImportSourceMap,
    prelude_sources: &PreludeSourceMap,
    callable_bodies: &CallableBodyIndex,
    source_scopes: &SourceScopeMap,
) -> ResolveOutput {
    let source_modules = SourceModuleMap::new(files, import_sources);
    let merged_modules = MergedModules::new(files, import_sources);
    let module_index = ModuleIndex::new(&merged_modules);
    let module_ast = module_index
        .ast_for_source(root.span.source)
        .map(|ast| callable_bodies.declaration_surface(ast))
        .unwrap_or_else(|| root.clone());
    let root_module = source_modules
        .module(root.span.source)
        .unwrap_or(root.span.source);
    let access = files
        .iter()
        .filter(|file| {
            source_modules
                .module(file.span.source)
                .unwrap_or(file.span.source)
                == root_module
        })
        .map(|file| root_access(file, import_sources, prelude_sources))
        .find(|access| matches!(access, ImportAccess::Package { .. }))
        .unwrap_or(ImportAccess::Public);
    let mut resolver = Resolver {
        sources,
        module_index,
        import_sources,
        prelude_sources,
        output: ResolveOutput::new(access)
            .with_callable_bodies(callable_bodies.clone())
            .with_source_modules(source_modules)
            .with_source_scopes(source_scopes.clone()),
        synthetic_prelude_symbol_spans: HashSet::new(),
        collected_hidden_type_dependencies: HashSet::new(),
        collecting_imported_type_names: RefCell::new(HashSet::new()),
        collecting_synthetic_prelude: false,
    };

    resolver.collect_top_level_symbols(&module_ast);
    resolver.collect_builtin_impl_surfaces(&module_ast);
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
            | Item::Test(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_)
            | Item::Interface(_)
            | Item::Impl(_)
            | Item::Construct(_)
            | Item::Coerce(_) => continue,
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
    collected_hidden_type_dependencies: HashSet<(SourceId, String)>,
    collecting_imported_type_names: RefCell<HashSet<SourceId>>,
    collecting_synthetic_prelude: bool,
}
