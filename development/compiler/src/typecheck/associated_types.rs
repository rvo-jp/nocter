//! Resolution and normalization for interface-owned associated types.

use super::conformance::implemented_interface_conformances;
use super::model::Type;
use super::type_expr::{infer_type_expr_substitutions, type_expr_to_type_with_substitutions};
use crate::resolve::ResolveOutput;
use crate::{
    ast::{AstFile, Item},
    diagnostics::{Diagnostic, DiagnosticNote},
    source::{ByteSpan, SourceMap},
};
use std::collections::{HashMap, HashSet};

pub(super) fn check_declarations(
    sources: &SourceMap,
    ast: &AstFile,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for interface in ast.items.iter().filter_map(|item| match item {
        Item::Interface(interface) => Some(interface),
        _ => None,
    }) {
        let mut seen = HashMap::<&str, ByteSpan>::new();
        for associated in &interface.associated_types {
            let Some(first_span) = seen.insert(&associated.name, associated.name_span) else {
                continue;
            };
            let mut diagnostic = Diagnostic::error(
                "E0432",
                format!(
                    "interface `{}` declares associated type `{}` more than once",
                    interface.name, associated.name
                ),
            );
            diagnostic.primary_span = sources
                .span_to_json(associated.name_span)
                .ok()
                .map(Box::new);
            if let Ok(span) = sources.span_to_json(first_span) {
                diagnostic.notes.push(DiagnosticNote {
                    message: "first declaration is here".to_string(),
                    span: Some(span),
                });
            }
            diagnostic.help =
                Some("keep one declaration for each interface associated type".to_string());
            diagnostics.push(diagnostic);
        }
    }
}

pub(super) fn normalize_projection(base: Type, member: &str, resolved: &ResolveOutput) -> Type {
    let Some((conformance, _)) = concrete_projection_contract(&base, member, resolved) else {
        return Type::Projection {
            base: Box::new(base),
            member: member.to_string(),
        };
    };
    normalize_projection_binding(base, member, conformance, resolved)
}

pub(super) fn normalize_projection_for_interface(
    base: Type,
    interface_canonical_name: &str,
    associated_declaration: ByteSpan,
    member: &str,
    resolved: &ResolveOutput,
) -> Type {
    let Some((conformance, _)) = projection_contract_for_interface(
        &base,
        interface_canonical_name,
        associated_declaration,
        member,
        resolved,
    ) else {
        return Type::Projection {
            base: Box::new(base),
            member: member.to_string(),
        };
    };
    normalize_projection_binding(base, member, conformance, resolved)
}

fn normalize_projection_binding(
    base: Type,
    member: &str,
    conformance: &crate::resolve::InterfaceConformance,
    resolved: &ResolveOutput,
) -> Type {
    let Some(binding) = conformance
        .associated_types
        .iter()
        .find(|binding| binding.name == member)
    else {
        return Type::Projection {
            base: Box::new(base),
            member: member.to_string(),
        };
    };

    let parameters = conformance
        .generic_parameters
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut substitutions = HashMap::new();
    infer_type_expr_substitutions(
        &conformance.target_ty,
        &base,
        resolved,
        None,
        &parameters,
        &mut substitutions,
    );
    type_expr_to_type_with_substitutions(&binding.value, resolved, Some(&base), &substitutions)
}

fn projection_contract_for_interface<'a>(
    base: &Type,
    interface_canonical_name: &str,
    associated_declaration: ByteSpan,
    member: &str,
    resolved: &'a ResolveOutput,
) -> Option<(
    &'a crate::resolve::InterfaceConformance,
    &'a crate::resolve::AssociatedTypeSignature,
)> {
    let interface = resolved.type_symbol_by_canonical_name(interface_canonical_name)?;
    let associated = interface.associated_types.iter().find(|associated| {
        associated.name == member && associated.name_span == associated_declaration
    })?;
    let mut candidates = implemented_interface_conformances(base, resolved)
        .into_iter()
        .filter(|(_, interface_type)| {
            interface_type.nominal_name() == Some(interface_canonical_name)
        })
        .map(|(conformance, _)| (conformance, associated));
    let candidate = candidates.next()?;
    candidates.next().is_none().then_some(candidate)
}

pub(super) fn concrete_projection_contract<'a>(
    base: &Type,
    member: &str,
    resolved: &'a ResolveOutput,
) -> Option<(
    &'a crate::resolve::InterfaceConformance,
    &'a crate::resolve::AssociatedTypeSignature,
)> {
    let mut candidates = implemented_interface_conformances(base, resolved)
        .into_iter()
        .filter_map(|(conformance, interface_type)| {
            let interface =
                resolved.type_symbol_by_canonical_name(interface_type.nominal_name()?)?;
            interface
                .associated_types
                .iter()
                .find(|associated| associated.name == member)
                .map(|associated| (conformance, associated))
        });
    let candidate = candidates.next()?;
    candidates.next().is_none().then_some(candidate)
}
