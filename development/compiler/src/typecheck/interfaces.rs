use super::diagnostics::{
    conformance_contract_not_interface_diagnostic, conformance_target_not_nominal_diagnostic,
    duplicate_conformance_diagnostic,
};
use super::environments::generic_parameter_substitutions;
use super::model::Type;
use super::type_expr::{infer_type_expr_substitutions, type_expr_to_type_with_substitutions};
use crate::ast::{AstFile, ConformanceDecl, Item, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::resolve::{MethodSignature, ResolveOutput, TypeSymbol, TypeSymbolKind};
use crate::source::SourceMap;
use std::collections::{HashMap, HashSet};

pub(super) fn check_conformances(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = Vec::<&ConformanceDecl>::new();

    for conformance_decl in ast.items.iter().filter_map(|item| match item {
        Item::Conformance(conformance_decl) => Some(conformance_decl),
        _ => None,
    }) {
        check_conformance(sources, conformance_decl, resolved, diagnostics, &mut seen);
    }
}

fn check_conformance<'a>(
    sources: &SourceMap,
    conformance_decl: &'a ConformanceDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    seen: &mut Vec<&'a ConformanceDecl>,
) {
    let conformance_substitutions = generic_parameter_substitutions(&conformance_decl.generics);
    let Some((interface_symbol, interface_type)) = resolve_interface_symbol(
        &conformance_decl.interface_ty,
        resolved,
        diagnostics,
        sources,
        &conformance_substitutions,
    ) else {
        return;
    };
    let Some((target_symbol, target_type)) = resolve_target_symbol(
        conformance_decl,
        resolved,
        diagnostics,
        sources,
        &conformance_substitutions,
    ) else {
        return;
    };

    if let Some(first) = seen.iter().copied().find(|first| {
        crate::ast::declaration_patterns_overlap(
            &[&first.interface_ty, &first.target_ty],
            first
                .generics
                .parameters
                .iter()
                .map(|parameter| &parameter.name),
            first.requirements.as_ref(),
            &[&conformance_decl.interface_ty, &conformance_decl.target_ty],
            conformance_decl
                .generics
                .parameters
                .iter()
                .map(|parameter| &parameter.name),
            conformance_decl.requirements.as_ref(),
        )
    }) {
        diagnostics.push(duplicate_conformance_diagnostic(
            sources,
            conformance_decl,
            interface_symbol,
            target_symbol,
            first.span,
        ));
        return;
    }
    seen.push(conformance_decl);

    super::conformance_members::check_conformance_members(
        sources,
        conformance_decl,
        interface_symbol,
        target_symbol,
        &interface_type,
        &target_type,
        resolved,
        diagnostics,
    );
}

fn resolve_interface_symbol<'a>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
    substitutions: &HashMap<String, Type>,
) -> Option<(&'a TypeSymbol, Type)> {
    let actual_type = type_expr_to_type_with_substitutions(ty, resolved, None, substitutions);
    if actual_type.is_unknown_or_unresolved() {
        return None;
    }
    let symbol = symbol_for_type(&actual_type, resolved);
    if symbol.is_none_or(|symbol| symbol.kind != TypeSymbolKind::Interface) {
        diagnostics.push(conformance_contract_not_interface_diagnostic(
            sources,
            ty,
            &actual_type,
            symbol,
        ));
        return None;
    }

    symbol.map(|symbol| (symbol, actual_type))
}

fn resolve_target_symbol<'a>(
    conformance_decl: &ConformanceDecl,
    resolved: &'a ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
    substitutions: &HashMap<String, Type>,
) -> Option<(&'a TypeSymbol, Type)> {
    let actual_type = type_expr_to_type_with_substitutions(
        &conformance_decl.target_ty,
        resolved,
        None,
        substitutions,
    );
    if actual_type.is_unknown_or_unresolved() {
        return None;
    }
    let symbol = symbol_for_type(&actual_type, resolved);
    if symbol
        .is_none_or(|symbol| !matches!(symbol.kind, TypeSymbolKind::Struct | TypeSymbolKind::Enum))
    {
        diagnostics.push(conformance_target_not_nominal_diagnostic(
            sources,
            &conformance_decl.target_ty,
            &actual_type,
            symbol,
        ));
        return None;
    }

    symbol.map(|symbol| (symbol, actual_type))
}

