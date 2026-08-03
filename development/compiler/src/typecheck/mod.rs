//! Type checking, ownership, borrowing, move, and drop checks.

mod allocation;
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
mod interface_bounds;
mod interfaces;
mod iteration;
mod literals;
mod member_presentation;
mod model;
mod numeric;
mod operations;
mod ownership;
mod places;
mod provenance;
mod provenance_contracts;
mod regions;
mod returns;
mod semantic_facts;
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
use regions::*;
use returns::*;
use sized::*;

pub(crate) use facts::{
    DropTypeSpecialization, FunctionCallSpecialization, MethodCallSpecialization,
    TypecheckCollectionForPlan, TypecheckCollectionForSourceMode, TypecheckFacts,
    TypecheckInterpolationPlan, TypecheckIterationMethod, TypecheckMethodReceiverKind,
    TypecheckPayloadBindingMode, TypecheckScalarViewKind, TypecheckSliceElementKind,
    collect_typecheck_facts, type_expr_presentation_label, type_symbol_presentation_label,
};
pub(crate) use member_presentation::{
    enum_variant_member_label, field_member_label, generic_type_owner_name, qualified_member_name,
};
pub(crate) use semantic_facts::{
    CallableSemanticFacts, StorageOriginFact, ValueProvenanceFact, collect_callable_semantic_facts,
};

pub(crate) fn type_expr_is_assignable(
    expected: &crate::ast::TypeExpr,
    actual: &crate::ast::TypeExpr,
    resolved: &ResolveOutput,
) -> bool {
    operations::is_assignable(
        &type_expr::type_expr_to_type(expected, resolved),
        &type_expr::type_expr_to_type(actual, resolved),
    )
}

pub(crate) fn type_expr_is_aborting_allocator_capability(
    ty: &crate::ast::TypeExpr,
    resolved: &ResolveOutput,
) -> bool {
    allocation::type_is_aborting_allocator_capability(
        &type_expr::type_expr_to_type(ty, resolved),
        resolved,
    )
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TypecheckSource<'a> {
    ast: &'a AstFile,
    resolved: &'a ResolveOutput,
}

impl<'a> TypecheckSource<'a> {
    pub(crate) fn new(ast: &'a AstFile, resolved: &'a ResolveOutput) -> Self {
        Self { ast, resolved }
    }
}

pub fn check(sources: &SourceMap, ast: &AstFile, resolved: &ResolveOutput) -> Vec<Diagnostic> {
    let summary_sources = [TypecheckSource::new(ast, resolved)];
    check_with_summary_sources(sources, ast, resolved, &summary_sources)
}

pub(crate) fn check_with_summary_sources(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    summary_sources: &[TypecheckSource<'_>],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_default_entry_function(sources, ast, resolved, &mut diagnostics);
    diagnostics.extend(check_module_with_summary_sources(
        sources,
        ast,
        resolved,
        summary_sources,
    ));

    diagnostics
}

pub fn check_module(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
) -> Vec<Diagnostic> {
    let summary_sources = [TypecheckSource::new(ast, resolved)];
    check_module_with_summary_sources(sources, ast, resolved, &summary_sources)
}

pub(crate) fn check_module_with_summary_sources(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    summary_sources: &[TypecheckSource<'_>],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    check_generic_type_arities(sources, ast, resolved, &mut diagnostics);
    check_drop_members(sources, ast, resolved, &mut diagnostics);
    check_sized_value_types(sources, ast, resolved, &mut diagnostics);
    check_interface_impls(sources, ast, resolved, &mut diagnostics);
    literals::check_literal_declarations(sources, ast, resolved, &mut diagnostics);
    check_body_expressions(sources, ast, resolved, &mut diagnostics);
    check_region_statements(sources, ast, &mut diagnostics);
    let provenance_summaries = callable_provenance_summaries(summary_sources);
    provenance_contracts::check_result_provenance_contracts(
        sources,
        ast,
        resolved,
        &provenance_summaries,
        &mut diagnostics,
    );
    check_ownership_states(
        sources,
        ast,
        resolved,
        &provenance_summaries,
        &mut diagnostics,
    );
    check_return_types(
        sources,
        ast,
        resolved,
        &provenance_summaries,
        &mut diagnostics,
    );

    diagnostics
}

#[cfg(test)]
mod tests;
