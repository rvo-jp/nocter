//! Validation for public result-allocation contracts.
//!
//! Body-bearing callables use an exact contract: `alloc` is required exactly
//! when a returned value can retain newly allocated storage. Bodyless
//! primitives and interface methods are declarations, so their modifiers are
//! consumed as contracts rather than checked against unavailable bodies.

use super::diagnostics::{
    missing_result_allocation_contract_diagnostic,
    unjustified_result_allocation_contract_diagnostic,
};
use super::provenance::{CallableId, CallableProvenanceSummaries, ValueProvenance};
use super::returns::function_summary_key;
use crate::ast::{AstFile, Block, ImplMember, Item, ResultAllocationModifier};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::semantics::TrustedDeclarationRole;
use crate::source::{ByteSpan, SourceMap};

pub(super) fn check_result_allocation_contracts(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Function(function) => check_body_contract(
                sources,
                function_summary_key(function),
                function.member_name_span,
                function.result_allocation.as_ref(),
                &function.body,
                resolved,
                summaries,
                diagnostics,
            ),
            Item::Interface(interface) => {
                for method in &interface.methods {
                    if let Some(body) = &method.body {
                        check_body_contract(
                            sources,
                            method.name_span,
                            method.name_span,
                            method.result_allocation.as_ref(),
                            body,
                            resolved,
                            summaries,
                            diagnostics,
                        );
                    }
                }
            }
            Item::Impl(implementation) => {
                for member in &implementation.members {
                    if let ImplMember::Method(method) = member
                        && let Some(body) = &method.body
                    {
                        check_body_contract(
                            sources,
                            method.name_span,
                            method.name_span,
                            method.result_allocation.as_ref(),
                            body,
                            resolved,
                            summaries,
                            diagnostics,
                        );
                    }
                }
            }
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    check_body_contract(
                        sources,
                        function_summary_key(function),
                        function.member_name_span,
                        function.result_allocation.as_ref(),
                        &function.body,
                        resolved,
                        summaries,
                        diagnostics,
                    );
                }
                for (_, literal) in construct.literals() {
                    check_body_contract(
                        sources,
                        literal.span,
                        literal.keyword_span,
                        literal.result_allocation.as_ref(),
                        &literal.body,
                        resolved,
                        summaries,
                        diagnostics,
                    );
                }
            }
            Item::Primitive(_)
            | Item::Test(_)
            | Item::Import(_)
            | Item::FromImport(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_) => {}
        }
    }
}

fn check_body_contract(
    sources: &SourceMap,
    summary_key: ByteSpan,
    declaration_span: ByteSpan,
    modifier: Option<&ResultAllocationModifier>,
    body: &Block,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let inferred = matches!(
        resolved.trusted_declarations.role(summary_key),
        Some(TrustedDeclarationRole::AllocationOperation { .. })
    ) || summaries
        .result(CallableId::declared_at(summary_key))
        .is_some_and(ValueProvenance::contains_result_allocation);
    match (modifier, inferred) {
        (None, true) => diagnostics.push(missing_result_allocation_contract_diagnostic(
            sources,
            declaration_span,
        )),
        (Some(modifier), false) => diagnostics.push(
            unjustified_result_allocation_contract_diagnostic(sources, modifier.span, body.span),
        ),
        (None, false) | (Some(_), true) => {}
    }
}
