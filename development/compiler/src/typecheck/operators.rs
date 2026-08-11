//! Validation and static selection for instance-owned equality.

use super::calls::{
    receiver_coerced_method_candidates_for_call, resolved_call_signature, resolved_method_call,
};
use super::model::{Type, TypeEnvironment};
use crate::ast::{
    AstFile, BinaryExpr, BorrowType, CallExpr, EQUALITY_OPERATOR_METHOD_NAME, Expr, Item,
    MemberExpr, MethodReceiverMode, TypeExpr, TypeReference,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashSet;

pub(crate) fn synthetic_equality_call(expression: &BinaryExpr) -> CallExpr {
    CallExpr {
        span: expression.span,
        callee: Box::new(Expr::Member(MemberExpr {
            span: expression.operator_span,
            object: expression.left.clone(),
            member: EQUALITY_OPERATOR_METHOD_NAME.to_string(),
            member_span: expression.operator_span,
        })),
        arguments_span: expression.right.span(),
        arguments: vec![expression.right.as_ref().clone()],
    }
}

pub(super) fn equality_operator_matches(
    expression: &BinaryExpr,
    left_type: &Type,
    right_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    if environment
        .equality_requirement_span(left_type, right_type)
        .is_some()
    {
        return true;
    }
    let call = synthetic_equality_call(expression);
    let Some(signature) = resolved_call_signature(resolved, &call, environment) else {
        return false;
    };
    let Some(parameter) = signature.signature.parameters.first() else {
        return false;
    };
    let expected = super::type_expr::type_expr_to_type_with_substitutions(
        &parameter.ty,
        resolved,
        signature.self_type.as_ref(),
        &std::collections::HashMap::new(),
    );
    !expected.is_unknown_or_unresolved()
        && equality_right_adjustment(
            &expected,
            &expression.right,
            right_type,
            resolved,
            environment,
        )
        .is_some()
}

#[derive(Debug, Clone)]
pub(super) struct EqualityOperandAdjustment {
    pub(super) implicit_readonly_borrow: bool,
    pub(super) conversion: Option<super::conversions::SelectedConversion>,
}

pub(super) fn equality_right_adjustment(
    expected: &Type,
    expression: &Expr,
    actual: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<EqualityOperandAdjustment> {
    if let Ok(conversion) = super::conversions::select_expression_conversion(
        super::conversions::ConversionMode::Contextual,
        expected,
        expression,
        resolved,
        environment,
    ) {
        return Some(EqualityOperandAdjustment {
            implicit_readonly_borrow: false,
            conversion: Some(conversion),
        });
    }

    let Type::Borrow {
        is_readwrite: false,
        inner: expected_inner,
    } = expected
    else {
        return None;
    };
    if environment.types_equal(expected_inner, actual) {
        return Some(EqualityOperandAdjustment {
            implicit_readonly_borrow: true,
            conversion: None,
        });
    }
    actual.nominal_name()?;
    let borrowed_actual = Type::Borrow {
        is_readwrite: false,
        inner: Box::new(actual.clone()),
    };
    let conversion = super::conversions::select_conversion(
        super::conversions::ConversionMode::Contextual,
        expected,
        &borrowed_actual,
        expression,
        resolved,
        environment,
    )
    .ok()?;
    Some(EqualityOperandAdjustment {
        implicit_readonly_borrow: true,
        conversion: Some(conversion),
    })
}

pub(super) fn types_support_equality(
    left_type: &Type,
    right_type: &Type,
    span: ByteSpan,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    if super::operations::types_have_builtin_equality(left_type, right_type, resolved) {
        return true;
    }
    let mut local = environment.clone();
    local.define("__nocter_operator_left".to_string(), left_type.clone());
    local.define("__nocter_operator_right".to_string(), right_type.clone());
    let identifier = |name: &str| {
        Expr::Identifier(crate::ast::IdentifierExpr {
            span,
            name: name.to_string(),
        })
    };
    let expression = BinaryExpr {
        span,
        left: Box::new(identifier("__nocter_operator_left")),
        operator: crate::ast::BinaryOperator::Equal,
        operator_span: span,
        right: Box::new(identifier("__nocter_operator_right")),
    };
    equality_operator_matches(&expression, left_type, right_type, resolved, &local)
}

pub(super) fn resolved_equality_method<'a>(
    expression: &BinaryExpr,
    resolved: &'a ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<super::calls::ResolvedMethodCall<'a>> {
    resolved_method_call(resolved, &synthetic_equality_call(expression), environment)
}

pub(super) fn ambiguous_equality_methods<'a>(
    expression: &BinaryExpr,
    right_type: &Type,
    resolved: &'a ResolveOutput,
    environment: &TypeEnvironment,
) -> Vec<super::calls::ResolvedMethodCall<'a>> {
    receiver_coerced_method_candidates_for_call(
        resolved,
        &synthetic_equality_call(expression),
        environment,
    )
    .into_iter()
    .filter(|selected| {
        let Some(parameter) = selected.method.signature.parameters.first() else {
            return false;
        };
        let expected = super::type_expr::type_expr_to_type_with_substitutions(
            &parameter.ty,
            resolved,
            Some(&selected.self_type),
            &std::collections::HashMap::new(),
        );
        equality_right_adjustment(
            &expected,
            &expression.right,
            right_type,
            resolved,
            environment,
        )
        .is_some()
    })
    .collect()
}

