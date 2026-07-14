//! Type checking, ownership, borrowing, move, and drop checks.

mod arrays;
mod bindings;
mod body;
mod calls;
mod controls;
mod diagnostics;
mod entry;
mod environments;
mod expressions;
mod facts;
mod fallible;
mod model;
mod numeric;
mod operations;
mod ownership;
mod returns;
mod sized;
mod strings;
mod structs;
mod type_expr;
mod variants;

use crate::ast::AstFile;
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;
use body::*;
use entry::*;
use ownership::*;
use returns::*;
use sized::*;

pub(crate) use facts::{TypecheckFacts, collect_typecheck_facts};

pub fn check(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    entry_name: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_default_entry_function(sources, ast, entry_name, &mut diagnostics);
    diagnostics.extend(check_module(sources, ast, resolved));

    diagnostics
}

pub fn check_module(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_sized_value_types(sources, ast, resolved, &mut diagnostics);
    check_body_expressions(sources, ast, resolved, &mut diagnostics);
    check_ownership_states(sources, ast, resolved, &mut diagnostics);
    check_return_types(sources, ast, resolved, &mut diagnostics);

    diagnostics
}

#[cfg(test)]
mod tests;
