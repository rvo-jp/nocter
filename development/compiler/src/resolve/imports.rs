use super::body::Scope;
use super::builtins::is_builtin_type_name;
use super::diagnostics::{
    builtin_name_reuse_diagnostic, missing_import_diagnostic, restricted_import_diagnostic,
    unloaded_import_diagnostic, widening_reexport_diagnostic,
};
use super::module_index::is_relative_module_path;
use super::signatures::{
    alias_type_symbol, attach_behavior_declarations_to_symbol, enum_type_symbol,
    function_signature, interface_type_symbol, primitive_signature, struct_type_symbol,
};
use super::{
    FunctionSignature, ImportAccess, ImportedSymbol, ImportedSymbolKind, InterfaceConformance,
    MethodSignature, ParameterSignature, Resolver, SymbolId, SymbolKind, TypeSymbol,
};
use crate::ast::{AstFile, FromImportItem, ImportItem, Item, TypeAliasDecl, TypeExpr, Visibility};
use crate::source::{ByteSpan, SourceId};
use std::collections::HashSet;

mod collection;
mod lookup;
mod model;
mod qualification;
mod symbols;

pub(in crate::resolve) use model::ImportedTypeName;
use model::{ImportableSymbol, ReexportLookup};
use qualification::*;
use symbols::*;

impl Resolver<'_> {
    fn filter_importable_members_for_use(
        &self,
        imported: ImportableSymbol,
        use_source: SourceId,
    ) -> ImportableSymbol {
        let member_access = self
            .output
            .source_access(imported.declaration_span.source, use_source);
        filter_importable_symbol_for_access(imported, member_access)
    }

    /// Qualifies the signatures carried by an implicitly loaded built-in type
    /// surface exactly as an explicit import would. The surface itself is
    /// globally available, but every type mentioned by its API still belongs
    /// to the declaration module and must retain that canonical identity.
    pub(super) fn prepare_builtin_surface_methods(
        &mut self,
        ast: &AstFile,
        module_path: &str,
        methods: &mut [MethodSignature],
    ) {
        let local_type_names = type_decl_names(ast);
        let imported_type_names = self.imported_type_names(ast);
        for method in methods {
            qualify_method_signature(method, module_path, &local_type_names, &imported_type_names);
        }
        if self
            .prepared_external_surface_sources
            .insert(ast.span.source)
        {
            self.collect_hidden_imported_type_symbols(
                ast,
                module_path,
                self.output.access,
                &local_type_names,
            );
            self.collect_hidden_imported_type_dependencies(&imported_type_names);
        }
    }

    pub(super) fn prepare_builtin_surface_symbol(
        &mut self,
        ast: &AstFile,
        module_path: &str,
        symbol: &mut TypeSymbol,
    ) {
        let local_type_names = type_decl_names(ast);
        let imported_type_names = self.imported_type_names(ast);
        qualify_type_symbol(symbol, module_path, &local_type_names, &imported_type_names);
        if self
            .prepared_external_surface_sources
            .insert(ast.span.source)
        {
            self.collect_hidden_imported_type_symbols(
                ast,
                module_path,
                self.output.access,
                &local_type_names,
            );
            self.collect_hidden_imported_type_dependencies(&imported_type_names);
        }
    }

    pub(super) fn prepare_external_conformance(
        &mut self,
        ast: &AstFile,
        module_path: &str,
        conformance: &mut InterfaceConformance,
    ) {
        let local_type_names = type_decl_names(ast);
        let imported_type_names = self.imported_type_names(ast);
        qualify_interface_conformance(
            conformance,
            module_path,
            &local_type_names,
            &imported_type_names,
        );
        if self
            .prepared_external_surface_sources
            .insert(ast.span.source)
        {
            self.collect_hidden_imported_type_symbols(
                ast,
                module_path,
                self.output.access,
                &local_type_names,
            );
            self.collect_hidden_imported_type_dependencies(&imported_type_names);
        }
    }
}
