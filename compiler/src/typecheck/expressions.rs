use super::arrays::{array_literal_type, index_expression_type};
use super::calls::{call_return_type, resolved_call_signature};
use super::diagnostics::error_member_unknown_diagnostic;
use super::model::{Type, TypeEnvironment};
use super::operations::{binary_expression_type, is_expression_assignable};
use super::strings::interpolated_string_type;
use super::structs::{struct_literal_type, struct_member_type};
use super::type_expr::type_expr_to_type_in_environment;
use super::variants::{
    enum_variant_call_type, enum_variant_member_type, pattern_conditional_expression_type,
};
use crate::ast::{Expr, MemberExpr, UnaryOperator};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

fn error_member_type(
    member: &MemberExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<Type> {
    if expression_type(&member.object, resolved, environment) != Type::Error {
        return None;
    }

    match member.member.as_str() {
        "code" | "message" => Some(Type::Str),
        _ => Some(Type::Unknown),
    }
}

pub(super) fn check_error_member_expression(
    sources: &SourceMap,
    member: &MemberExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    if expression_type(&member.object, resolved, environment) != Type::Error {
        return;
    }

    if matches!(member.member.as_str(), "code" | "message") {
        return;
    }

    diagnostics.push(error_member_unknown_diagnostic(sources, member));
}

fn optional_default_type(
    value_type: Type,
    default: &Expr,
    default_type: Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let Type::Optional(inner) = value_type else {
        return default_type;
    };

    if default_type.is_unknown() || is_expression_assignable(&inner, default, resolved, environment)
    {
        *inner
    } else {
        default_type
    }
}

pub(super) fn expression_type(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression {
        Expr::IntegerLiteral(_) => Type::I32,
        Expr::StringLiteral(_) => Type::Str,
        Expr::InterpolatedString(_) => interpolated_string_type(resolved),
        Expr::BoolLiteral(_) => Type::Primitive("bool".to_string()),
        Expr::NoneLiteral(_) => Type::None,
        Expr::ArrayLiteral(expression) => array_literal_type(expression, resolved, environment),
        Expr::StructLiteral(expression) => struct_literal_type(expression, resolved, environment),
        Expr::Binary(expression) => binary_expression_type(expression, resolved, environment),
        Expr::Unary(expression) => match expression.operator {
            UnaryOperator::LogicalNot => Type::Primitive("bool".to_string()),
            UnaryOperator::Negate | UnaryOperator::Move => {
                expression_type(&expression.operand, resolved, environment)
            }
        },
        Expr::TypeConversion(expression) => {
            type_expr_to_type_in_environment(&expression.ty, resolved, environment)
        }
        Expr::Propagate(expression) => {
            expression_type(&expression.expression, resolved, environment).into_propagated_type()
        }
        Expr::Force(expression) => {
            expression_type(&expression.expression, resolved, environment).into_propagated_type()
        }
        Expr::Catch(expression) => expression_type(&expression.expression, resolved, environment)
            .into_fallible_success_type(),
        Expr::Borrow(expression) => borrow_expression_type(expression, resolved, environment),
        Expr::Call(expression) => {
            if collection_len_call_type(expression, resolved, environment).is_some() {
                return Type::Primitive("usize".to_string());
            }

            resolved_call_signature(resolved, expression, environment)
                .map(|signature| call_return_type(expression, &signature, resolved, environment))
                .or_else(|| enum_variant_call_type(expression, resolved, environment))
                .unwrap_or(Type::Unknown)
        }
        Expr::Group(expression) => expression_type(&expression.expression, resolved, environment),
        Expr::Index(expression) => index_expression_type(expression, resolved, environment),
        Expr::OptionalDefault(expression) => {
            let value_type = expression_type(&expression.value, resolved, environment);
            let default_type = expression_type(&expression.default, resolved, environment);
            optional_default_type(
                value_type,
                &expression.default,
                default_type,
                resolved,
                environment,
            )
        }
        Expr::PatternConditional(expression) => {
            pattern_conditional_expression_type(expression, resolved, environment)
        }
        Expr::Identifier(expression) => environment
            .get(&expression.name)
            .cloned()
            .unwrap_or(Type::Unknown),
        Expr::Member(expression) => enum_variant_member_type(expression, resolved)
            .or_else(|| error_member_type(expression, resolved, environment))
            .or_else(|| struct_member_type(expression, resolved, environment))
            .unwrap_or(Type::Unknown),
    }
}

fn borrow_expression_type(
    expression: &crate::ast::BorrowExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let inner = expression_type(&expression.expression, resolved, environment);
    if inner.is_unknown_or_unresolved() {
        return Type::Unknown;
    }

    Type::Named(format!(
        "{}{}",
        if expression.is_readwrite { "&+" } else { "&" },
        inner.display()
    ))
}

pub(super) fn collection_len_call_type(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<Type> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    if member.member != "len" || !call.arguments.is_empty() {
        return None;
    }

    let receiver_type = expression_type(&member.object, resolved, environment);
    if collection_has_len(&receiver_type) {
        Some(Type::Primitive("usize".to_string()))
    } else {
        None
    }
}

pub(super) fn collection_has_len(ty: &Type) -> bool {
    matches!(ty, Type::Str | Type::View { .. })
}
