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
mod fallible;
mod model;
mod numeric;
mod operations;
mod returns;
mod structs;
mod type_expr;
mod variants;

use crate::ast::AstFile;
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;
use body::*;
use entry::*;
use returns::*;

pub fn check(sources: &SourceMap, ast: &AstFile, resolved: &ResolveOutput) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_program_entry(sources, ast, &mut diagnostics);
    diagnostics.extend(check_module(sources, ast, resolved));

    diagnostics
}

pub fn check_module(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_body_expressions(sources, ast, resolved, &mut diagnostics);
    check_return_types(sources, ast, resolved, &mut diagnostics);

    diagnostics
}

#[cfg(test)]
mod tests;
