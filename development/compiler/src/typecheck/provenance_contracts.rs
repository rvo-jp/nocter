use super::diagnostics::{
    independent_result_contract_diagnostic, invalid_provenance_origin_diagnostic,
    missing_result_contract_diagnostic, result_contract_violation_diagnostic,
};
use super::environments::{
    environment_for_function, environment_for_literal, environment_for_method,
};
use super::model::TypeEnvironment;
use super::provenance::{
    CallableProvenanceSummaries, eligible_input_origin_count, provenance_satisfies_contract,
    result_provenance_contract, type_may_carry_result_provenance,
};
use super::returns::{borrow_return_provenance_for_callable_body, type_expr_contains_borrow_like};
use super::type_expr::type_expr_to_type_in_environment;
use crate::ast::{
    AstFile, ImplDecl, ImplMember, Item, MethodDecl, Parameter, ResultProvenanceClause, TypeExpr,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn check_result_provenance_contracts(
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
                check_clause(
                    sources,
                    function.result_provenance.as_ref(),
                    None,
                    &function.parameters.parameters,
                    &function.return_type,
                    Some(&environment),
                    resolved,
                    diagnostics,
                );
                if let Some(clause) = &function.result_provenance {
                    check_body_contract(
                        sources,
                        &function.body,
                        clause,
                        None,
                        &function.parameters.parameters,
                        &function.return_type,
                        resolved,
                        &environment,
                        summaries,
                        diagnostics,
                    );
                }
            }
            Item::Primitive(primitive) => check_clause(
                sources,
                primitive.result_provenance.as_ref(),
                None,
                &primitive.parameters.parameters,
                &primitive.return_type,
                None,
                resolved,
                diagnostics,
            ),
            Item::Literal(literal) => {
                let environment = environment_for_literal(literal, resolved);
                check_clause(
                    sources,
                    literal.result_provenance.as_ref(),
                    None,
                    &literal.parameters.parameters,
                    &literal.return_type,
                    Some(&environment),
                    resolved,
                    diagnostics,
                );
                if let Some(clause) = &literal.result_provenance {
                    check_body_contract(
                        sources,
                        &literal.body,
                        clause,
                        None,
                        &literal.parameters.parameters,
                        &literal.return_type,
                        resolved,
                        &environment,
                        summaries,
                        diagnostics,
                    );
                }
            }
            Item::Interface(interface) => {
                for method in &interface.methods {
                    check_clause(
                        sources,
                        method.result_provenance.as_ref(),
                        Some(method),
                        &method.parameters.parameters,
                        &method.return_type,
                        None,
                        resolved,
                        diagnostics,
                    );
                    if method.body.is_none()
                        && method.result_provenance.is_none()
                        && return_type_carries_storage(&method.return_type, None, resolved)
                    {
                        let eligible = eligible_input_origin_count(
                            Some(method),
                            &method.parameters.parameters,
                            resolved,
                        );
                        if eligible != 1 {
                            diagnostics.push(missing_result_contract_diagnostic(
                                sources,
                                method.return_type.span(),
                                eligible,
                            ));
                        }
                    }
                }
            }
            Item::Impl(impl_) => {
                check_impl_methods(sources, impl_, resolved, summaries, diagnostics)
            }
            _ => {}
        }
    }
}

fn check_impl_methods(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for member in &impl_.members {
        let ImplMember::Method(method) = member else {
            continue;
        };
        check_clause(
            sources,
            method.result_provenance.as_ref(),
            Some(method),
            &method.parameters.parameters,
            &method.return_type,
            Some(&environment_for_method(method, resolved, impl_)),
            resolved,
            diagnostics,
        );
        let (Some(clause), Some(body)) = (&method.result_provenance, &method.body) else {
            continue;
        };
        let environment = environment_for_method(method, resolved, impl_);
        check_body_contract(
            sources,
            body,
            clause,
            Some(method),
            &method.parameters.parameters,
            &method.return_type,
            resolved,
            &environment,
            summaries,
            diagnostics,
        );
    }
}

fn check_clause(
    sources: &SourceMap,
    clause: Option<&ResultProvenanceClause>,
    method: Option<&MethodDecl>,
    parameters: &[Parameter],
    return_type: &TypeExpr,
    environment: Option<&TypeEnvironment>,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(clause) = clause else {
        return;
    };
    if !return_type_carries_storage(return_type, environment, resolved) {
        diagnostics.push(independent_result_contract_diagnostic(
            sources,
            clause,
            return_type,
        ));
    }
    if let Err(errors) = result_provenance_contract(clause, method, parameters, resolved) {
        diagnostics.extend(
            errors.into_iter().map(|(origin, error)| {
                invalid_provenance_origin_diagnostic(sources, origin, &error)
            }),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn check_body_contract(
    sources: &SourceMap,
    body: &crate::ast::Block,
    clause: &ResultProvenanceClause,
    method: Option<&MethodDecl>,
    parameters: &[Parameter],
    return_type: &TypeExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Ok(contract) = result_provenance_contract(clause, method, parameters, resolved) else {
        return;
    };
    let return_type = type_expr_to_type_in_environment(return_type, resolved, environment);
    let actual = borrow_return_provenance_for_callable_body(
        body,
        &return_type,
        resolved,
        environment,
        summaries,
    );
    if actual
        .as_ref()
        .is_some_and(|actual| !provenance_satisfies_contract(actual, &contract))
    {
        diagnostics.push(result_contract_violation_diagnostic(
            sources, body.span, clause,
        ));
    }
}

fn return_type_carries_storage(
    return_type: &TypeExpr,
    environment: Option<&TypeEnvironment>,
    resolved: &ResolveOutput,
) -> bool {
    if let Some(environment) = environment {
        let return_type = type_expr_to_type_in_environment(return_type, resolved, environment);
        return type_may_carry_result_provenance(&return_type, resolved);
    }
    type_expr_contains_borrow_like(
        return_type,
        resolved,
        &std::collections::HashMap::new(),
        &mut std::collections::HashSet::new(),
    ) || {
        let return_type =
            type_expr_to_type_in_environment(return_type, resolved, &TypeEnvironment::default());
        type_may_carry_result_provenance(&return_type, resolved)
    }
}
