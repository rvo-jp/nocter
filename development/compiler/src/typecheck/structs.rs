use super::diagnostics::{
    member_target_type_mismatch_diagnostic, struct_field_unknown_diagnostic,
    struct_literal_duplicate_field_diagnostic, struct_literal_field_type_mismatch_diagnostic,
    struct_literal_inaccessible_field_diagnostic,
    struct_literal_inaccessible_missing_field_diagnostic, struct_literal_missing_field_diagnostic,
    struct_literal_target_type_mismatch_diagnostic, struct_literal_unknown_field_diagnostic,
    structural_construction_inaccessible_diagnostic,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::operations::is_expression_assignable;
use super::type_expr::{type_expr_to_type_in_environment, type_expr_to_type_with_substitutions};
use super::visibility::member_visibility_is_accessible;
use crate::ast::{MemberExpr, StructLiteralExpr, StructLiteralField};
use crate::diagnostics::Diagnostic;
use crate::resolve::{
    ConstructionEntryKind, ResolveOutput, StructFieldSignature, TypeSymbol, TypeSymbolKind,
};
use crate::source::SourceMap;
use std::collections::HashMap;

pub(super) fn struct_member_type(
    member: &MemberExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<Type> {
    let target_type = expression_type(&member.object, resolved, environment);
    let struct_symbol = struct_type_symbol_for_type(&target_type, resolved)?;
    Some(
        struct_field_for_member(member, struct_symbol)
            .filter(|field| struct_field_is_accessible(field, member.member_span.source, resolved))
            .map(|field| {
                struct_field_type_for_owner(
                    field,
                    struct_symbol,
                    &target_type,
                    resolved,
                    environment,
                )
            })
            .unwrap_or(Type::Unknown),
    )
}

pub(super) fn resolved_struct_field_for_member<'a>(
    member: &MemberExpr,
    resolved: &'a ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<(&'a TypeSymbol, &'a StructFieldSignature)> {
    let target_type = expression_type(&member.object, resolved, environment);
    let struct_symbol = struct_type_symbol_for_type(&target_type, resolved)?;
    let field = struct_field_for_member(member, struct_symbol)?;
    if !struct_field_is_accessible(field, member.member_span.source, resolved) {
        return None;
    }
    Some((struct_symbol, field))
}

pub(super) fn resolved_struct_field_for_literal_field<'a>(
    literal: &StructLiteralExpr,
    field: &StructLiteralField,
    resolved: &'a ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<(&'a TypeSymbol, &'a StructFieldSignature)> {
    let target_type = type_expr_to_type_in_environment(&literal.ty, resolved, environment);
    let struct_symbol = struct_type_symbol_for_type(&target_type, resolved)?;
    let field = struct_field_for_literal_field(field, struct_symbol)?;
    if !struct_field_is_accessible(field, literal.ty.span().source, resolved) {
        return None;
    }
    Some((struct_symbol, field))
}

pub(super) fn check_struct_member_expression(
    sources: &SourceMap,
    member: &MemberExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let target_type = expression_type(&member.object, resolved, environment);
    if target_type.is_unknown_or_unresolved() || target_type == Type::Error {
        return;
    }

    let Some(struct_symbol) = struct_type_symbol_for_type(&target_type, resolved) else {
        diagnostics.push(member_target_type_mismatch_diagnostic(
            sources,
            member,
            &target_type,
        ));
        return;
    };
    let Some(field) = struct_field_for_member(member, struct_symbol) else {
        diagnostics.push(struct_field_unknown_diagnostic(
            sources,
            member,
            struct_symbol,
        ));
        return;
    };

    if !struct_field_is_accessible(field, member.member_span.source, resolved) {
        diagnostics.push(struct_literal_inaccessible_field_diagnostic(
            sources,
            member.member_span,
            struct_symbol,
            field,
        ));
    }
}

pub(super) fn struct_literal_type(
    literal: &StructLiteralExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let target_type = type_expr_to_type_in_environment(&literal.ty, resolved, environment);
    if struct_type_symbol_for_type(&target_type, resolved).is_some() {
        target_type
    } else {
        Type::Unknown
    }
}

