//! Type checking, ownership, borrowing, move, and drop checks.

mod allocation;
mod arrays;
mod associated_types;
mod bindings;
mod body;
mod callables;
mod calls;
mod closures;
mod coercions;
mod conformance;
mod controls;
mod conversions;
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
mod interface_impl_members;
mod interface_methods;
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
mod sized;
mod strings;
mod structs;
mod test_declarations;
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
    CallableCallFact, CallableCallSpecialization, DropTypeSpecialization,
    FunctionCallSpecialization, GenericParameterFact, MethodCallSpecialization,
    TypeOccurrenceTarget, TypecheckClosurePlan, TypecheckCoercionPlan, TypecheckCollectionForPlan,
    TypecheckCollectionForSourceMode, TypecheckConversionKind, TypecheckConversionPlan,
    TypecheckFacts, TypecheckInterpolationPlan, TypecheckIterationMethod,
    TypecheckMethodReceiverKind, TypecheckPayloadBindingMode, TypecheckScalarViewKind,
    TypecheckSequenceSpreadMode, TypecheckSequenceSpreadPlan, TypecheckSliceElementKind,
    collect_typecheck_facts, drop_type_specialization_from_self_ty, type_expr_presentation_label,
    type_symbol_presentation_label,
};
pub(crate) use interface_methods::completion_candidates_for_type_expr as interface_method_completion_candidates;
pub(crate) use interface_methods::implementation_for_interface_type_expr;
pub(crate) use literals::sequence_spread;
pub(crate) use member_presentation::{
    enum_variant_member_label, field_member_label, generic_type_owner_name,
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

pub(crate) fn normalize_associated_type_expr(
    ty: &crate::ast::TypeExpr,
    resolved: &ResolveOutput,
) -> Option<crate::ast::TypeExpr> {
    let normalized = type_expr::type_expr_to_type(ty, resolved);
    let mut parameters = std::collections::HashSet::new();
    let result =
        facts::type_to_type_expr_allowing_parameters(&normalized, ty.span(), &mut parameters)?;
    (!matches!(result, crate::ast::TypeExpr::Projection(_))).then_some(result)
}

pub(crate) fn concrete_associated_types(
    ty: &crate::ast::TypeExpr,
    resolved: &ResolveOutput,
) -> Vec<(String, String, crate::source::ByteSpan)> {
    let ty = type_expr::type_expr_to_type(ty, resolved);
    let mut entries = conformance::implemented_interface_conformances(&ty, resolved)
        .into_iter()
        .filter_map(|(_, interface)| {
            resolved.type_symbol_by_canonical_name(interface.nominal_name()?)
        })
        .flat_map(|interface| {
            interface.associated_types.iter().map(|associated| {
                (
                    associated.name.clone(),
                    interface.canonical_name.clone(),
                    associated.name_span,
                )
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.1
            .cmp(&right.1)
            .then(left.0.cmp(&right.0))
            .then(left.2.source.raw().cmp(&right.2.source.raw()))
            .then(left.2.start.cmp(&right.2.start))
    });
    entries.dedup();
    entries
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

    test_declarations::check_test_declarations(sources, ast, &mut diagnostics);
    check_generic_type_arities(sources, ast, resolved, &mut diagnostics);
    associated_types::check_declarations(sources, ast, &mut diagnostics);
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
