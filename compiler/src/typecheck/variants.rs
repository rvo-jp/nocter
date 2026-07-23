use super::diagnostics::{
    duplicate_pattern_conditional_arm_variant_diagnostic, duplicate_switch_arm_variant_diagnostic,
    enum_variant_payload_count_mismatch_diagnostic, enum_variant_payload_type_mismatch_diagnostic,
    enum_variant_payloadless_call_diagnostic, enum_variant_unknown_diagnostic,
    if_is_enum_mismatch_diagnostic, if_is_non_enum_diagnostic, if_is_payload_mismatch_diagnostic,
    if_is_target_type_mismatch_diagnostic, if_is_unknown_enum_diagnostic,
    if_is_unknown_variant_diagnostic, pattern_conditional_arm_enum_mismatch_diagnostic,
    pattern_conditional_arm_non_enum_diagnostic,
    pattern_conditional_arm_payload_mismatch_diagnostic,
    pattern_conditional_arm_type_mismatch_diagnostic,
    pattern_conditional_arm_unknown_enum_diagnostic,
    pattern_conditional_arm_unknown_variant_diagnostic,
    pattern_conditional_target_type_mismatch_diagnostic, switch_arm_enum_mismatch_diagnostic,
    switch_arm_non_enum_diagnostic, switch_arm_payload_mismatch_diagnostic,
    switch_arm_unknown_enum_diagnostic, switch_arm_unknown_variant_diagnostic,
    switch_target_type_mismatch_diagnostic,
};
use super::environments::environment_for_pattern_conditional_arm;
use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment};
use super::operations::is_expression_assignable;
use super::type_expr::{infer_type_expr_substitutions, type_expr_to_type_with_substitutions};
use crate::ast::{
    CallExpr, Expr, IfIsStmt, MemberExpr, PatternConditionalArm, PatternConditionalExpr, SwitchArm,
    SwitchStmt,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::{EnumVariantSignature, ResolveOutput, TypeSymbol, TypeSymbolKind};
use crate::source::SourceMap;
use std::collections::{HashMap, HashSet};

pub(super) fn check_switch_statement(
    sources: &SourceMap,
    statement: &SwitchStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let target_type = expression_type(&statement.expression, resolved, environment);
    if target_type.is_unknown_or_unresolved() {
        return;
    }

    let target_symbol = enum_type_symbol_for_type(&target_type, resolved);
    if target_symbol.is_none() {
        diagnostics.push(switch_target_type_mismatch_diagnostic(
            sources,
            statement,
            &target_type,
        ));
    }

    for arm in &statement.arms {
        check_switch_arm_pattern(sources, arm, target_symbol, resolved, diagnostics);
    }

    if let Some(target_symbol) = target_symbol {
        check_duplicate_switch_arm_variants(
            sources,
            statement,
            target_symbol,
            resolved,
            diagnostics,
        );
    }
}

pub(super) fn check_if_is_statement(
    sources: &SourceMap,
    statement: &IfIsStmt,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let target_type = expression_type(&statement.expression, resolved, environment);
    if target_type.is_unknown_or_unresolved() {
        return;
    }

    let target_symbol = enum_type_symbol_for_type(&target_type, resolved);
    if target_symbol.is_none() {
        diagnostics.push(if_is_target_type_mismatch_diagnostic(
            sources,
            statement,
            &target_type,
        ));
    }

    check_if_is_pattern(sources, statement, target_symbol, resolved, diagnostics);
}

pub(super) fn check_pattern_conditional_expression(
    sources: &SourceMap,
    expression: &PatternConditionalExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let target_type = expression_type(&expression.target, resolved, environment);
    let target_symbol = if target_type.is_unknown_or_unresolved() {
        None
    } else {
        let target_symbol = enum_type_symbol_for_type(&target_type, resolved);
        if target_symbol.is_none() {
            diagnostics.push(pattern_conditional_target_type_mismatch_diagnostic(
                sources,
                expression,
                &target_type,
            ));
        }
        target_symbol
    };

    for arm in &expression.arms {
        check_pattern_conditional_arm_pattern(sources, arm, target_symbol, resolved, diagnostics);
    }

    if let Some(target_symbol) = target_symbol {
        check_duplicate_pattern_conditional_arm_variants(
            sources,
            expression,
            target_symbol,
            resolved,
            diagnostics,
        );
    }

    let expected = pattern_conditional_expression_type(expression, resolved, environment);
    if expected.is_unknown_or_unresolved() {
        return;
    }

    for arm in &expression.arms {
        let arm_environment =
            environment_for_pattern_conditional_arm(arm, &expression.target, resolved, environment);
        if !is_expression_assignable(&expected, &arm.expression, resolved, &arm_environment) {
            let actual = expression_type(&arm.expression, resolved, &arm_environment);
            diagnostics.push(pattern_conditional_arm_type_mismatch_diagnostic(
                sources, arm, &expected, &actual,
            ));
        }
    }
}

pub(super) fn pattern_conditional_expression_type(
    expression: &PatternConditionalExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let fallback_type = expression_type(&expression.fallback, resolved, environment);
    if let Some(candidate) =
        compatible_pattern_conditional_arm_type(expression, &fallback_type, resolved, environment)
    {
        return candidate;
    }

    if !fallback_type.is_unknown_or_unresolved() {
        return fallback_type;
    }

    expression
        .arms
        .iter()
        .map(|arm| {
            let arm_environment = environment_for_pattern_conditional_arm(
                arm,
                &expression.target,
                resolved,
                environment,
            );
            expression_type(&arm.expression, resolved, &arm_environment)
        })
        .find(|ty| !ty.is_unknown_or_unresolved())
        .unwrap_or(fallback_type)
}

fn compatible_pattern_conditional_arm_type(
    expression: &PatternConditionalExpr,
    fallback_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<Type> {
    expression.arms.iter().find_map(|candidate_arm| {
        let candidate_environment = environment_for_pattern_conditional_arm(
            candidate_arm,
            &expression.target,
            resolved,
            environment,
        );
        let candidate_type =
            expression_type(&candidate_arm.expression, resolved, &candidate_environment);
        if candidate_type.is_unknown_or_unresolved() {
            return None;
        }
        if !fallback_type.is_unknown_or_unresolved()
            && !is_expression_assignable(
                &candidate_type,
                &expression.fallback,
                resolved,
                environment,
            )
        {
            return None;
        }

        let arms_fit_candidate = expression.arms.iter().all(|arm| {
            let arm_environment = environment_for_pattern_conditional_arm(
                arm,
                &expression.target,
                resolved,
                environment,
            );
            let arm_type = expression_type(&arm.expression, resolved, &arm_environment);
            arm_type.is_unknown_or_unresolved()
                || is_expression_assignable(
                    &candidate_type,
                    &arm.expression,
                    resolved,
                    &arm_environment,
                )
        });

        arms_fit_candidate.then_some(candidate_type)
    })
}

fn check_if_is_pattern(
    sources: &SourceMap,
    statement: &IfIsStmt,
    target_symbol: Option<&TypeSymbol>,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(pattern_symbol) = resolved.type_symbol_by_name(&statement.enum_name) else {
        diagnostics.push(if_is_unknown_enum_diagnostic(sources, statement));
        return;
    };

    if pattern_symbol.kind != TypeSymbolKind::Enum {
        diagnostics.push(if_is_non_enum_diagnostic(
            sources,
            statement,
            pattern_symbol,
        ));
        return;
    }

    if let Some(target_symbol) = target_symbol
        && target_symbol.canonical_name != pattern_symbol.canonical_name
    {
        diagnostics.push(if_is_enum_mismatch_diagnostic(
            sources,
            statement,
            target_symbol,
            pattern_symbol,
        ));
        return;
    }

    let Some(variant) = pattern_symbol
        .variants
        .iter()
        .find(|variant| variant.name == statement.variant_name)
    else {
        diagnostics.push(if_is_unknown_variant_diagnostic(
            sources,
            statement,
            pattern_symbol,
        ));
        return;
    };

    let provided_payload_count = usize::from(statement.payload.is_some());
    if variant.payload.len() != provided_payload_count {
        diagnostics.push(if_is_payload_mismatch_diagnostic(
            sources,
            statement,
            pattern_symbol,
            variant.payload.len(),
            provided_payload_count,
        ));
    }
}

fn check_pattern_conditional_arm_pattern(
    sources: &SourceMap,
    arm: &PatternConditionalArm,
    target_symbol: Option<&TypeSymbol>,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(pattern_symbol) = resolved.type_symbol_by_name(&arm.enum_name) else {
        diagnostics.push(pattern_conditional_arm_unknown_enum_diagnostic(
            sources, arm,
        ));
        return;
    };

    if pattern_symbol.kind != TypeSymbolKind::Enum {
        diagnostics.push(pattern_conditional_arm_non_enum_diagnostic(
            sources,
            arm,
            pattern_symbol,
        ));
        return;
    }

    if let Some(target_symbol) = target_symbol
        && target_symbol.canonical_name != pattern_symbol.canonical_name
    {
        diagnostics.push(pattern_conditional_arm_enum_mismatch_diagnostic(
            sources,
            arm,
            target_symbol,
            pattern_symbol,
        ));
        return;
    }

    let Some(variant) = pattern_symbol
        .variants
        .iter()
        .find(|variant| variant.name == arm.variant_name)
    else {
        diagnostics.push(pattern_conditional_arm_unknown_variant_diagnostic(
            sources,
            arm,
            pattern_symbol,
        ));
        return;
    };

    let provided_payload_count = usize::from(arm.payload.is_some());
    if variant.payload.len() != provided_payload_count {
        diagnostics.push(pattern_conditional_arm_payload_mismatch_diagnostic(
            sources,
            arm,
            pattern_symbol,
            variant.payload.len(),
            provided_payload_count,
        ));
    }
}

fn check_switch_arm_pattern(
    sources: &SourceMap,
    arm: &SwitchArm,
    target_symbol: Option<&TypeSymbol>,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(pattern_symbol) = resolved.type_symbol_by_name(&arm.enum_name) else {
        diagnostics.push(switch_arm_unknown_enum_diagnostic(sources, arm));
        return;
    };

    if pattern_symbol.kind != TypeSymbolKind::Enum {
        diagnostics.push(switch_arm_non_enum_diagnostic(sources, arm, pattern_symbol));
        return;
    }

    if let Some(target_symbol) = target_symbol
        && target_symbol.canonical_name != pattern_symbol.canonical_name
    {
        diagnostics.push(switch_arm_enum_mismatch_diagnostic(
            sources,
            arm,
            target_symbol,
            pattern_symbol,
        ));
        return;
    }

    let Some(variant) = pattern_symbol
        .variants
        .iter()
        .find(|variant| variant.name == arm.variant_name)
    else {
        diagnostics.push(switch_arm_unknown_variant_diagnostic(
            sources,
            arm,
            pattern_symbol,
        ));
        return;
    };

    let provided_payload_count = usize::from(arm.payload.is_some());
    if variant.payload.len() != provided_payload_count {
        diagnostics.push(switch_arm_payload_mismatch_diagnostic(
            sources,
            arm,
            pattern_symbol,
            variant.payload.len(),
            provided_payload_count,
        ));
    }
}

fn check_duplicate_switch_arm_variants(
    sources: &SourceMap,
    statement: &SwitchStmt,
    target_symbol: &TypeSymbol,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = HashMap::new();

    for arm in &statement.arms {
        if valid_switch_arm_variant(arm, target_symbol, resolved).is_none() {
            continue;
        }

        if let Some(first_span) = seen.get(arm.variant_name.as_str()).copied() {
            diagnostics.push(duplicate_switch_arm_variant_diagnostic(
                sources,
                arm,
                target_symbol,
                first_span,
            ));
        } else {
            seen.insert(arm.variant_name.as_str(), arm.variant_name_span);
        }
    }
}

fn check_duplicate_pattern_conditional_arm_variants(
    sources: &SourceMap,
    expression: &PatternConditionalExpr,
    target_symbol: &TypeSymbol,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = HashMap::new();

    for arm in &expression.arms {
        if valid_pattern_conditional_arm_variant(arm, target_symbol, resolved).is_none() {
            continue;
        }

        if let Some(first_span) = seen.get(arm.variant_name.as_str()).copied() {
            diagnostics.push(duplicate_pattern_conditional_arm_variant_diagnostic(
                sources,
                arm,
                target_symbol,
                first_span,
            ));
        } else {
            seen.insert(arm.variant_name.as_str(), arm.variant_name_span);
        }
    }
}

pub(super) fn switch_statement_covers_all_variants(
    statement: &SwitchStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let target_type = expression_type(&statement.expression, resolved, environment);
    let Some(target_symbol) = enum_type_symbol_for_type(&target_type, resolved) else {
        return false;
    };

    let covered = valid_switch_arm_variants(&statement.arms, target_symbol, resolved);
    target_symbol
        .variants
        .iter()
        .all(|variant| covered.contains(variant.name.as_str()))
}

fn valid_switch_arm_variants<'a>(
    arms: &[SwitchArm],
    target_symbol: &'a TypeSymbol,
    resolved: &ResolveOutput,
) -> HashSet<&'a str> {
    arms.iter()
        .filter_map(|arm| valid_switch_arm_variant(arm, target_symbol, resolved))
        .map(|variant| variant.name.as_str())
        .collect()
}