pub(super) fn struct_literal_field_type(
    literal: &StructLiteralExpr,
    field: &StructLiteralField,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<Type> {
    let target_type = type_expr_to_type_in_environment(&literal.ty, resolved, environment);
    let struct_symbol = struct_type_symbol_for_type(&target_type, resolved)?;
    let expected_field = struct_field_for_literal_field(field, struct_symbol)?;
    if !struct_field_is_accessible(expected_field, literal.ty.span().source, resolved) {
        return None;
    }
    Some(struct_field_type_for_owner(
        expected_field,
        struct_symbol,
        &target_type,
        resolved,
        environment,
    ))
}

pub(super) fn check_struct_literal_expression(
    sources: &SourceMap,
    literal: &StructLiteralExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let target_type = type_expr_to_type_in_environment(&literal.ty, resolved, environment);
    if target_type.is_unknown_or_unresolved() {
        return;
    }

    let Some(struct_symbol) = struct_type_symbol_for_type(&target_type, resolved) else {
        diagnostics.push(struct_literal_target_type_mismatch_diagnostic(
            sources,
            literal,
            &target_type,
            resolved,
        ));
        return;
    };
    let structural_is_hidden = struct_symbol
        .construction
        .entries
        .iter()
        .find(|entry| entry.kind == ConstructionEntryKind::Structural)
        .is_some_and(|entry| !entry.is_accessible);
    let is_defining_source = struct_symbol
        .construction
        .declaration_span
        .is_some_and(|span| span.source == literal.ty.span().source);
    if structural_is_hidden && !is_defining_source {
        diagnostics.push(structural_construction_inaccessible_diagnostic(
            sources,
            literal,
            struct_symbol,
        ));
        return;
    }

    let mut seen: HashMap<String, &StructLiteralField> = HashMap::new();
    for field in &literal.fields {
        if let Some(first) = seen.get(&field.name) {
            diagnostics.push(struct_literal_duplicate_field_diagnostic(
                sources,
                field,
                first,
                struct_symbol,
            ));
            continue;
        }
        seen.insert(field.name.clone(), field);

        let Some(expected_field) = struct_field_for_literal_field(field, struct_symbol) else {
            diagnostics.push(struct_literal_unknown_field_diagnostic(
                sources,
                field,
                struct_symbol,
            ));
            continue;
        };

        if !struct_field_is_accessible(expected_field, field.name_span.source, resolved) {
            diagnostics.push(struct_literal_inaccessible_field_diagnostic(
                sources,
                field.name_span,
                struct_symbol,
                expected_field,
            ));
            continue;
        }

        let expected = struct_field_type_for_owner(
            expected_field,
            struct_symbol,
            &target_type,
            resolved,
            environment,
        );
        let actual = expression_type(&field.value, resolved, environment);
        if expected.is_unknown_or_unresolved() || actual.is_unknown_or_unresolved() {
            continue;
        }

        if !is_expression_assignable(&expected, &field.value, resolved, environment) {
            diagnostics.push(struct_literal_field_type_mismatch_diagnostic(
                sources,
                field,
                expected_field,
                &expected,
                &actual,
                struct_symbol,
            ));
        }
    }

    for expected_field in &struct_symbol.fields {
        if seen.contains_key(&expected_field.name) {
            continue;
        }

        if struct_field_is_accessible(expected_field, literal.ty.span().source, resolved) {
            diagnostics.push(struct_literal_missing_field_diagnostic(
                sources,
                literal,
                struct_symbol,
                expected_field,
            ));
        } else {
            diagnostics.push(struct_literal_inaccessible_missing_field_diagnostic(
                sources,
                literal,
                struct_symbol,
                expected_field,
            ));
        }
    }
}

fn struct_type_symbol_for_type<'a>(
    ty: &Type,
    resolved: &'a ResolveOutput,
) -> Option<&'a TypeSymbol> {
    let owner_type = struct_owner_type(ty);
    let canonical_name = match &owner_type {
        Type::Named(canonical_name) => canonical_name.as_str(),
        Type::Generic { name, .. } => name.as_str(),
        _ => return None,
    };

    resolved
        .type_symbol_by_canonical_name(canonical_name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Struct)
}

