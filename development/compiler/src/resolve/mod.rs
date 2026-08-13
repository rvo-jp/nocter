//! Import resolution, visibility, and name lookup.

mod body;
mod builtin_conformances;
mod builtin_surfaces;
mod builtins;
mod closures;
mod coercions;
mod collector;
mod compile_unit_context;
mod conformance;
mod constructions;
mod declaration_index;
mod diagnostics;
mod generic_requirements;
mod imports;
mod literals;
mod module_index;
mod regions;
mod signatures;
mod standard_conformances;
mod symbols;
mod type_surfaces;

#[cfg(test)]
pub(crate) use builtin_conformances::attach_test_builtin_conformances;

#[cfg(test)]
mod tests;

pub use generic_requirements::{GenericRequirement, GenericRequirements};
pub use symbols::{
    AssociatedFunctionSignature, AssociatedTypeBindingSignature, AssociatedTypeSignature,
    CoercionSignature, ConstructionEntry, ConstructionEntryKind, ConstructionSurface,
    DestructSignature, EnumVariantSignature, FunctionSignature, ImportAccess, ImportKind,
    ImportSource, ImportSourceMap, ImportedSymbol, ImportedSymbolKind, InterfaceConformance,
    LiteralCaptureSignature, LiteralResolution, LiteralSignature, LocalSymbol, LocalSymbolId,
    LocalSymbolKind, MethodSignature, ParameterSignature, PreludeSourceMap, ResolveOutput,
    StructFieldSignature, Symbol, SymbolId, SymbolKind, SymbolTable, TypeSymbol, TypeSymbolKind,
};

pub(crate) use compile_unit_context::ResolveCompileUnitContext;
pub(crate) use declaration_index::ResolvedDeclaration;
use module_index::ModuleIndex;

use crate::ast::{AstFile, Item};
use crate::callable_bodies::CallableBodyIndex;
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::source_scopes::SourceScopeMap;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

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
    let context = ResolveCompileUnitContext::new(files, import_sources);
    resolve_compile_unit_with_context(
        sources,
        root,
        files,
        import_sources,
        prelude_sources,
        callable_bodies,
        source_scopes,
        &context,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_compile_unit_with_context(
    sources: &SourceMap,
    root: &AstFile,
    files: &[AstFile],
    import_sources: &ImportSourceMap,
    prelude_sources: &PreludeSourceMap,
    callable_bodies: &CallableBodyIndex,
    source_scopes: &SourceScopeMap,
    context: &ResolveCompileUnitContext,
) -> ResolveOutput {
    let source_modules = &context.source_modules;
    let module_index = ModuleIndex::new(&context.merged_modules);
    let module_ast = crate::timing::measure_detail("resolve.declaration_surface", || {
        module_index
            .ast_for_source(root.span.source)
            .map(|ast| callable_bodies.declaration_surface(ast))
            .unwrap_or_else(|| root.clone())
    });
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
        output: ResolveOutput::new(access, context.semantic_db.clone())
            .with_callable_bodies(callable_bodies.clone())
            .with_source_modules(source_modules.clone())
            .with_source_scopes(source_scopes.clone()),
        synthetic_prelude_symbol_spans: HashSet::new(),
        collected_hidden_type_dependencies: HashSet::new(),
        collecting_imported_type_names: RefCell::new(HashSet::new()),
        imported_type_name_cache: &context.imported_type_names,
        prepared_external_surface_sources: HashSet::new(),
        collecting_synthetic_prelude: false,
    };

    crate::timing::measure_detail("resolve.collect_top_level", || {
        resolver.collect_top_level_symbols(&module_ast)
    });
    crate::timing::measure_detail("resolve.collect_builtin_surfaces", || {
        resolver.collect_builtin_source_surfaces(&module_ast)
    });
    crate::timing::measure_detail("resolve.collect_builtin_conformances", || {
        resolver.collect_builtin_conformance_surfaces()
    });
    crate::timing::measure_detail("resolve.collect_standard_conformances", || {
        resolver.collect_standard_nominal_conformance_surfaces()
    });
    crate::timing::measure_detail("resolve.callable_bodies", || {
        resolver.resolve_callable_bodies(root)
    });
    resolver.output.rebuild_declaration_index();
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
            | Item::Instance(_)
            | Item::Conformance(_)
            | Item::Destruct(_)
            | Item::Construct(_) => continue,
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
    imported_type_name_cache: &'a RefCell<HashMap<SourceId, Vec<imports::ImportedTypeName>>>,
    prepared_external_surface_sources: HashSet<SourceId>,
    collecting_synthetic_prelude: bool,
}
