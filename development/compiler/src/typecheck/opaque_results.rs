use super::associated_types::normalize_projection_for_interface;
use super::conformance::type_satisfies_interface_bound;
use super::environments::{
    environment_for_function, environment_for_interface_method, environment_for_method,
};
use super::model::TypeEnvironment;
use super::type_expr::type_expr_to_type_in_environment;
use crate::ast::{AstFile, ConformanceMember, Item, MethodOwnerDecl, TypeExpr, visit_type_exprs};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::resolve::{ResolveOutput, TypeSymbolKind};
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

pub(super) fn check_opaque_results(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                reject_opaque_types(
                    sources,
                    function
                        .parameters
                        .parameters
                        .iter()
                        .map(|parameter| &parameter.ty),
                    diagnostics,
                );
                let environment = environment_for_function(function, resolved);
                check_callable_return(
                    sources,
                    &function.return_type,
                    function.body.is_some(),
                    resolved,
                    &environment,
                    true,
                    diagnostics,
                );
            }
            Item::Primitive(primitive) => {
                reject_opaque_types(
                    sources,
                    primitive
                        .parameters
                        .parameters
                        .iter()
                        .map(|parameter| &parameter.ty)
                        .chain(std::iter::once(&primitive.return_type)),
                    diagnostics,
                );
            }
            Item::TypeAlias(alias) => {
                reject_opaque_types(sources, std::iter::once(&alias.target), diagnostics)
            }
            Item::Struct(struct_) => reject_opaque_types(
                sources,
                struct_.fields.iter().map(|field| &field.ty),
                diagnostics,
            ),
            Item::Enum(enum_) => reject_opaque_types(
                sources,
                enum_
                    .variants
                    .iter()
                    .flat_map(|variant| variant.payload.iter().map(|payload| &payload.ty)),
                diagnostics,
            ),
            Item::Interface(interface) => {
                for method in &interface.methods {
                    reject_opaque_types(
                        sources,
                        method
                            .parameters
                            .parameters
                            .iter()
                            .map(|parameter| &parameter.ty),
                        diagnostics,
                    );
                    let environment = environment_for_interface_method(method, resolved, interface);
                    check_callable_return(
                        sources,
                        &method.return_type,
                        method.body.is_some(),
                        resolved,
                        &environment,
                        method.body.is_some(),
                        diagnostics,
                    );
                }
            }
            Item::Instance(instance) => {
                check_method_owner(sources, instance, resolved, true, diagnostics)
            }
            Item::Conformance(conformance) => {
                for member in &conformance.members {
                    match member {
                        ConformanceMember::AssociatedType(binding) => reject_opaque_types(
                            sources,
                            std::iter::once(&binding.value),
                            diagnostics,
                        ),
                        ConformanceMember::Method(method) => {
                            reject_opaque_types(
                                sources,
                                method
                                    .parameters
                                    .parameters
                                    .iter()
                                    .map(|parameter| &parameter.ty)
                                    .chain(std::iter::once(&method.return_type)),
                                diagnostics,
                            );
                        }
                    }
                }
            }
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    reject_opaque_types(
                        sources,
                        function
                            .parameters
                            .parameters
                            .iter()
                            .map(|parameter| &parameter.ty),
                        diagnostics,
                    );
                    let environment = environment_for_function(function, resolved);
                    check_callable_return(
                        sources,
                        &function.return_type,
                        function.body.is_some(),
                        resolved,
                        &environment,
                        true,
                        diagnostics,
                    );
                }
                for (_, literal) in construct.literals() {
                    reject_opaque_types(
                        sources,
                        literal
                            .parameters
                            .parameters
                            .iter()
                            .map(|parameter| &parameter.ty)
                            .chain(std::iter::once(&literal.return_type)),
                        diagnostics,
                    );
                }
            }
            Item::Import(_) | Item::FromImport(_) | Item::Test(_) | Item::Destruct(_) => {}
        }
    }
}

fn check_method_owner(
    sources: &SourceMap,
    owner: &dyn MethodOwnerDecl,
    resolved: &ResolveOutput,
    allow_result: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for method in owner.callables() {
        reject_opaque_types(
            sources,
            method
                .parameters
                .parameters
                .iter()
                .map(|parameter| &parameter.ty),
            diagnostics,
        );
        let environment = environment_for_method(method, resolved, owner);
        check_callable_return(
            sources,
            &method.return_type,
            method.body.is_some(),
            resolved,
            &environment,
            allow_result,
            diagnostics,
        );
    }
}

fn check_callable_return(
    sources: &SourceMap,
    return_type: &TypeExpr,
    has_body: bool,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    allow_result: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(opaque) = opaque_result_payload(return_type) else {
        reject_opaque_types(sources, std::iter::once(return_type), diagnostics);
        return;
    };
    if !allow_result || !has_body {
        diagnostics.push(unsupported_position_diagnostic(sources, opaque.some_span));
        return;
    }
    validate_contract(sources, opaque, resolved, environment, diagnostics);
}