fn valid_switch_arm_variant<'a>(
    arm: &SwitchArm,
    target_symbol: &'a TypeSymbol,
    resolved: &ResolveOutput,
) -> Option<&'a EnumVariantSignature> {
    let pattern_symbol = resolved.type_symbol_by_name(&arm.enum_name)?;
    if pattern_symbol.kind != TypeSymbolKind::Enum
        || pattern_symbol.canonical_name != target_symbol.canonical_name
    {
        return None;
    }

    let variant = target_symbol
        .variants
        .iter()
        .find(|variant| variant.name == arm.variant_name)?;
    let provided_payload_count = usize::from(arm.payload.is_some());
    (variant.payload.len() == provided_payload_count).then_some(variant)
}

fn valid_pattern_conditional_arm_variant<'a>(
    arm: &PatternConditionalArm,
    target_symbol: &'a TypeSymbol,
    resolved: &ResolveOutput,
) -> Option<&'a EnumVariantSignature> {
    let pattern_symbol = resolved.type_symbol_by_name(&arm.enum_name)?;
    if pattern_symbol.kind != TypeSymbolKind::Enum
        || pattern_symbol.canonical_name != target_symbol.canonical_name
    {
        return None;
    }

    let variant = target_symbol
        .variants
        .iter()
        .find(|variant| variant.name == arm.variant_name)?;
    let provided_payload_count = usize::from(arm.payload.is_some());
    (variant.payload.len() == provided_payload_count).then_some(variant)
}

