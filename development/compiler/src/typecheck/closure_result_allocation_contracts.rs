//! Result-allocation variance for concrete closures.
//!
//! Closures do not carry a written modifier. Their result effect is inferred
//! from the body, then checked when the materialized closure type crosses a
//! structural callable bound. The bound remains an upper bound: a narrower
//! non-allocating closure may satisfy `alloc`, never the reverse.

use super::closures::environment_for_materialized_closure;
use super::diagnostics::closure_result_allocation_contract_diagnostic;
use super::facts::collect_typecheck_facts;
use super::provenance::CallableProvenanceSummaries;
use super::returns::result_allocation_witness_for_callable_body;
use super::type_expr::type_expr_to_type_in_environment;
use crate::ast::{AstFile, TypeExpr, closure_expression_by_span};
use crate::diagnostics::Diagnostic;
use crate::resolve::{FunctionSignature, ResolveOutput, SymbolKind};
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

pub(super) fn check_closure_result_allocation_contracts(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let facts = collect_typecheck_facts(ast, resolved);
    let evidence = closure_allocation_evidence(ast, resolved, summaries, &facts);

    for (_, specialization) in facts.function_call_specialization_entries() {
        let Some(signature) = callable_signature_at(resolved, specialization.declaration_span)
        else {
            continue;
        };
        check_substitutions(
            sources,
            signature,
            &specialization.substitutions,
            &evidence,
            diagnostics,
        );
    }
    for (_, specialization) in facts.method_call_specialization_entries() {
        let Some(signature) = callable_signature_at(resolved, specialization.declaration_span)
        else {
            continue;
        };
        check_substitutions(
            sources,
            signature,
            &specialization.substitutions,
            &evidence,
            diagnostics,
        );
    }
}

fn closure_allocation_evidence(
    ast: &AstFile,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    facts: &super::facts::TypecheckFacts,
) -> HashMap<ByteSpan, ByteSpan> {
    let mut evidence = HashMap::new();
    for (span, plan) in facts.closure_plan_entries() {
        let Some(closure) = closure_expression_by_span(ast, span) else {
            continue;
        };
        let environment = environment_for_materialized_closure(closure, &plan.ty, resolved);
        let return_type =
            type_expr_to_type_in_environment(&plan.ty.return_type, resolved, &environment);
        if let Some(witness) = result_allocation_witness_for_callable_body(
            &closure.body,
            &return_type,
            resolved,
            &environment,
            summaries,
        ) {
            evidence.insert(span, witness);
        }
    }
    evidence
}

fn callable_signature_at(
    resolved: &ResolveOutput,
    declaration_span: ByteSpan,
) -> Option<&FunctionSignature> {
    resolved
        .symbols
        .symbols()
        .find_map(|symbol| match &symbol.kind {
            SymbolKind::Function(signature) | SymbolKind::Primitive(signature)
                if symbol.declaration_span == declaration_span =>
            {
                Some(signature)
            }
            SymbolKind::Type(owner) => owner
                .associated_functions
                .iter()
                .find(|function| function.name_span == declaration_span)
                .map(|function| &function.signature)
                .or_else(|| {
                    owner
                        .methods
                        .iter()
                        .find(|method| method.name_span == declaration_span)
                        .map(|method| &method.signature)
                })
                .or_else(|| {
                    owner
                        .interface_conformances
                        .iter()
                        .flat_map(|conformance| &conformance.methods)
                        .find(|method| method.name_span == declaration_span)
                        .map(|method| &method.signature)
                }),
            SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Imported(_) => None,
        })
}

fn check_substitutions(
    sources: &SourceMap,
    signature: &FunctionSignature,
    substitutions: &HashMap<String, TypeExpr>,
    evidence: &HashMap<ByteSpan, ByteSpan>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (parameter, bounds) in signature
        .generic_parameters
        .iter()
        .zip(&signature.generic_parameter_bounds)
    {
        let Some(TypeExpr::Closure(closure)) = substitutions.get(parameter) else {
            continue;
        };
        let Some(witness) = evidence.get(&closure.span).copied() else {
            continue;
        };
        for bound in bounds {
            let TypeExpr::Callable(callable) = bound else {
                continue;
            };
            if callable.result_allocation.is_none() {
                diagnostics.push(closure_result_allocation_contract_diagnostic(
                    sources,
                    witness,
                    bound.span(),
                ));
            }
        }
    }
}