fn validate_contract(
    sources: &SourceMap,
    opaque: &crate::ast::OpaqueType,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let interface_type = type_expr_to_type_in_environment(&opaque.interface, resolved, environment);
    let Some(interface_name) = interface_type.nominal_name() else {
        diagnostics.push(contract_diagnostic(
            sources,
            opaque.interface.span(),
            "opaque results require one nominal interface",
            "replace this contract with an accessible interface type",
        ));
        return;
    };
    let Some(interface) = resolved.type_symbol_by_canonical_name(interface_name) else {
        return;
    };
    if interface.kind != TypeSymbolKind::Interface {
        diagnostics.push(contract_diagnostic(
            sources,
            opaque.interface.span(),
            &format!(
                "`{}` is not an interface",
                crate::ast::canonical_type_expr(&opaque.interface)
            ),
            "name an interface after `some`",
        ));
        return;
    }

    let mut seen = HashMap::new();
    for binding in &opaque.associated_bindings {
        if let Some(first) = seen.insert(binding.name.as_str(), binding.name_span) {
            let mut diagnostic = contract_diagnostic(
                sources,
                binding.name_span,
                &format!("associated type `{}` is bound more than once", binding.name),
                "keep one binding for each advertised associated type",
            );
            diagnostic.notes.push(DiagnosticNote {
                message: "first binding is here".to_string(),
                span: sources.span_to_json(first).ok(),
            });
            diagnostics.push(diagnostic);
            continue;
        }
        if !interface
            .associated_types
            .iter()
            .any(|associated| associated.name == binding.name)
        {
            diagnostics.push(contract_diagnostic(
                sources,
                binding.name_span,
                &format!(
                    "interface `{}` has no associated type `{}`",
                    interface.canonical_name, binding.name
                ),
                "remove this binding or use an associated type declared by the interface",
            ));
        }
    }

    let Some(witness_expr) = opaque.witness.as_deref() else {
        return;
    };
    let witness = type_expr_to_type_in_environment(witness_expr, resolved, environment);
    if witness.is_unknown_or_unresolved() || interface_type.is_unknown_or_unresolved() {
        return;
    }
    if !type_satisfies_interface_bound(&witness, &interface_type, resolved) {
        diagnostics.push(contract_diagnostic(
            sources,
            witness_expr.span(),
            &format!(
                "opaque witness `{}` does not conform to `{}`",
                crate::ast::canonical_type_expr(witness_expr),
                crate::ast::canonical_type_expr(&opaque.interface)
            ),
            "return one concrete type with an explicit matching conformance",
        ));
        return;
    }

    for binding in &opaque.associated_bindings {
        let Some(declaration) = interface
            .associated_types
            .iter()
            .find(|associated| associated.name == binding.name)
        else {
            continue;
        };
        let actual = normalize_projection_for_interface(
            witness.clone(),
            &interface.canonical_name,
            declaration.name_span,
            &binding.name,
            resolved,
        );
        let expected = type_expr_to_type_in_environment(&binding.value, resolved, environment);
        if !actual.is_unknown_or_unresolved() && actual != expected {
            diagnostics.push(contract_diagnostic(
                sources,
                binding.value.span(),
                &format!(
                    "opaque witness binds `{}` to `{}`, not `{}`",
                    binding.name,
                    actual.display(),
                    expected.display()
                ),
                "publish the witness's associated type or return a different witness",
            ));
        }
    }
}

fn opaque_result_payload(ty: &TypeExpr) -> Option<&crate::ast::OpaqueType> {
    match ty {
        TypeExpr::Opaque(opaque) => Some(opaque),
        TypeExpr::Optional(optional) => opaque_result_payload(&optional.inner),
        TypeExpr::Fallible(fallible) => opaque_result_payload(&fallible.success),
        _ => None,
    }
}

fn reject_opaque_types<'a>(
    sources: &SourceMap,
    types: impl IntoIterator<Item = &'a TypeExpr>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for ty in types {
        visit_type_exprs(ty, &mut |ty| {
            if let TypeExpr::Opaque(opaque) = ty {
                diagnostics.push(unsupported_position_diagnostic(sources, opaque.some_span));
            }
        });
    }
}

fn unsupported_position_diagnostic(sources: &SourceMap, span: ByteSpan) -> Diagnostic {
    contract_diagnostic(
        sources,
        span,
        "opaque types are supported only as body-bearing callable results",
        "use a named concrete type in parameters, fields, aliases, and bodyless contracts",
    )
}

fn contract_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    message: &str,
    help: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0459", message);
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(help.to_string());
    diagnostic
}