fn struct_owner_type(ty: &Type) -> Type {
    match ty {
        Type::Borrow { inner, .. } => inner.as_ref().clone(),
        _ => ty.clone(),
    }
}

fn struct_field_type_for_owner(
    field: &StructFieldSignature,
    struct_symbol: &TypeSymbol,
    owner_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let substitutions = generic_substitutions_for_owner(struct_symbol, owner_type, resolved);
    type_expr_to_type_with_substitutions(
        &field.ty,
        resolved,
        environment.self_type(),
        &substitutions,
    )
}

fn generic_substitutions_for_owner(
    struct_symbol: &TypeSymbol,
    owner_type: &Type,
    resolved: &ResolveOutput,
) -> HashMap<String, Type> {
    let owner_type = struct_owner_type(owner_type);
    let Type::Generic { name, arguments } = owner_type else {
        return HashMap::new();
    };
    if name != struct_symbol.canonical_name.as_str()
        || arguments.len() != struct_symbol.generic_parameters.len()
    {
        return HashMap::new();
    }

    struct_symbol
        .generic_parameters
        .iter()
        .cloned()
        .zip(
            arguments
                .into_iter()
                .map(|argument| normalize_unresolved_type_parameters(argument, resolved)),
        )
        .collect()
}

fn normalize_unresolved_type_parameters(ty: Type, resolved: &ResolveOutput) -> Type {
    match ty {
        Type::Named(name)
            if is_plain_display_type_parameter_name(&name)
                && resolved.type_symbol_by_canonical_name(&name).is_none()
                && resolved.type_symbol_by_reference_name(&name).is_none() =>
        {
            Type::Parameter(name)
        }
        Type::Generic { name, arguments } => Type::Generic {
            name,
            arguments: arguments
                .into_iter()
                .map(|argument| normalize_unresolved_type_parameters(argument, resolved))
                .collect(),
        },
        Type::Pointer(inner) => Type::Pointer(Box::new(normalize_unresolved_type_parameters(
            *inner, resolved,
        ))),
        Type::Borrow {
            is_readwrite,
            inner,
        } => Type::Borrow {
            is_readwrite,
            inner: Box::new(normalize_unresolved_type_parameters(*inner, resolved)),
        },
        Type::Optional(inner) => Type::Optional(Box::new(normalize_unresolved_type_parameters(
            *inner, resolved,
        ))),
        Type::Fallible { success, error } => Type::Fallible {
            success: Box::new(normalize_unresolved_type_parameters(*success, resolved)),
            error: Box::new(normalize_unresolved_type_parameters(*error, resolved)),
        },
        Type::View {
            is_readwrite,
            element,
        } => Type::View {
            is_readwrite,
            element: Box::new(normalize_unresolved_type_parameters(*element, resolved)),
        },
        Type::ArrayData { element } => Type::ArrayData {
            element: Box::new(normalize_unresolved_type_parameters(*element, resolved)),
        },
        Type::Array { element, length } => Type::Array {
            element: Box::new(normalize_unresolved_type_parameters(*element, resolved)),
            length,
        },
        _ => ty,
    }
}

fn is_plain_display_type_parameter_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn struct_field_for_member<'a>(
    member: &MemberExpr,
    struct_symbol: &'a TypeSymbol,
) -> Option<&'a StructFieldSignature> {
    struct_symbol
        .fields
        .iter()
        .find(|field| field.name == member.member)
}

fn struct_field_for_literal_field<'a>(
    field: &StructLiteralField,
    struct_symbol: &'a TypeSymbol,
) -> Option<&'a StructFieldSignature> {
    struct_symbol
        .fields
        .iter()
        .find(|expected| expected.name == field.name)
}

fn struct_field_is_accessible(
    field: &StructFieldSignature,
    use_source: crate::source::SourceId,
    resolved: &ResolveOutput,
) -> bool {
    field.is_accessible
        && member_visibility_is_accessible(field.visibility, field.name_span, use_source, resolved)
}
