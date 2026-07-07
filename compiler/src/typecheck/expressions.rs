use super::*;

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

fn optional_default_type(value_type: Type, default_type: Type) -> Type {
    let Type::Optional(inner) = value_type else {
        return default_type;
    };

    if default_type.is_unknown() || is_assignable(&inner, &default_type) {
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
        Expr::BoolLiteral(_) => Type::Primitive("bool".to_string()),
        Expr::NoneLiteral(_) => Type::None,
        Expr::ArrayLiteral(expression) => array_literal_type(expression, resolved, environment),
        Expr::StructLiteral(expression) => struct_literal_type(expression, resolved, environment),
        Expr::Binary(expression) => binary_expression_type(expression, resolved, environment),
        Expr::Unary(expression) => match expression.operator {
            UnaryOperator::LogicalNot => Type::Primitive("bool".to_string()),
            UnaryOperator::Negate => expression_type(&expression.operand, resolved, environment),
        },
        Expr::TypeConversion(expression) => {
            type_expr_to_type_in_environment(&expression.ty, resolved, environment)
        }
        Expr::Try(expression) => {
            expression_type(&expression.expression, resolved, environment).into_success_type()
        }
        Expr::TryCatch(expression) => {
            expression_type(&expression.expression, resolved, environment).into_success_type()
        }
        Expr::Call(expression) => {
            enum_variant_call_type(expression, resolved).unwrap_or_else(|| {
                resolved_call_signature(resolved, expression, environment)
                    .map(|signature| {
                        type_expr_to_type_with_self_type(
                            &signature.signature.return_type,
                            resolved,
                            signature.self_type.as_ref(),
                        )
                    })
                    .unwrap_or(Type::Unknown)
            })
        }
        Expr::Group(expression) => expression_type(&expression.expression, resolved, environment),
        Expr::Index(expression) => index_expression_type(expression, resolved, environment),
        Expr::OptionalDefault(expression) => {
            let value_type = expression_type(&expression.value, resolved, environment);
            let default_type = expression_type(&expression.default, resolved, environment);
            optional_default_type(value_type, default_type)
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
