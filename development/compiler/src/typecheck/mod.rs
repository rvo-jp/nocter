//! Type checking, ownership, borrowing, move, and drop checks.

mod arrays;
mod bindings;
mod body;
mod calls;
mod controls;
mod copyability;
mod diagnostics;
mod drop_members;
mod entry;
mod environments;
mod expressions;
mod facts;
mod fallible;
mod generics;
mod interfaces;
mod model;
mod numeric;
mod operations;
mod ownership;
mod places;
mod returns;
mod sized;
mod strings;
mod structs;
mod type_expr;
mod variants;
mod visibility;

use crate::ast::AstFile;
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;
use body::*;
use drop_members::*;
use entry::*;
use generics::*;
use interfaces::*;
use ownership::*;
use returns::*;
use sized::*;

pub(crate) use facts::{
    DropTypeSpecialization, FunctionCallSpecialization, MethodCallSpecialization, TypecheckFacts,
    TypecheckMethodReceiverKind, TypecheckScalarViewKind, TypecheckSliceElementKind,
    collect_typecheck_facts,
};

pub fn check(sources: &SourceMap, ast: &AstFile, resolved: &ResolveOutput) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_default_entry_function(sources, ast, resolved, &mut diagnostics);
    diagnostics.extend(check_module(sources, ast, resolved));

    diagnostics
}

pub fn check_module(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_generic_type_arities(sources, ast, resolved, &mut diagnostics);
    check_drop_members(sources, ast, resolved, &mut diagnostics);
    check_sized_value_types(sources, ast, resolved, &mut diagnostics);
    check_interface_impls(sources, ast, resolved, &mut diagnostics);
    check_body_expressions(sources, ast, resolved, &mut diagnostics);
    check_ownership_states(sources, ast, resolved, &mut diagnostics);
    check_return_types(sources, ast, resolved, &mut diagnostics);

    diagnostics
}

#[cfg(test)]
mod tests;
