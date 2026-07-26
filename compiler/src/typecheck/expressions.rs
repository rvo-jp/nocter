use super::arrays::{array_literal_type, index_expression_type};
use super::bindings::continuing_binding_type;
use super::calls::{call_return_type, resolved_call_signature};
use super::diagnostics::error_member_unknown_diagnostic;
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use super::operations::binary_expression_type;
use super::returns::block_guarantees_return_or_never;
use super::strings::interpolated_string_type;
use super::structs::{struct_literal_type, struct_member_type};
use super::type_expr::type_expr_to_type_in_environment;
use super::variants::{enum_variant_call_type, enum_variant_member_type, match_expression_type};
use crate::ast::{Block, Expr, IfIsStmt, IfStmt, MemberExpr, Stmt, UnaryOperator};
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

fn otherwise_type(
    value_type: Type,
    fallback: &Block,
    fallback_type: Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let Type::Optional(inner) = value_type else {
        return fallback_type;
    };

    if fallback_type.is_unknown()
        || super::operations::block_result_is_assignable(&inner, fallback, resolved, environment)
    {
        *inner
    } else {
        fallback_type
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
            if let Some(ty) = collection_builtin_call_type(expression, resolved, environment) {
                return ty;
            }

            resolved_call_signature(resolved, expression, environment)
                .map(|signature| call_return_type(expression, &signature, resolved, environment))
                .or_else(|| enum_variant_call_type(expression, resolved, environment))
                .unwrap_or(Type::Unknown)
        }
        Expr::Group(expression) => expression_type(&expression.expression, resolved, environment),
        Expr::Index(expression) => index_expression_type(expression, resolved, environment),
        Expr::Otherwise(expression) => {
            let value_type = expression_type(&expression.value, resolved, environment);
            let fallback_type = block_result_type(&expression.fallback, resolved, environment);
            otherwise_type(
                value_type,
                &expression.fallback,
                fallback_type,
                resolved,
                environment,
            )
        }
        Expr::If(expression) => if_expression_type(expression, resolved, environment),
        Expr::IfIs(expression) => if_is_expression_type(expression, resolved, environment),
        Expr::Match(expression) => match_expression_type(expression, resolved, environment),
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

pub(super) fn block_result_type(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let result_environment = block_result_environment(block, resolved, environment);
    if let Some(result) = &block.result {
        return expression_type(result, resolved, &result_environment);
    }

    if block_guarantees_return_or_never(block, resolved, &result_environment) {
        Type::Never
    } else {
        Type::Void
    }
}

pub(super) fn block_result_environment(
    block: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut environment = environment.clone();
    for statement in &block.statements {
        apply_statement_type_effect(statement, resolved, &mut environment);
    }
    environment
}

fn apply_statement_type_effect(
    statement: &Stmt,
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
) {
    if let Stmt::Binding(statement) = statement {
        let initializer_type = expression_type(&statement.initializer, resolved, environment);
        let binding_type =
            continuing_binding_type(statement, initializer_type, resolved, environment);
        environment.define_binding(
            statement.name.clone(),
            binding_type,
            binding_kind_is_mutable(statement.kind),
        );
    }
}

fn if_expression_type(
    expression: &IfStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let Some(else_block) = &expression.else_block else {
        return Type::Void;
    };

    compatible_block_result_type(&expression.then_block, else_block, resolved, environment)
}

fn if_is_expression_type(
    expression: &IfIsStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let Some(else_block) = &expression.else_block else {
        return Type::Void;
    };

    let then_environment =
        super::environments::environment_for_if_is_binding(expression, resolved, environment);
    compatible_block_result_type_with_environments(
        &expression.then_block,
        &then_environment,
        else_block,
        environment,
        resolved,
    )
}

fn compatible_block_result_type(
    then_block: &Block,
    else_block: &Block,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    compatible_block_result_type_with_environments(
        then_block,
        environment,
        else_block,
        environment,
        resolved,
    )
}

pub(super) fn compatible_block_result_type_with_environments(
    then_block: &Block,
    then_environment: &TypeEnvironment,
    else_block: &Block,
    else_environment: &TypeEnvironment,
    resolved: &ResolveOutput,
) -> Type {
    let then_type = block_result_type(then_block, resolved, then_environment);
    let else_type = block_result_type(else_block, resolved, else_environment);

    if then_type == Type::Never {
        return else_type;
    }
    if else_type == Type::Never {
        return then_type;
    }
    if then_type.is_unknown_or_unresolved() {
        return else_type;
    }
    if else_type.is_unknown_or_unresolved() {
        return then_type;
    }
    if super::operations::is_assignable(&then_type, &else_type) {
        return then_type;
    }
    if super::operations::is_assignable(&else_type, &then_type) {
        return else_type;
    }

    then_type
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
    collection_method_call_type(
        call,
        resolved,
        environment,
        "len",
        Type::Primitive("usize".to_string()),
    )
}

pub(super) fn collection_builtin_call_type(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<Type> {
    collection_len_call_type(call, resolved, environment).or_else(|| {
        collection_method_call_type(
            call,
            resolved,
            environment,
            "is_empty",
            Type::Primitive("bool".to_string()),
        )
    })
}

fn collection_method_call_type(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    method_name: &str,
    return_type: Type,
) -> Option<Type> {
    let Expr::Member(member) = call.callee.as_ref() else {
        return None;
    };
    if member.member != method_name || !call.arguments.is_empty() {
        return None;
    }

    let receiver_type = expression_type(&member.object, resolved, environment);
    if collection_has_len(&receiver_type) {
        Some(return_type)
    } else {
        None
    }
}

pub(super) fn collection_has_len(ty: &Type) -> bool {
    matches!(ty, Type::Str | Type::View { .. })
}