fn enum_type_symbol_for_type<'a>(ty: &Type, resolved: &'a ResolveOutput) -> Option<&'a TypeSymbol> {
    let canonical_name = ty.nominal_name()?;

    resolved
        .type_symbol_by_canonical_name(canonical_name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Enum)
}

pub(super) fn is_enum_variant_call(call: &CallExpr, resolved: &ResolveOutput) -> bool {
    enum_member_for_call(call)
        .and_then(|member| enum_symbol_for_member(member, resolved))
        .is_some()
}

pub(super) fn enum_variant_member_type(
    member: &MemberExpr,
    resolved: &ResolveOutput,
) -> Option<Type> {
    enum_symbol_for_member(member, resolved)
        .map(|symbol| Type::Named(symbol.canonical_name.clone()))
}

pub(super) fn types_are_same_payloadless_enum(
    left: &Type,
    right: &Type,
    resolved: &ResolveOutput,
) -> bool {
    if left != right {
        return false;
    }

    let Some(symbol) = enum_type_symbol_for_type(left, resolved) else {
        return false;
    };

    symbol
        .variants
        .iter()
        .all(|variant| variant.payload.is_empty())
}

pub(super) fn resolved_enum_variant_for_member<'a>(
    member: &MemberExpr,
    resolved: &'a ResolveOutput,
) -> Option<(&'a TypeSymbol, &'a EnumVariantSignature)> {
    let enum_symbol = enum_symbol_for_member(member, resolved)?;
    let variant = enum_variant_for_member(member, enum_symbol)?;
    Some((enum_symbol, variant))
}

