//! Validation for public result-allocation contracts.
//!
//! Body-bearing callables use an exact contract: `alloc` is required exactly
//! when a returned value can retain newly allocated storage. Bodyless
//! primitives and interface methods are declarations, so their modifiers are
//! consumed as contracts rather than checked against unavailable bodies.

use super::diagnostics::{
    incompatible_trusted_result_allocation_contract_diagnostic,
    missing_result_allocation_contract_diagnostic,
    unjustified_result_allocation_contract_diagnostic,
};
use super::environments::{
    environment_for_function, environment_for_interface_method, environment_for_literal,
    environment_for_method,
};
use super::model::TypeEnvironment;
use super::provenance::{CallableId, CallableProvenanceSummaries, result_contains_allocation};
use super::returns::{function_summary_key, result_allocation_witness_for_callable_body};
use super::type_expr::type_expr_to_type_in_environment;
use crate::ast::{AstFile, Block, ImplMember, Item, ResultAllocationModifier, TypeExpr};
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
            Item::Function(function) => {
                let environment = environment_for_function(function, resolved);
                check_body_contract(
                    sources,
                    function_summary_key(function),
                    function.member_name_span,
                    function.result_allocation.as_ref(),
                    &function.return_type,
                    &function.body,
                    resolved,
                    &environment,
                    summaries,
                    diagnostics,
                );
            }
            Item::Interface(interface) => {
                for method in &interface.methods {
                    if let Some(body) = &method.body {
                        let environment =
                            environment_for_interface_method(method, resolved, interface);
                        check_body_contract(
                            sources,
                            method.name_span,
                            method.name_span,
                            method.result_allocation.as_ref(),
                            &method.return_type,
                            body,
                            resolved,
                            &environment,
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
                        let environment = environment_for_method(method, resolved, implementation);
                        check_body_contract(
                            sources,
                            method.name_span,
                            method.name_span,
                            method.result_allocation.as_ref(),
                            &method.return_type,
                            body,
                            resolved,
                            &environment,
                            summaries,
                            diagnostics,
                        );
                    }
                }
            }
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    let environment = environment_for_function(function, resolved);
                    check_body_contract(
                        sources,
                        function_summary_key(function),
                        function.member_name_span,
                        function.result_allocation.as_ref(),
                        &function.return_type,
                        &function.body,
                        resolved,
                        &environment,
                        summaries,
                        diagnostics,
                    );
                }
                for (_, literal) in construct.literals() {
                    let environment = environment_for_literal(literal, resolved);
                    check_body_contract(
                        sources,
                        literal.span,
                        literal.keyword_span,
                        literal.result_allocation.as_ref(),
                        &literal.return_type,
                        &literal.body,
                        resolved,
                        &environment,
                        summaries,
                        diagnostics,
                    );
                }
            }
            Item::Primitive(primitive) => check_trusted_primitive_contract(
                sources,
                primitive.name_span,
                primitive.result_allocation.as_ref(),
                resolved,
                diagnostics,
            ),
            Item::Test(_)
            | Item::Import(_)
            | Item::FromImport(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_) => {}
        }
    }
}

fn check_trusted_primitive_contract(
    sources: &SourceMap,
    declaration_span: ByteSpan,
    modifier: Option<&ResultAllocationModifier>,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(role) = resolved.trusted_declarations.role(declaration_span) else {
        // An ordinary bodyless primitive owns its declared upper-bound contract.
        return;
    };
    match (role, modifier) {
        (TrustedDeclarationRole::AllocationOperation { .. }, None) => diagnostics.push(
            missing_result_allocation_contract_diagnostic(sources, declaration_span, None),
        ),
        (TrustedDeclarationRole::AllocationOperation { .. }, Some(_)) => {}
        (_, Some(modifier)) => {
            diagnostics.push(incompatible_trusted_result_allocation_contract_diagnostic(
                sources,
                modifier.span,
                declaration_span,
            ))
        }
        (_, None) => {}
    }
}

fn check_body_contract(
    sources: &SourceMap,
    summary_key: ByteSpan,
    declaration_span: ByteSpan,
    modifier: Option<&ResultAllocationModifier>,
    return_type: &TypeExpr,
    body: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let return_type = type_expr_to_type_in_environment(return_type, resolved, environment);
    let inferred = matches!(
        resolved.trusted_declarations.role(summary_key),
        Some(TrustedDeclarationRole::AllocationOperation { .. })
    ) || summaries
        .result(CallableId::declared_at(summary_key))
        .is_some_and(|summary| result_contains_allocation(summary, &return_type, resolved));
    match (modifier, inferred) {
        (None, true) => {
            let witness = result_allocation_witness_for_callable_body(
                body,
                &return_type,
                resolved,
                environment,
                summaries,
            );
            diagnostics.push(missing_result_allocation_contract_diagnostic(
                sources,
                declaration_span,
                witness,
            ));
        }
        (Some(modifier), false) => diagnostics.push(
            unjustified_result_allocation_contract_diagnostic(sources, modifier.span, body.span),
        ),
        (None, false) | (Some(_), true) => {}
    }
}
