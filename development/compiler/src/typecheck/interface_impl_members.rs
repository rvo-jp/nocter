//! Contract validation for members owned by body-bearing interface implementations.

use super::diagnostics::{
    duplicate_interface_impl_method_diagnostic, interface_impl_extra_method_diagnostic,
    interface_method_missing_diagnostic, interface_method_signature_mismatch_diagnostic,
};
use super::interfaces::{
    method_impl_target_substitutions, method_shape, method_shape_label,
    result_provenance_contract_is_compatible, type_symbol_generic_substitutions,
};
use super::model::Type;
use super::provenance::type_may_carry_result_provenance;
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
        let expected = method_shape(required, resolved, self_type, &interface_substitutions);
        let found = method_shape(actual, resolved, self_type, &actual_substitutions);
        if expected.has_unknown_or_unresolved() || found.has_unknown_or_unresolved() {
            continue;
        }
        let allocation_contract_is_compatible =
            required.signature.result_may_allocate || !actual.signature.result_may_allocate;
        if expected != found
            || !allocation_contract_is_compatible
            || !result_provenance_contract_is_compatible(
                required,
                actual,
                type_may_carry_result_provenance(found.return_type(), resolved),
            )
        {
            diagnostics.push(interface_method_signature_mismatch_diagnostic(
                sources,
                interface_symbol,
                target_symbol,
                required,
                actual,
                method_shape_label(required, resolved, self_type, &interface_substitutions),
                method_shape_label(actual, resolved, self_type, &actual_substitutions),
            ));
        }
    }
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
