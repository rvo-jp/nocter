use super::arrays::{array_literal_type, index_expression_type};
use super::bindings::continuing_binding_type;
use super::calls::{call_return_type, resolved_call_signature};
use super::diagnostics::error_member_unknown_diagnostic;
use super::literals::{literal_expression_type, literal_pack_len_call_type};
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable};
use super::operations::binary_expression_type;
use super::returns::block_guarantees_control_exit_or_never;
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

    Some(
        if crate::builtin_types::BuiltinErrorField::from_source_name(&member.member).is_some() {
            Type::Str
        } else {
            Type::Unknown
        },
    )
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

    if crate::builtin_types::BuiltinErrorField::from_source_name(&member.member).is_some() {
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
        Expr::Closure(closure) => {
            super::closures::infer_closure_type(closure, resolved, environment, None, None)
                .unwrap_or(Type::Unknown)
        }
        Expr::IntegerLiteral(_) => Type::I32,
        Expr::ByteLiteral(_) => Type::Primitive("u8".to_string()),
        Expr::StringLiteral(_) => Type::Str,
        Expr::InterpolatedString(_) => interpolated_string_type(resolved),
        Expr::BoolLiteral(_) => Type::Primitive("bool".to_string()),
        Expr::NoneLiteral(_) => Type::None,
        Expr::ArrayLiteral(expression) => array_literal_type(expression, resolved, environment),
        Expr::TypedSequenceLiteral(_) | Expr::TypedStringLiteral(_) => {
            literal_expression_type(expression, resolved, environment)
        }
        Expr::StructLiteral(expression) => struct_literal_type(expression, resolved, environment),
        Expr::Binary(expression) => binary_expression_type(expression, resolved, environment),
        Expr::Unary(expression) => match expression.operator {
            UnaryOperator::LogicalNot => Type::Primitive("bool".to_string()),
            UnaryOperator::Negate | UnaryOperator::Move | UnaryOperator::Spread => {
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
            if let Some(ty) = literal_pack_len_call_type(expression, resolved, environment) {
                return ty;
            }
            if let Some(contract) =
                super::callables::callable_contract_for_call(expression, resolved, environment)
            {
                return contract.return_type;
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

    if block_guarantees_control_exit_or_never(block, resolved, &result_environment) {
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
    if super::operations::is_assignable_in_environment(&then_type, &else_type, then_environment) {
        return then_type;
    }
    if super::operations::is_assignable_in_environment(&else_type, &then_type, else_environment) {
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

    if let Type::Borrow { is_readwrite, .. } = &inner
        && (!expression.is_readwrite || *is_readwrite)
        && let Type::Borrow { inner, .. } = inner
    {
        return Type::Borrow {
            is_readwrite: expression.is_readwrite,
            inner,
        };
    }

    Type::Borrow {
        is_readwrite: expression.is_readwrite,
        inner: Box::new(inner),
    }
}
