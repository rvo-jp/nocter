//! Contract validation for members owned by body-bearing interface implementations.

use super::diagnostics::{
    associated_type_bound_not_satisfied_diagnostic, associated_type_extra_diagnostic,
    associated_type_missing_diagnostic, duplicate_associated_type_binding_diagnostic,
    duplicate_interface_impl_method_diagnostic, interface_impl_extra_method_diagnostic,
    interface_method_missing_diagnostic, interface_method_signature_mismatch_diagnostic,
};
use super::interfaces::{
    method_impl_target_substitutions, method_shape, method_shape_label,
    result_provenance_contract_is_compatible, type_symbol_generic_substitutions,
};
use super::model::Type;
use super::type_expr::type_expr_to_type_with_substitutions;
use crate::ast::ImplDecl;
use crate::diagnostics::Diagnostic;
use crate::resolve::{InterfaceConformance, ResolveOutput, TypeSymbol};
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

pub(super) fn check_interface_impl_members(
    sources: &SourceMap,
    impl_: &ImplDecl,
    interface_symbol: &TypeSymbol,
    target_symbol: &TypeSymbol,
    interface_type: &Type,
    self_type: &Type,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(conformance) = target_symbol
        .interface_conformances
        .iter()
        .find(|conformance| conformance.declaration_span == impl_.span)
    else {
        return;
    };
    report_extra_and_duplicate_methods(
        sources,
        conformance,
        interface_symbol,
        target_symbol,
        diagnostics,
    );
    report_associated_type_bindings(
        sources,
        impl_,
        conformance,
        interface_symbol,
        target_symbol,
        interface_type,
        self_type,
        resolved,
        diagnostics,
    );

    let interface_substitutions =
        type_symbol_generic_substitutions(interface_symbol, interface_type);
    for required in &interface_symbol.methods {
        let actual = conformance
            .methods
            .iter()
            .find(|method| method.name == required.name);
        let Some(actual) = actual else {
            if !required.has_default_body {
                diagnostics.push(interface_method_missing_diagnostic(
                    sources,
                    impl_,
                    interface_symbol,
                    target_symbol,
                    required,
                ));
            }
            continue;
        };

        let actual_substitutions = method_impl_target_substitutions(actual, self_type, resolved);
        let associated_types = conformance
            .associated_types
            .iter()
            .map(|binding| {
                (
                    binding.name.clone(),
                    type_expr_to_type_with_substitutions(
                        &binding.value,
                        resolved,
                        Some(self_type),
                        &actual_substitutions,
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        let expected = method_shape(
            required,
            resolved,
            self_type,
            &interface_substitutions,
            &associated_types,
        );
        let found = method_shape(
            actual,
            resolved,
            self_type,
            &actual_substitutions,
            &associated_types,
        );
        if expected.has_unknown_or_unresolved() || found.has_unknown_or_unresolved() {
            continue;
        }
        if expected != found
            || !result_provenance_contract_is_compatible(required, actual, &expected, resolved)
        {
            diagnostics.push(interface_method_signature_mismatch_diagnostic(
                sources,
                interface_symbol,
                target_symbol,
                required,
                actual,
                method_shape_label(
                    required,
                    resolved,
                    self_type,
                    &interface_substitutions,
                    &associated_types,
                ),
                method_shape_label(
                    actual,
                    resolved,
                    self_type,
                    &actual_substitutions,
                    &associated_types,
                ),
            ));
        }
    }
}

fn report_associated_type_bindings(
    sources: &SourceMap,
    impl_: &ImplDecl,
    conformance: &InterfaceConformance,
    interface_symbol: &TypeSymbol,
    target_symbol: &TypeSymbol,
    interface_type: &Type,
    self_type: &Type,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = HashMap::<&str, ByteSpan>::new();
    for binding in &conformance.associated_types {
        if let Some(first_span) = seen.insert(&binding.name, binding.name_span) {
            diagnostics.push(duplicate_associated_type_binding_diagnostic(
                sources,
                target_symbol,
                binding,
                first_span,
            ));
            continue;
        }
        if !interface_symbol
            .associated_types
            .iter()
            .any(|required| required.name == binding.name)
        {
            diagnostics.push(associated_type_extra_diagnostic(
                sources,
                interface_symbol,
                target_symbol,
                binding,
            ));
        }
    }
    for required in &interface_symbol.associated_types {
        let binding = conformance
            .associated_types
            .iter()
            .find(|binding| binding.name == required.name);
        let Some(binding) = binding else {
            diagnostics.push(associated_type_missing_diagnostic(
                sources,
                impl_,
                interface_symbol,
                target_symbol,
                required,
            ));
            continue;
        };

        let impl_substitutions = conformance
            .generic_parameters
            .iter()
            .map(|name| (name.clone(), Type::Parameter(name.clone())))
            .collect::<HashMap<_, _>>();
        let actual = type_expr_to_type_with_substitutions(
            &binding.value,
            resolved,
            Some(self_type),
            &impl_substitutions,
        );
        let interface_substitutions =
            type_symbol_generic_substitutions(interface_symbol, interface_type);
        for authored_bound in required.requirements.type_bounds() {
            let bound = type_expr_to_type_with_substitutions(
                authored_bound,
                resolved,
                Some(self_type),
                &interface_substitutions,
            );
            if actual.is_unknown_or_unresolved()
                || bound.is_unknown_or_unresolved()
                || associated_binding_satisfies_bound(
                    &actual,
                    &bound,
                    conformance,
                    resolved,
                    &impl_substitutions,
                )
            {
                continue;
            }
            diagnostics.push(associated_type_bound_not_satisfied_diagnostic(
                sources,
                interface_symbol,
                binding,
                &actual,
                &bound,
                authored_bound.span(),
            ));
        }
    }
}

fn associated_binding_satisfies_bound(
    actual: &Type,
    bound: &Type,
    conformance: &InterfaceConformance,
    resolved: &ResolveOutput,
    substitutions: &HashMap<String, Type>,
) -> bool {
    if super::conformance::type_satisfies_interface_bound(actual, bound, resolved) {
        return true;
    }
    let Type::Parameter(name) = actual else {
        return false;
    };
    let Some((_, requirements)) = conformance
        .generic_parameters
        .iter()
        .zip(&conformance.generic_parameter_requirements)
        .find(|(parameter, _)| *parameter == name)
    else {
        return false;
    };
    requirements.type_bounds().any(|authored| {
        let declared =
            type_expr_to_type_with_substitutions(authored, resolved, None, substitutions);
        declared == *bound
            || super::conformance::type_satisfies_interface_bound(&declared, bound, resolved)
    })
}

fn report_extra_and_duplicate_methods(
    sources: &SourceMap,
    conformance: &InterfaceConformance,
    interface_symbol: &TypeSymbol,
    target_symbol: &TypeSymbol,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = HashMap::<&str, ByteSpan>::new();
    for actual in &conformance.methods {
        if let Some(first_span) = seen.insert(&actual.name, actual.name_span) {
            diagnostics.push(duplicate_interface_impl_method_diagnostic(
                sources,
                interface_symbol,
                target_symbol,
                actual,
                first_span,
            ));
            continue;
        }
        if !interface_symbol
            .methods
            .iter()
            .any(|required| required.name == actual.name)
        {
            diagnostics.push(interface_impl_extra_method_diagnostic(
                sources,
                interface_symbol,
                target_symbol,
                actual,
            ));
        }
    }
}
