use super::diagnostics::{
    ambiguous_bodyless_result_contract_diagnostic, independent_result_contract_diagnostic,
    invalid_provenance_origin_diagnostic, missing_external_result_contract_diagnostic,
    result_contract_violation_diagnostic,
};
use super::environments::{
    environment_for_function, environment_for_interface_method, environment_for_literal,
    environment_for_method,
};
use super::model::TypeEnvironment;
use super::provenance::{
    CallableProvenanceSummaries, ElidedResultContract, InputId, ResultProvenanceInputs,
    ValueProvenance, elided_declaration_result_contract, provenance_satisfies_contract,
    result_provenance_contract, type_may_carry_result_provenance,
};
use super::returns::{borrow_return_provenance_for_callable_body, type_expr_contains_borrow_like};
use super::type_expr::type_expr_to_type_in_environment;
use crate::ast::{
    AstFile, ConformanceMember, Item, MethodDecl, MethodOwnerDecl, Parameter,
    ResultProvenanceClause, TypeExpr, Visibility,
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
                    ResultProvenanceInputs::parameters(&function.parameters.parameters),
                    &function.return_type,
                    Some(&environment),
                    resolved,
                    diagnostics,
                );
                if let Some(body) = &function.body {
                    check_body_result_contract(
                        sources,
                        BodyResultContract {
                            declaration_span: super::returns::function_summary_key(
                                function, resolved,
                            ),
                            body,
                            clause: function.result_provenance.as_ref(),
                            method: None,
                            contract_inputs: ResultProvenanceInputs::parameters(
                                &function.parameters.parameters,
                            ),
                            parameters: &function.parameters.parameters,
                            return_type: &function.return_type,
                            environment: &environment,
                            externally_callable: function.visibility == Visibility::Public,
                        },
                        resolved,
                        summaries,
                        diagnostics,
                    );
                }
            }
            Item::Primitive(primitive) => check_clause(
                sources,
                primitive.result_provenance.as_ref(),
                None,
                ResultProvenanceInputs::parameters(&primitive.parameters.parameters),
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
                        ResultProvenanceInputs::parameters(&method.parameters.parameters),
                        &method.return_type,
                        Some(&environment),
                        resolved,
                        diagnostics,
                    );
                    if let Some(body) = &method.body {
                        check_body_result_contract(
                            sources,
                            BodyResultContract {
                                declaration_span: method.name_span,
                                body,
                                clause: method.result_provenance.as_ref(),
                                method: Some(method),
                                contract_inputs: ResultProvenanceInputs::parameters(
                                    &method.parameters.parameters,
                                ),
                                parameters: &method.parameters.parameters,
                                return_type: &method.return_type,
                                environment: &environment,
                                externally_callable: true,
                            },
                            resolved,
                            summaries,
                            diagnostics,
                        );
                    } else if method.result_provenance.is_none() {
                        check_bodyless_elision(
                            sources,
                            method.return_type.span(),
                            Some(method),
                            ResultProvenanceInputs::parameters(&method.parameters.parameters),
                            &method.return_type,
                            Some(&environment),
                            resolved,
                            diagnostics,
                        );
                    }
                }
            }
            Item::Instance(instance) => check_method_contracts(
                sources,
                instance,
                instance.methods.iter(),
                false,
                resolved,
                summaries,
                diagnostics,
            ),
            Item::Conformance(conformance) => check_method_contracts(
                sources,
                conformance,
                conformance
                    .members
                    .iter()
                    .filter_map(|member| match member {
                        ConformanceMember::AssociatedType(_) => None,
                        ConformanceMember::Method(method) => Some(method),
                    }),
                true,
                resolved,
                summaries,
                diagnostics,
            ),
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    let environment = environment_for_function(function, resolved);
                    check_clause(
                        sources,
                        function.result_provenance.as_ref(),
                        None,
                        ResultProvenanceInputs::parameters(&function.parameters.parameters),
                        &function.return_type,
                        Some(&environment),
                        resolved,
                        diagnostics,
                    );
                    if let Some(body) = &function.body {
                        check_body_result_contract(
                            sources,
                            BodyResultContract {
                                declaration_span: super::returns::function_summary_key(
                                    function, resolved,
                                ),
                                body,
                                clause: function.result_provenance.as_ref(),
                                method: None,
                                contract_inputs: ResultProvenanceInputs::parameters(
                                    &function.parameters.parameters,
                                ),
                                parameters: &function.parameters.parameters,
                                return_type: &function.return_type,
                                environment: &environment,
                                externally_callable: function.visibility == Visibility::Public,
                            },
                            resolved,
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
                        ResultProvenanceInputs::literal(literal),
                        &literal.return_type,
                        Some(&environment),
                        resolved,
                        diagnostics,
                    );
                    if let Some(body) = &literal.body {
                        check_body_result_contract(
                            sources,
                            BodyResultContract {
                                declaration_span: literal.span,
                                body,
                                clause: literal.result_provenance.as_ref(),
                                method: None,
                                contract_inputs: ResultProvenanceInputs::literal(literal),
                                parameters: &literal.parameters.parameters,
                                return_type: &literal.return_type,
                                environment: &environment,
                                externally_callable: literal.visibility == Visibility::Public,
                            },
                            resolved,
                            summaries,
                            diagnostics,
                        );
                    }
                }
            }
            Item::Coerce(coerce) => {
                let instance = coerce.callable_instance();
                check_method_contracts(
                    sources,
                    &instance,
                    instance.methods.iter(),
                    false,
                    resolved,
                    summaries,
                    diagnostics,
                )
            }
            _ => {}
        }
    }
}

