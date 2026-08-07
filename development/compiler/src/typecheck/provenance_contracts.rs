use super::diagnostics::{
    independent_result_contract_diagnostic, invalid_provenance_origin_diagnostic,
    missing_external_result_contract_diagnostic, result_contract_violation_diagnostic,
};
use super::environments::{
    environment_for_function, environment_for_interface_method, environment_for_literal,
    environment_for_method,
};
use super::model::TypeEnvironment;
use super::provenance::{
    CallableProvenanceSummaries, InputId, ValueProvenance, provenance_satisfies_contract,
    result_provenance_contract, type_may_carry_result_provenance,
};
use super::returns::{borrow_return_provenance_for_callable_body, type_expr_contains_borrow_like};
use super::type_expr::type_expr_to_type_in_environment;
use crate::ast::{
    AstFile, ImplDecl, ImplMember, Item, MethodDecl, Parameter, ResultProvenanceClause, TypeExpr,
    Visibility,
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
                        super::returns::function_summary_key(function),
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
                } else if function.visibility == Visibility::Public {
                    check_public_body_without_contract(
                        sources,
                        super::returns::function_summary_key(function),
                        &function.body,
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
            Item::Interface(interface) => {
                for method in &interface.methods {
                    let environment = environment_for_interface_method(method, resolved, interface);
                    check_clause(
                        sources,
                        method.result_provenance.as_ref(),
                        Some(method),
                        &method.parameters.parameters,
                        &method.return_type,
                        Some(&environment),
                        resolved,
                        diagnostics,
                    );
                    if let (Some(body), Some(clause)) =
                        (method.body.as_ref(), method.result_provenance.as_ref())
                    {
                        check_body_contract(
                            sources,
                            method.name_span,
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
                    } else if let Some(body) = &method.body {
                        check_public_body_without_contract(
                            sources,
                            method.name_span,
                            body,
                            &method.parameters.parameters,
                            &method.return_type,
                            resolved,
                            &environment,
                            summaries,
                            diagnostics,
                        );
                    }
                }
            }
            Item::Impl(impl_) => {
                check_impl_methods(sources, impl_, resolved, summaries, diagnostics)
            }
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
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
                            super::returns::function_summary_key(function),
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
                    } else if function.visibility == Visibility::Public {
                        check_public_body_without_contract(
                            sources,
                            super::returns::function_summary_key(function),
                            &function.body,
                            &function.parameters.parameters,
                            &function.return_type,
                            resolved,
                            &environment,
                            summaries,
                            diagnostics,
                        );
                    }
                }
                for (_, literal) in construct.literals() {
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
                            literal.span,
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
            method.name_span,
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
    declaration_span: crate::source::ByteSpan,
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
    let actual = trusted_result_provenance(declaration_span, parameters, &return_type, resolved)
        .or_else(|| {
            borrow_return_provenance_for_callable_body(
                body,
                &return_type,
                resolved,
                environment,
                summaries,
            )
        });
    if actual.as_ref().is_some_and(|actual| {
        !provenance_satisfies_contract(actual, &contract, &return_type, resolved)
    }) {
        diagnostics.push(result_contract_violation_diagnostic(
            sources, body.span, clause,
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn check_public_body_without_contract(
    sources: &SourceMap,
    declaration_span: crate::source::ByteSpan,
    body: &crate::ast::Block,
    parameters: &[Parameter],
    return_type: &TypeExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let return_type_value = type_expr_to_type_in_environment(return_type, resolved, environment);
    if !type_may_carry_result_provenance(&return_type_value, resolved) {
        return;
    }
    let actual =
        trusted_result_provenance(declaration_span, parameters, &return_type_value, resolved)
            .or_else(|| {
                borrow_return_provenance_for_callable_body(
                    body,
                    &return_type_value,
                    resolved,
                    environment,
                    summaries,
                )
            });
    let no_external_origins = ValueProvenance::Independent;
    if actual.as_ref().is_some_and(|actual| {
        !provenance_satisfies_contract(actual, &no_external_origins, &return_type_value, resolved)
    }) {
        diagnostics.push(missing_external_result_contract_diagnostic(
            sources,
            return_type.span(),
        ));
    }
}

fn trusted_result_provenance(
    declaration_span: crate::source::ByteSpan,
    parameters: &[Parameter],
    return_type: &super::model::Type,
    resolved: &ResolveOutput,
) -> Option<ValueProvenance> {
    let crate::semantics::TrustedDeclarationRole::AllocationOperation { source, .. } =
        resolved.trusted_declarations.role(declaration_span)?
    else {
        return None;
    };
    let provenance = match source {
        crate::semantics::AllocationSource::CurrentContext => {
            ValueProvenance::current_allocation_context()
        }
        crate::semantics::AllocationSource::Input(index) => {
            let parameter = parameters.get(index)?;
            ValueProvenance::input(InputId::declared_at(parameter.name_span))
        }
    }
    .allocated();
    Some(match return_type {
        super::model::Type::Fallible { .. } => ValueProvenance::Fallible {
            success: Some(Box::new(provenance)),
            error: Some(Box::new(ValueProvenance::Independent)),
        },
        _ => provenance,
    })
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
