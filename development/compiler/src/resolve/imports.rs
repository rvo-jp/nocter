use super::body::Scope;
use super::builtins::is_builtin_type_name;
use super::diagnostics::{
    builtin_name_reuse_diagnostic, missing_import_diagnostic, restricted_import_diagnostic,
    unloaded_import_diagnostic,
};
use super::module_index::is_relative_module_path;
use super::signatures::{
    alias_type_symbol, attach_inherent_impl_members_to_symbol, enum_type_symbol,
    function_signature, interface_type_symbol, primitive_signature, struct_type_symbol,
};
use super::{
    FunctionSignature, ImportAccess, ImportedSymbol, ImportedSymbolKind, MethodSignature,
    ParameterSignature, Resolver, SymbolId, SymbolKind, TypeSymbol,
};
use crate::ast::{AstFile, FromImportItem, ImportItem, Item, TypeAliasDecl, TypeExpr, Visibility};
use crate::source::{ByteSpan, SourceId};
use std::collections::HashSet;

mod collection;
mod lookup;
mod model;
mod qualification;
mod symbols;

use model::*;
use qualification::*;
use symbols::*;

impl Resolver<'_> {
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
        self.collect_hidden_imported_type_symbols(
            ast,
            module_path,
            self.output.access,
            &local_type_names,
        );
        self.collect_hidden_imported_type_dependencies(&imported_type_names);
    }
}