fn check_method_contracts<'a>(
    sources: &SourceMap,
    owner: &(impl MethodOwnerDecl + ?Sized),
    methods: impl Iterator<Item = &'a MethodDecl>,
    externally_callable: bool,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for method in methods {
        check_clause(
            sources,
            method.result_provenance.as_ref(),
            Some(method),
            ResultProvenanceInputs::parameters(&method.parameters.parameters),
            &method.return_type,
            Some(&environment_for_method(method, resolved, owner)),
            resolved,
            diagnostics,
        );
        let Some(body) = &method.body else {
            continue;
        };
        let environment = environment_for_method(method, resolved, owner);
        check_body_result_contract(
            sources,
            BodyResultContract {
                declaration_span: method.name_span,
                body,
                clause: method.result_provenance.as_ref(),
                method: Some(method),
                contract_inputs: ResultProvenanceInputs::parameters(&method.parameters.parameters),
                parameters: &method.parameters.parameters,
                return_type: &method.return_type,
                environment: &environment,
                externally_callable: externally_callable || method.visibility == Visibility::Public,
            },
            resolved,
            summaries,
            diagnostics,
        );
    }
}

struct BodyResultContract<'a> {
    declaration_span: crate::source::ByteSpan,
    body: &'a crate::ast::Block,
    clause: Option<&'a ResultProvenanceClause>,
    method: Option<&'a MethodDecl>,
    contract_inputs: ResultProvenanceInputs<'a>,
    parameters: &'a [Parameter],
    return_type: &'a TypeExpr,
    environment: &'a TypeEnvironment,
    externally_callable: bool,
}

fn check_body_result_contract(
    sources: &SourceMap,
    subject: BodyResultContract<'_>,
    resolved: &ResolveOutput,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(clause) = subject.clause {
        check_body_contract(
            sources,
            subject.declaration_span,
            subject.body,
            clause,
            subject.method,
            subject.contract_inputs,
            subject.parameters,
            subject.return_type,
            resolved,
            subject.environment,
            summaries,
            diagnostics,
        );
    } else if subject.externally_callable {
        check_public_body_without_contract(
            sources,
            subject.declaration_span,
            subject.body,
            subject.method,
            subject.contract_inputs,
            subject.parameters,
            subject.return_type,
            resolved,
            subject.environment,
            summaries,
            diagnostics,
        );
    }
}

fn check_clause(
    sources: &SourceMap,
    clause: Option<&ResultProvenanceClause>,
    method: Option<&MethodDecl>,
    inputs: ResultProvenanceInputs<'_>,
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
    if let Err(errors) = result_provenance_contract(clause, method, inputs, resolved) {
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
    inputs: ResultProvenanceInputs<'_>,
    parameters: &[Parameter],
    return_type: &TypeExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Ok(contract) = result_provenance_contract(clause, method, inputs, resolved) else {
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
    method: Option<&MethodDecl>,
    inputs: ResultProvenanceInputs<'_>,
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
    let elided = elided_declaration_result_contract(method, inputs, &return_type_value, resolved);
    let allowed = elided
        .allowed_contract()
        .cloned()
        .unwrap_or(ValueProvenance::Independent);
    if actual.as_ref().is_some_and(|actual| {
        !provenance_satisfies_contract(actual, &allowed, &return_type_value, resolved)
    }) {
        let candidates = match &elided {
            ElidedResultContract::Ambiguous { labels, .. } => labels.clone(),
            ElidedResultContract::Unique { label, .. } => vec![label.clone()],
            ElidedResultContract::Independent => Vec::new(),
        };
        diagnostics.push(missing_external_result_contract_diagnostic(
            sources,
            return_type.span(),
            &candidates,
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn check_bodyless_elision(
    sources: &SourceMap,
    return_span: crate::source::ByteSpan,
    method: Option<&MethodDecl>,
    inputs: ResultProvenanceInputs<'_>,
    return_type: &TypeExpr,
    environment: Option<&TypeEnvironment>,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let return_type = environment.map_or_else(
        || type_expr_to_type_in_environment(return_type, resolved, &TypeEnvironment::default()),
        |environment| type_expr_to_type_in_environment(return_type, resolved, environment),
    );
    if let ElidedResultContract::Ambiguous { labels, .. } =
        elided_declaration_result_contract(method, inputs, &return_type, resolved)
    {
        diagnostics.push(ambiguous_bodyless_result_contract_diagnostic(
            sources,
            return_span,
            &labels,
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