pub(super) fn enum_variant_call_type(
    call: &CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<Type> {
    if resolved.associated_function_for_call(call).is_some() {
        return None;
    }

    let member = enum_member_for_call(call)?;
    let enum_symbol = enum_symbol_for_member(member, resolved)?;
    let variant = enum_variant_for_member(member, enum_symbol)?;
    Some(enum_variant_constructor_type(
        enum_symbol,
        variant,
        &call.arguments,
        resolved,
        environment,
    ))
}

pub(super) fn enum_variant_expression_is_assignable(
    expected: &Type,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<bool> {
    match expression {
        Expr::Call(call) => enum_variant_call_is_assignable(expected, call, resolved, environment),
        Expr::Member(member) => enum_variant_member_is_assignable(expected, member, resolved),
        Expr::Group(group) => enum_variant_expression_is_assignable(
            expected,
            &group.expression,
            resolved,
            environment,
        ),
        _ => None,
    }
}

pub(super) fn check_enum_variant_member(
    sources: &SourceMap,
    member: &MemberExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(enum_symbol) = enum_symbol_for_member(member, resolved) else {
        return;
    };

    let Some(variant) = enum_variant_for_member(member, enum_symbol) else {
        diagnostics.push(enum_variant_unknown_diagnostic(
            sources,
            member,
            enum_symbol,
        ));
        return;
    };

    if !variant.payload.is_empty() {
        diagnostics.push(enum_variant_payload_count_mismatch_diagnostic(
            sources,
            member.member_span,
            enum_symbol,
            variant,
            variant.payload.len(),
            0,
        ));
    }
}

pub(super) fn check_enum_variant_call(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let Some(member) = enum_member_for_call(call) else {
        return;
    };
    if resolved.associated_function_for_call(call).is_some() {
        return;
    }

    let Some(enum_symbol) = enum_symbol_for_member(member, resolved) else {
        return;
    };

    let Some(variant) = enum_variant_for_member(member, enum_symbol) else {
        diagnostics.push(enum_variant_unknown_diagnostic(
            sources,
            member,
            enum_symbol,
        ));
        return;
    };

    if variant.payload.is_empty() && call.arguments.is_empty() {
        diagnostics.push(enum_variant_payloadless_call_diagnostic(
            sources,
            call,
            enum_symbol,
            variant,
        ));
        return;
    }

    if variant.payload.len() != call.arguments.len() {
        diagnostics.push(enum_variant_payload_count_mismatch_diagnostic(
            sources,
            call.arguments_span,
            enum_symbol,
            variant,
            variant.payload.len(),
            call.arguments.len(),
        ));
        return;
    }

    let substitutions = infer_enum_variant_substitutions(
        enum_symbol,
        variant,
        &call.arguments,
        resolved,
        environment,
    );
    for (index, (argument, parameter)) in call
        .arguments
        .iter()
        .zip(variant.payload.iter())
        .enumerate()
    {
        let expected = type_expr_to_type_with_substitutions(
            &parameter.ty,
            resolved,
            environment.self_type(),
            &substitutions,
        );
        let actual = expression_type(argument, resolved, environment);
        if expected.is_unknown_or_unresolved() || actual.is_unknown_or_unresolved() {
            continue;
        }

        if !is_expression_assignable(&expected, argument, resolved, environment) {
            diagnostics.push(enum_variant_payload_type_mismatch_diagnostic(
                sources,
                argument,
                enum_symbol,
                variant,
                index,
                &expected,
                &actual,
            ));
        }
    }
}

fn enum_variant_constructor_type(
    enum_symbol: &TypeSymbol,
    variant: &EnumVariantSignature,
    arguments: &[Expr],
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    if enum_symbol.generic_parameters.is_empty() {
        return Type::Named(enum_symbol.canonical_name.clone());
    }

    let substitutions =
        infer_enum_variant_substitutions(enum_symbol, variant, arguments, resolved, environment);
    let Some(arguments) = enum_symbol
        .generic_parameters
        .iter()
        .map(|parameter| substitutions.get(parameter).cloned())
        .collect::<Option<Vec<_>>>()
    else {
        return Type::Named(enum_symbol.canonical_name.clone());
    };

    Type::Generic {
        name: enum_symbol.canonical_name.clone(),
        arguments,
    }
}

fn infer_enum_variant_substitutions(
    enum_symbol: &TypeSymbol,
    variant: &EnumVariantSignature,
    arguments: &[Expr],
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> HashMap<String, Type> {
    if enum_symbol.generic_parameters.is_empty() {
        return HashMap::new();
    }

    let parameters = enum_symbol
        .generic_parameters
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut substitutions = HashMap::new();
    for (argument, parameter) in arguments.iter().zip(variant.payload.iter()) {
        let actual = expression_type(argument, resolved, environment);
        if actual.is_unknown_or_unresolved() {
            continue;
        }
        infer_type_expr_substitutions(
            &parameter.ty,
            &actual,
            resolved,
            environment.self_type(),
            &parameters,
            &mut substitutions,
        );
    }
    substitutions
}

fn enum_variant_call_is_assignable(
    expected: &Type,
    call: &CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<bool> {
    if !matches!(expected, Type::Generic { .. }) {
        return None;
    }
    if resolved.associated_function_for_call(call).is_some() {
        return None;
    }

    let expected_symbol = enum_type_symbol_for_type(expected, resolved)?;
    let member = enum_member_for_call(call)?;
    let enum_symbol = enum_symbol_for_member(member, resolved)?;
    if enum_symbol.canonical_name != expected_symbol.canonical_name {
        return Some(false);
    }

    let Some(variant) = enum_variant_for_member(member, enum_symbol) else {
        return Some(true);
    };
    if variant.payload.len() != call.arguments.len()
        || variant.payload.is_empty() && call.arguments.is_empty()
    {
        return Some(true);
    }

    let substitutions = generic_substitutions_for_enum_owner(expected_symbol, expected);
    Some(
        call.arguments
            .iter()
            .zip(variant.payload.iter())
            .all(|(argument, parameter)| {
                let expected = type_expr_to_type_with_substitutions(
                    &parameter.ty,
                    resolved,
                    environment.self_type(),
                    &substitutions,
                );
                expected.is_unknown_or_unresolved()
                    || is_expression_assignable(&expected, argument, resolved, environment)
            }),
    )
}

fn enum_variant_member_is_assignable(
    expected: &Type,
    member: &MemberExpr,
    resolved: &ResolveOutput,
) -> Option<bool> {
    if !matches!(expected, Type::Generic { .. }) {
        return None;
    }
    let expected_symbol = enum_type_symbol_for_type(expected, resolved)?;
    let enum_symbol = enum_symbol_for_member(member, resolved)?;
    if enum_symbol.canonical_name != expected_symbol.canonical_name {
        return Some(false);
    }

    Some(true)
}

fn generic_substitutions_for_enum_owner(
    enum_symbol: &TypeSymbol,
    owner_type: &Type,
) -> HashMap<String, Type> {
    let Type::Generic { name, arguments } = owner_type else {
        return HashMap::new();
    };
    if name != &enum_symbol.canonical_name
        || arguments.len() != enum_symbol.generic_parameters.len()
    {
        return HashMap::new();
    }

    enum_symbol
        .generic_parameters
        .iter()
        .cloned()
        .zip(arguments.iter().cloned())
        .collect()
}

fn enum_symbol_for_member<'a>(
    member: &MemberExpr,
    resolved: &'a ResolveOutput,
) -> Option<&'a TypeSymbol> {
    let Expr::Identifier(enum_name) = member.object.as_ref() else {
        return None;
    };

    resolved
        .type_symbol_by_name(&enum_name.name)
        .filter(|symbol| symbol.kind == TypeSymbolKind::Enum)
}

fn enum_variant_for_member<'a>(
    member: &MemberExpr,
    enum_symbol: &'a TypeSymbol,
) -> Option<&'a EnumVariantSignature> {
    enum_symbol
        .variants
        .iter()
        .find(|variant| variant.name == member.member)
}

fn enum_member_for_call(call: &CallExpr) -> Option<&MemberExpr> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };

    Some(member)
}
