//! Validation and static selection for instance-owned fixed comparisons.

use super::calls::{
    receiver_coerced_method_candidates_for_call, resolved_call_signature, resolved_method_call,
};
use super::model::{Type, TypeEnvironment};
use crate::ast::{
    AstFile, BinaryExpr, BinaryOperator, BorrowType, CallExpr, ComparisonOperatorKind, Expr, Item,
    MemberExpr, MethodReceiverMode, TypeExpr, TypeReference,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ComparisonSemantics {
    pub(crate) kind: ComparisonOperatorKind,
    pub(crate) reverse_operands: bool,
    pub(crate) invert_result: bool,
}

pub(crate) fn comparison_semantics(operator: BinaryOperator) -> Option<ComparisonSemantics> {
    Some(match operator {
        BinaryOperator::Equal => ComparisonSemantics {
            kind: ComparisonOperatorKind::Equality,
            reverse_operands: false,
            invert_result: false,
        },
        BinaryOperator::NotEqual => ComparisonSemantics {
            kind: ComparisonOperatorKind::Equality,
            reverse_operands: false,
            invert_result: true,
        },
        BinaryOperator::Less => ComparisonSemantics {
            kind: ComparisonOperatorKind::StrictOrder,
            reverse_operands: false,
            invert_result: false,
        },
        BinaryOperator::Greater => ComparisonSemantics {
            kind: ComparisonOperatorKind::StrictOrder,
            reverse_operands: true,
            invert_result: false,
        },
        BinaryOperator::LessEqual => ComparisonSemantics {
            kind: ComparisonOperatorKind::StrictOrder,
            reverse_operands: true,
            invert_result: true,
        },
        BinaryOperator::GreaterEqual => ComparisonSemantics {
            kind: ComparisonOperatorKind::StrictOrder,
            reverse_operands: false,
            invert_result: true,
        },
        _ => return None,
    })
}

fn semantic_operands(expression: &BinaryExpr) -> (&Expr, &Expr) {
    if comparison_semantics(expression.operator).is_some_and(|semantics| semantics.reverse_operands)
    {
        (&expression.right, &expression.left)
    } else {
        (&expression.left, &expression.right)
    }
}

pub(crate) fn synthetic_comparison_call(expression: &BinaryExpr) -> CallExpr {
    let semantics = comparison_semantics(expression.operator).expect("comparison operator");
    let (left, right) = semantic_operands(expression);
    CallExpr {
        span: expression.span,
        callee: Box::new(Expr::Member(MemberExpr {
            span: expression.operator_span,
            object: Box::new(left.clone()),
            member: crate::semantic::OperatorCallableKind::for_comparison(semantics.kind)
                .lookup_name()
                .to_string(),
            member_span: expression.operator_span,
        })),
        arguments_span: right.span(),
        arguments: vec![right.clone()],
    }
}

/// Preserves authored left-to-right evaluation. Native call lowering swaps the already evaluated
/// scalar arguments when the comparison plan uses reversed strict-order orientation.
pub(crate) fn synthetic_comparison_runtime_call(expression: &BinaryExpr) -> CallExpr {
    let semantics = comparison_semantics(expression.operator).expect("comparison operator");
    CallExpr {
        span: expression.span,
        callee: Box::new(Expr::Member(MemberExpr {
            span: expression.operator_span,
            object: expression.left.clone(),
            member: crate::semantic::OperatorCallableKind::for_comparison(semantics.kind)
                .lookup_name()
                .to_string(),
            member_span: expression.operator_span,
        })),
        arguments_span: expression.right.span(),
        arguments: vec![expression.right.as_ref().clone()],
    }
}

pub(super) fn comparison_operator_matches(
    expression: &BinaryExpr,
    left_type: &Type,
    right_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    let semantics = comparison_semantics(expression.operator).expect("comparison operator");
    let (semantic_left_type, semantic_right_type) = if semantics.reverse_operands {
        (right_type, left_type)
    } else {
        (left_type, right_type)
    };
    let requirement = match semantics.kind {
        ComparisonOperatorKind::Equality => {
            environment.equality_requirement_span(semantic_left_type, semantic_right_type)
        }
        ComparisonOperatorKind::StrictOrder => {
            environment.ordering_requirement_span(semantic_left_type, semantic_right_type)
        }
    };
    if requirement.is_some() {
        return true;
    }
    let call = synthetic_comparison_call(expression);
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
        && comparison_operand_adjustment(
            &expected,
            semantic_operands(expression).1,
            semantic_right_type,
            resolved,
            environment,
        )
        .is_some()
}

#[derive(Debug, Clone)]
pub(super) struct ComparisonOperandAdjustment {
    pub(super) implicit_readonly_borrow: bool,
    pub(super) conversion: Option<super::conversions::SelectedConversion>,
}

pub(super) fn comparison_operand_adjustment(
    expected: &Type,
    expression: &Expr,
    actual: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<ComparisonOperandAdjustment> {
    if let Ok(conversion) = super::conversions::select_expression_conversion(
        super::conversions::ConversionMode::Contextual,
        expected,
        expression,
        resolved,
        environment,
    ) {
        return Some(ComparisonOperandAdjustment {
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
        return Some(ComparisonOperandAdjustment {
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
    Some(ComparisonOperandAdjustment {
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
    comparison_operator_matches(&expression, left_type, right_type, resolved, &local)
}

pub(super) fn types_support_ordering(
    left_type: &Type,
    right_type: &Type,
    span: ByteSpan,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    if super::operations::types_have_builtin_ordering(left_type, right_type) {
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
        operator: BinaryOperator::Less,
        operator_span: span,
        right: Box::new(identifier("__nocter_operator_right")),
    };
    comparison_operator_matches(&expression, left_type, right_type, resolved, &local)
}

pub(super) fn resolved_comparison_method<'a>(
    expression: &BinaryExpr,
    resolved: &'a ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<super::calls::ResolvedMethodCall<'a>> {
    resolved_method_call(
        resolved,
        &synthetic_comparison_call(expression),
        environment,
    )
}

pub(super) fn ambiguous_comparison_methods<'a>(
    expression: &BinaryExpr,
    semantic_right_type: &Type,
    resolved: &'a ResolveOutput,
    environment: &TypeEnvironment,
) -> Vec<super::calls::ResolvedMethodCall<'a>> {
    receiver_coerced_method_candidates_for_call(
        resolved,
        &synthetic_comparison_call(expression),
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
        comparison_operand_adjustment(
            &expected,
            semantic_operands(expression).1,
            semantic_right_type,
            resolved,
            environment,
        )
        .is_some()
    })
    .collect()
}

pub(super) fn comparison_ambiguity_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    candidates: &[super::calls::ResolvedMethodCall<'_>],
) -> Diagnostic {
    let description = if comparison_semantics(expression.operator)
        .is_some_and(|semantics| semantics.kind == ComparisonOperatorKind::StrictOrder)
    {
        "ordering"
    } else {
        "equality"
    };
    let mut diagnostic = Diagnostic::error(
        "E0474",
        format!("{description} is ambiguous across readonly operand coercions"),
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
                    "coercion to `{}` selects the {description} operator declared for `{}`",
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

pub(super) fn operator_method_fact(
    selected: &super::calls::ResolvedMethodCall<'_>,
    span: ByteSpan,
    resolved: &ResolveOutput,
) -> Option<super::facts::TypecheckProtocolMethod> {
    let mut free_type_parameters = HashSet::new();
    let self_ty = super::facts::type_to_type_expr_allowing_parameters(
        selected.self_type.opaque_lowering_view(),
        span,
        &mut free_type_parameters,
    )?;
    Some(super::facts::TypecheckProtocolMethod::new(
        resolved
            .semantic_db
            .definition_at(selected.method.name_span)
            .expect("resolved operator must have a semantic definition"),
        selected.method.name_span,
        super::facts::method_target_name_from_self_ty(&self_ty, &selected.method.name),
        self_ty,
        selected.method.receiver.mode,
        selected.method.name.clone(),
        free_type_parameters,
    ))
}

pub(crate) fn specialize_comparison_plan(
    mut plan: super::facts::TypecheckComparisonPlan,
    resolved: &ResolveOutput,
) -> Option<super::facts::TypecheckComparisonPlan> {
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
        operator: match (plan.kind, plan.reverse_operands) {
            (ComparisonOperatorKind::Equality, _) => BinaryOperator::Equal,
            (ComparisonOperatorKind::StrictOrder, false) => BinaryOperator::Less,
            (ComparisonOperatorKind::StrictOrder, true) => BinaryOperator::Greater,
        },
        operator_span: plan.operator_span,
        right: Box::new(identifier("__nocter_operator_right", plan.right_span)),
    };
    let selected = resolved_comparison_method(&expression, resolved, &environment)?;
    plan.method = operator_method_fact(&selected, plan.operator_span, resolved);
    let (semantic_left_type, semantic_right_type, semantic_left_span, semantic_right_span) =
        if plan.reverse_operands {
            (&right_type, &left_type, plan.right_span, plan.left_span)
        } else {
            (&left_type, &right_type, plan.left_span, plan.right_span)
        };
    if let Some(coercion) = selected.receiver_coercion.clone() {
        let selected = super::conversions::selected_receiver_coercion(semantic_left_type, coercion);
        let conversion = super::facts::typecheck_conversion_plan(
            semantic_left_span,
            semantic_left_span,
            None,
            selected,
        );
        if plan.reverse_operands {
            plan.right_conversion = conversion;
        } else {
            plan.left_conversion = conversion;
        }
    }
    let parameter = selected.method.signature.parameters.first()?;
    let expected = super::type_expr::type_expr_to_type_with_substitutions(
        &parameter.ty,
        resolved,
        Some(&selected.self_type),
        &std::collections::HashMap::new(),
    );
    let adjustment = comparison_operand_adjustment(
        &expected,
        semantic_operands(&expression).1,
        semantic_right_type,
        resolved,
        &environment,
    )?;
    let semantic_right_conversion = adjustment.conversion.and_then(|conversion| {
        super::facts::typecheck_conversion_plan(
            semantic_right_span,
            semantic_right_span,
            None,
            conversion,
        )
    });
    if plan.reverse_operands {
        plan.left_conversion = semantic_right_conversion;
    } else {
        plan.right_conversion = semantic_right_conversion;
    }
    let expected_receiver = Type::Borrow {
        is_readwrite: false,
        inner: Box::new(selected.self_type.clone()),
    };
    plan.right_implicit_readonly_borrow = comparison_operand_adjustment(
        &expected_receiver,
        &expression.right,
        &right_type,
        resolved,
        &environment,
    )
    .is_some_and(|adjustment| adjustment.implicit_readonly_borrow);
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
        for operator in instance.comparison_operators() {
            let callable = operator.callable();
            let description = match operator.kind {
                ComparisonOperatorKind::Equality => "equality",
                ComparisonOperatorKind::StrictOrder => "ordering",
            };
            if callable.receiver.mode != MethodReceiverMode::ReadonlyBorrow {
                diagnostics.push(operator_shape_diagnostic(
                    sources,
                    callable.receiver.span,
                    &format!("{description} left operand must be readonly `&self`"),
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
                    &format!("{description} right operand type must be readonly `&Self`"),
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
                    &format!("{description} operator return type must be `bool`"),
                ));
            }
        }
        for operator in instance.index_operators() {
            let callable = operator.callable();
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
        for operator in instance.expansion_operators() {
            let callable = operator.callable();
            let environment =
                super::environments::environment_for_method(callable, resolved, instance);
            if !callable.parameters.parameters.is_empty() {
                diagnostics.push(operator_shape_diagnostic(
                    sources,
                    callable.parameters.span,
                    "expansion operator does not accept ordinary parameters",
                ));
            }
            if let Some(runtime) = resolved.trusted_declarations.iteration_runtime() {
                let result = super::type_expr::type_expr_to_type_in_environment(
                    &callable.return_type,
                    resolved,
                    &environment,
                );
                if !result.is_unknown_or_unresolved()
                    && super::iteration::conformed_protocol_type(
                        &result,
                        &runtime.iterator,
                        resolved,
                        &environment,
                    )
                    .is_none()
                {
                    diagnostics.push(operator_shape_diagnostic(
                        sources,
                        callable.return_type.span(),
                        "expansion operator return type must conform to `Iterator`",
                    ));
                }
            }
        }
    }
}

fn operator_shape_diagnostic(sources: &SourceMap, span: ByteSpan, message: &str) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0470", message);
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic
}