pub(super) fn equality_ambiguity_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    candidates: &[super::calls::ResolvedMethodCall<'_>],
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0474",
        "equality is ambiguous across readonly operand coercions",
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    for candidate in candidates {
        let Some(coercion) = &candidate.receiver_coercion else {
            continue;
        };
        if let Ok(span) = sources.span_to_json(coercion.focus_span) {
            diagnostic.notes.push(crate::diagnostics::DiagnosticNote {
                message: format!(
                    "coercion to `{}` selects the equality operator declared for `{}`",
                    coercion.target_type.display(),
                    candidate.owner.canonical_name,
                ),
                span: Some(span),
            });
        }
    }
    diagnostic.help =
        Some("use an explicit `as` coercion to choose one readonly operand type".to_string());
    diagnostic
}

pub(super) fn equality_method_fact(
    selected: &super::calls::ResolvedMethodCall<'_>,
    span: ByteSpan,
) -> Option<super::facts::TypecheckProtocolMethod> {
    let mut free_type_parameters = HashSet::new();
    let self_ty = super::facts::type_to_type_expr_allowing_parameters(
        selected.self_type.opaque_lowering_view(),
        span,
        &mut free_type_parameters,
    )?;
    Some(super::facts::TypecheckProtocolMethod::new(
        selected.method.name_span,
        super::facts::method_target_name_from_self_ty(&self_ty, &selected.method.name),
        self_ty,
        selected.method.receiver.mode,
        selected.method.name.clone(),
        free_type_parameters,
    ))
}