fn symbol_for_type<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a TypeSymbol> {
    let canonical_name = ty.nominal_name()?;

    resolved.type_symbol_by_canonical_name(canonical_name)
}

pub(super) fn type_symbol_generic_substitutions(
    symbol: &TypeSymbol,
    ty: &Type,
) -> HashMap<String, Type> {
    let Type::Generic { name, arguments } = ty else {
        return HashMap::new();
    };
    if name != &symbol.canonical_name {
        return HashMap::new();
    }

    symbol
        .generic_parameters
        .iter()
        .cloned()
        .zip(arguments.iter().cloned())
        .collect()
}

pub(super) fn method_owner_target_substitutions(
    method: &MethodSignature,
    self_type: &Type,
    resolved: &ResolveOutput,
) -> HashMap<String, Type> {
    let Some(owner_target_ty) = &method.owner_target_ty else {
        return HashMap::new();
    };

    let parameters = method
        .signature
        .generic_parameters
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut substitutions = HashMap::new();
    infer_type_expr_substitutions(
        owner_target_ty,
        self_type,
        resolved,
        None,
        &parameters,
        &mut substitutions,
    );
    substitutions
}

pub(super) fn method_shape(
    method: &MethodSignature,
    resolved: &ResolveOutput,
    self_type: &Type,
    substitutions: &HashMap<String, Type>,
    associated_types: &HashMap<String, Type>,
) -> MethodShape {
    MethodShape {
        receiver: replace_self_associated_types(
            type_expr_to_type_with_substitutions(
                &method.receiver.implicit_parameter().ty,
                resolved,
                Some(self_type),
                substitutions,
            ),
            self_type,
            associated_types,
        ),
        parameters: method
            .signature
            .parameters
            .iter()
            .map(|parameter| {
                replace_self_associated_types(
                    type_expr_to_type_with_substitutions(
                        &parameter.ty,
                        resolved,
                        Some(self_type),
                        substitutions,
                    ),
                    self_type,
                    associated_types,
                )
            })
            .collect(),
        return_type: replace_self_associated_types(
            type_expr_to_type_with_substitutions(
                &method.signature.return_type,
                resolved,
                Some(self_type),
                substitutions,
            ),
            self_type,
            associated_types,
        ),
    }
}

fn replace_self_associated_types(
    ty: Type,
    self_type: &Type,
    associated_types: &HashMap<String, Type>,
) -> Type {
    match ty {
        Type::Projection { base, member } if base.as_ref() == self_type => associated_types
            .get(&member)
            .cloned()
            .unwrap_or(Type::Projection { base, member }),
        Type::ArrayData { element } => Type::ArrayData {
            element: Box::new(replace_self_associated_types(
                *element,
                self_type,
                associated_types,
            )),
        },
        Type::View {
            is_readwrite,
            element,
        } => Type::View {
            is_readwrite,
            element: Box::new(replace_self_associated_types(
                *element,
                self_type,
                associated_types,
            )),
        },
        Type::Array { element, length } => Type::Array {
            element: Box::new(replace_self_associated_types(
                *element,
                self_type,
                associated_types,
            )),
            length,
        },
        Type::Pointer(inner) => Type::Pointer(Box::new(replace_self_associated_types(
            *inner,
            self_type,
            associated_types,
        ))),
        Type::Borrow {
            is_readwrite,
            inner,
        } => Type::Borrow {
            is_readwrite,
            inner: Box::new(replace_self_associated_types(
                *inner,
                self_type,
                associated_types,
            )),
        },
        Type::Optional(inner) => Type::Optional(Box::new(replace_self_associated_types(
            *inner,
            self_type,
            associated_types,
        ))),
        Type::Fallible { success, error } => Type::Fallible {
            success: Box::new(replace_self_associated_types(
                *success,
                self_type,
                associated_types,
            )),
            error: Box::new(replace_self_associated_types(
                *error,
                self_type,
                associated_types,
            )),
        },
        Type::Generic { name, arguments } => Type::Generic {
            name,
            arguments: arguments
                .into_iter()
                .map(|argument| {
                    replace_self_associated_types(argument, self_type, associated_types)
                })
                .collect(),
        },
        other => other,
    }
}

