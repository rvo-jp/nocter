use super::body::Scope;
use super::builtins::is_builtin_type_name;
use super::diagnostics::{
    builtin_name_reuse_diagnostic, missing_import_diagnostic, restricted_import_diagnostic,
    unloaded_import_diagnostic,
};
use super::literals::attach_literal_definitions_to_symbol;
use super::module_index::is_relative_module_path;
use super::signatures::{
    alias_type_symbol, attach_inherent_impl_members_to_symbol, enum_type_symbol,
    function_signature, interface_type_symbol, primitive_signature, struct_type_symbol,
};
use super::{
    FunctionSignature, ImportAccess, ImportedSymbol, ImportedSymbolKind, ParameterSignature,
    Resolver, SymbolId, SymbolKind, TypeSymbol,
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