pub(crate) fn specialize_equality_plan(
    mut plan: super::facts::TypecheckEqualityPlan,
    resolved: &ResolveOutput,
) -> Option<super::facts::TypecheckEqualityPlan> {
    if plan.method.is_some() {
        return Some(plan);
    }
    let left_type = super::type_expr::type_expr_to_type(&plan.left_ty, resolved);
    let right_type = super::type_expr::type_expr_to_type(&plan.right_ty, resolved);
    if left_type.is_unknown_or_unresolved() || right_type.is_unknown_or_unresolved() {
        return None;
    }
    let mut environment = TypeEnvironment::default();
    environment.define("__nocter_operator_left".to_string(), left_type.clone());
    environment.define("__nocter_operator_right".to_string(), right_type.clone());
    let identifier = |name: &str, span: ByteSpan| {
        Expr::Identifier(crate::ast::IdentifierExpr {
            span,
            name: name.to_string(),
        })
    };
    let expression = BinaryExpr {
        span: plan.call_span,
        left: Box::new(identifier("__nocter_operator_left", plan.left_span)),
        operator: crate::ast::BinaryOperator::Equal,
        operator_span: plan.operator_span,
        right: Box::new(identifier("__nocter_operator_right", plan.right_span)),
    };
    let selected = resolved_equality_method(&expression, resolved, &environment)?;
    plan.method = equality_method_fact(&selected, plan.operator_span);
    if let Some(coercion) = selected.receiver_coercion.clone() {
        let selected = super::conversions::selected_receiver_coercion(&left_type, coercion);
        plan.left_conversion =
            super::facts::typecheck_conversion_plan(plan.left_span, plan.left_span, None, selected);
    }
    let parameter = selected.method.signature.parameters.first()?;
    let expected = super::type_expr::type_expr_to_type_with_substitutions(
        &parameter.ty,
        resolved,
        Some(&selected.self_type),
        &std::collections::HashMap::new(),
    );
    let adjustment = equality_right_adjustment(
        &expected,
        &expression.right,
        &right_type,
        resolved,
        &environment,
    )?;
    plan.right_implicit_readonly_borrow = adjustment.implicit_readonly_borrow;
    plan.right_conversion = adjustment.conversion.and_then(|conversion| {
        super::facts::typecheck_conversion_plan(plan.right_span, plan.right_span, None, conversion)
    });
    Some(plan)
}

pub(super) fn check_operator_declarations(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for instance in ast.items.iter().filter_map(|item| match item {
        Item::Instance(instance) => Some(instance),
        _ => None,
    }) {
        for operator in instance.equality_operators() {
            let callable = operator.callable_method();
            if callable.receiver.mode != MethodReceiverMode::ReadonlyBorrow {
                diagnostics.push(operator_shape_diagnostic(
                    sources,
                    callable.receiver.span,
                    "equality left operand must be readonly `&self`",
                ));
            }
            let valid_right = callable.parameters.parameters.first().is_some_and(|parameter| {
                matches!(
                    &parameter.ty,
                    TypeExpr::Borrow(BorrowType {
                        is_readwrite: false,
                        inner,
                        ..
                    }) if matches!(inner.as_ref(), TypeExpr::Reference(TypeReference { name, .. }) if name == "Self")
                )
            });
            if !valid_right {
                diagnostics.push(operator_shape_diagnostic(
                    sources,
                    callable.parameters.span,
                    "equality right operand type must be readonly `&Self`",
                ));
            }
            let return_type = super::type_expr::type_expr_to_type_in_environment(
                &callable.return_type,
                resolved,
                &super::environments::environment_for_method(callable, resolved, instance),
            );
            if return_type != Type::Primitive("bool".to_string()) {
                diagnostics.push(operator_shape_diagnostic(
                    sources,
                    callable.return_type.span(),
                    "equality operator return type must be `bool`",
                ));
            }
        }
        for operator in instance.index_operators() {
            let callable = operator.callable_method();
            if !matches!(
                callable.receiver.mode,
                MethodReceiverMode::ReadonlyBorrow | MethodReceiverMode::ReadwriteBorrow
            ) {
                diagnostics.push(operator_shape_diagnostic(
                    sources,
                    callable.receiver.span,
                    "index receiver must be `&self` or `&+self`",
                ));
            }
            let result = super::type_expr::type_expr_to_type_in_environment(
                &callable.return_type,
                resolved,
                &super::environments::environment_for_method(callable, resolved, instance),
            );
            let expected_readwrite = callable.receiver.mode == MethodReceiverMode::ReadwriteBorrow;
            if !matches!(
                result,
                Type::Borrow { is_readwrite, .. } if is_readwrite == expected_readwrite
            ) {
                diagnostics.push(operator_shape_diagnostic(
                    sources,
                    callable.return_type.span(),
                    if expected_readwrite {
                        "readwrite index operator return type must be `&+T`"
                    } else {
                        "readonly index operator return type must be `&T`"
                    },
                ));
            }
        }
    }
}

fn operator_shape_diagnostic(sources: &SourceMap, span: ByteSpan, message: &str) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0470", message);
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic
}