pub(super) fn method_shape_label(
    method: &MethodSignature,
    resolved: &ResolveOutput,
    self_type: &Type,
    substitutions: &HashMap<String, Type>,
    associated_types: &HashMap<String, Type>,
) -> String {
    let shape = method_shape(method, resolved, self_type, substitutions, associated_types);
    let parameters = shape
        .parameters
        .iter()
        .map(Type::display)
        .collect::<Vec<_>>()
        .join(", ");
    let mut label = format!(
        "method {}.{}({}): {}",
        method_receiver_shape_label(&shape.receiver),
        method.name,
        parameters,
        shape.return_type.display()
    );
    if let Some(clause) = &method.signature.result_provenance {
        label.push_str(" from ");
        label.push_str(
            &clause
                .origins
                .iter()
                .map(|origin| origin.kind.source_label())
                .collect::<Vec<_>>()
                .join(" | "),
        );
    }
    label
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProvenanceSlot {
    Receiver,
    Parameter(usize),
    Static,
}

pub(super) fn result_provenance_contract_is_compatible(
    required: &MethodSignature,
    actual: &MethodSignature,
    required_shape: &MethodShape,
    resolved: &ResolveOutput,
) -> bool {
    let required_slots = required
        .signature
        .result_provenance
        .as_ref()
        .map(|clause| provenance_slots(required, clause))
        .unwrap_or_else(|| elided_provenance_slots(required, required_shape, resolved));
    let Some(actual_clause) = &actual.signature.result_provenance else {
        return true;
    };
    provenance_slots(actual, actual_clause)
        .into_iter()
        .all(|slot| slot == ProvenanceSlot::Static || required_slots.contains(&slot))
}

fn elided_provenance_slots(
    method: &MethodSignature,
    shape: &MethodShape,
    resolved: &ResolveOutput,
) -> Vec<ProvenanceSlot> {
    let receiver = super::provenance::InputId::declared_at(method.receiver.name_span);
    let inputs = std::iter::once(("self".to_string(), receiver, shape.receiver.clone())).chain(
        method
            .signature
            .parameters
            .iter()
            .zip(&shape.parameters)
            .map(|(parameter, ty)| {
                (
                    parameter.name.clone(),
                    super::provenance::InputId::declared_at(parameter.name_span),
                    ty.clone(),
                )
            }),
    );
    let Some(unique) =
        super::provenance::elided_typed_result_contract(inputs, &shape.return_type, resolved)
            .unique_input()
    else {
        return Vec::new();
    };
    if unique == receiver {
        return vec![ProvenanceSlot::Receiver];
    }
    method
        .signature
        .parameters
        .iter()
        .position(|parameter| {
            super::provenance::InputId::declared_at(parameter.name_span) == unique
        })
        .map(ProvenanceSlot::Parameter)
        .into_iter()
        .collect()
}

fn provenance_slots(
    method: &MethodSignature,
    clause: &crate::ast::ResultProvenanceClause,
) -> Vec<ProvenanceSlot> {
    clause
        .origins
        .iter()
        .filter_map(|origin| match &origin.kind {
            crate::ast::ResultProvenanceOriginKind::Receiver => Some(ProvenanceSlot::Receiver),
            crate::ast::ResultProvenanceOriginKind::Parameter(name) => method
                .signature
                .parameters
                .iter()
                .position(|parameter| parameter.name == *name)
                .map(ProvenanceSlot::Parameter),
            crate::ast::ResultProvenanceOriginKind::Static => Some(ProvenanceSlot::Static),
        })
        .collect()
}

fn method_receiver_shape_label(receiver: &Type) -> &'static str {
    let display = receiver.display();
    if display.starts_with("&+") {
        return "&+self";
    }
    if display.starts_with('&') {
        return "&self";
    }
    "self"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MethodShape {
    receiver: Type,
    parameters: Vec<Type>,
    return_type: Type,
}

impl MethodShape {
    pub(super) fn has_unknown_or_unresolved(&self) -> bool {
        self.receiver.is_unknown_or_unresolved()
            || self.parameters.iter().any(Type::is_unknown_or_unresolved)
            || self.return_type.is_unknown_or_unresolved()
    }
}
