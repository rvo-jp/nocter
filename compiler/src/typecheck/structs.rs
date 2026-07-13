use super::diagnostics::{
    member_target_type_mismatch_diagnostic, struct_field_unknown_diagnostic,
    struct_literal_duplicate_field_diagnostic, struct_literal_field_type_mismatch_diagnostic,
    struct_literal_inaccessible_field_diagnostic,
    struct_literal_inaccessible_missing_field_diagnostic, struct_literal_missing_field_diagnostic,
    struct_literal_target_type_mismatch_diagnostic, struct_literal_unknown_field_diagnostic,
};
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::operations::is_expression_assignable;
use super::type_expr::type_expr_to_type_in_environment;
use crate::ast::{MemberExpr, StructLiteralExpr, StructLiteralField};
use crate::diagnostics::Diagnostic;
use crate::resolve::{ResolveOutput, StructFieldSignature, TypeSymbol, TypeSymbolKind};
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
            .map(|field| type_expr_to_type_in_environment(&field.ty, resolved, environment))
            .unwrap_or(Type::Unknown),
    )
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

    if struct_field_for_member(member, struct_symbol).is_none() {
        diagnostics.push(struct_field_unknown_diagnostic(
            sources,
            member,
            struct_symbol,
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

        if !expected_field.is_accessible {
            diagnostics.push(struct_literal_inaccessible_field_diagnostic(
                sources,
                field.name_span,
                struct_symbol,
                expected_field,
            ));
            continue;
        }

        let expected = type_expr_to_type_in_environment(&expected_field.ty, resolved, environment);
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

        if expected_field.is_accessible {
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
    let Type::Named(canonical_name) = ty else {
        return None;
    };
    let canonical_name = canonical_name
        .strip_prefix("&+")
        .or_else(|| canonical_name.strip_prefix('&'))
        .unwrap_or(canonical_name);

    resolved
        .type_symbol_by_canonical_name(canonical_name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Struct)
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
