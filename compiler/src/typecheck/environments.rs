use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment, binding_kind_is_mutable, same_known_type};
use super::numeric::{is_integer_literal_expr, is_integer_type};
use super::operations::is_expression_assignable;
use super::type_expr::{
    type_expr_display_lossy, type_expr_to_type, type_expr_to_type_in_environment,
};
use crate::ast::{
    Expr, ForRangeStmt, IfIsStmt, IfLetStmt, ImplDecl, MethodDecl, Parameter, SwitchArm,
    WhileLetStmt,
};
use crate::resolve::{ResolveOutput, TypeSymbolKind};

pub(super) fn environment_for_parameters(
    parameters: &[Parameter],
    resolved: &ResolveOutput,
) -> TypeEnvironment {
    let mut environment = TypeEnvironment::default();
    define_parameters_in_environment(parameters, resolved, &mut environment);
    environment
}

pub(super) fn environment_for_parameters_with_self_type(
    parameters: &[Parameter],
    resolved: &ResolveOutput,
    self_type: Type,
) -> TypeEnvironment {
    let mut environment = TypeEnvironment::with_self_type(self_type);
    define_parameters_in_environment(parameters, resolved, &mut environment);
    environment
}

pub(super) fn environment_for_method(
    method: &MethodDecl,
    resolved: &ResolveOutput,
    self_type: Type,
) -> TypeEnvironment {
    let mut environment = TypeEnvironment::with_self_type(self_type);
    let receiver_type =
        type_expr_to_type_in_environment(&method.receiver.ty, resolved, &environment);
    environment.define(method.receiver.name.clone(), receiver_type);
    define_parameters_in_environment(&method.parameters.parameters, resolved, &mut environment);
    environment
}

fn define_parameters_in_environment(
    parameters: &[Parameter],
    resolved: &ResolveOutput,
    environment: &mut TypeEnvironment,
) {
    for parameter in parameters {
        let ty = type_expr_to_type_in_environment(&parameter.ty, resolved, environment);
        environment.define(parameter.name.clone(), ty);
    }
}

pub(super) fn impl_self_type(impl_: &ImplDecl, resolved: &ResolveOutput) -> Type {
    type_expr_to_type(&impl_.target_ty, resolved)
}

pub(super) fn impl_member_name(impl_: &ImplDecl, member_name: &str) -> String {
    format!(
        "{}.{}",
        type_expr_display_lossy(&impl_.target_ty),
        member_name
    )
}

pub(super) fn environment_for_catch(
    error_name: String,
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut catch_environment = environment.clone();
    let error_type = match expression_type(expression, resolved, environment) {
        Type::Fallible { error, .. } => *error,
        _ => Type::Unknown,
    };
    catch_environment.define(error_name, error_type);
    catch_environment
}

pub(super) fn environment_for_if_let_binding(
    statement: &IfLetStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut then_environment = environment.clone();
    then_environment.define_binding(
        statement.name.clone(),
        if_let_binding_type(statement, resolved, environment),
        binding_kind_is_mutable(statement.kind),
    );
    then_environment
}

pub(super) fn environment_for_if_is_binding(
    statement: &IfIsStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut then_environment = environment.clone();
    if let Some(payload) = &statement.payload {
        then_environment.define(
            payload.name.clone(),
            enum_pattern_payload_type(&statement.enum_name, &statement.variant_name, resolved)
                .unwrap_or(Type::Unknown),
        );
    }
    then_environment
}

fn if_let_binding_type(
    statement: &IfLetStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression_type(&statement.initializer, resolved, environment) {
        Type::Optional(inner) => *inner,
        Type::Unknown => Type::Unknown,
        _ => Type::Unknown,
    }
}

pub(super) fn environment_for_while_let_binding(
    statement: &WhileLetStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut body_environment = environment.clone();
    body_environment.define_binding(
        statement.name.clone(),
        while_let_binding_type(statement, resolved, environment),
        binding_kind_is_mutable(statement.kind),
    );
    body_environment
}

fn while_let_binding_type(
    statement: &WhileLetStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    match expression_type(&statement.initializer, resolved, environment) {
        Type::Optional(inner) => *inner,
        Type::Unknown => Type::Unknown,
        _ => Type::Unknown,
    }
}

pub(super) fn environment_for_switch_arm(
    arm: &SwitchArm,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut arm_environment = environment.clone();
    if let Some(payload) = &arm.payload {
        arm_environment.define(
            payload.name.clone(),
            switch_arm_payload_type(arm, resolved).unwrap_or(Type::Unknown),
        );
    }
    arm_environment
}

fn switch_arm_payload_type(arm: &SwitchArm, resolved: &ResolveOutput) -> Option<Type> {
    enum_pattern_payload_type(&arm.enum_name, &arm.variant_name, resolved)
}

fn enum_pattern_payload_type(
    enum_name: &str,
    variant_name: &str,
    resolved: &ResolveOutput,
) -> Option<Type> {
    let symbol = resolved.type_symbol_by_name(enum_name)?;
    if symbol.kind != TypeSymbolKind::Enum {
        return None;
    }

    let variant = symbol
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)?;
    let [payload] = variant.payload.as_slice() else {
        return None;
    };

    Some(type_expr_to_type(&payload.ty, resolved))
}

pub(super) fn environment_for_for_range_binding(
    statement: &ForRangeStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut body_environment = environment.clone();
    body_environment.define(
        statement.name.clone(),
        for_range_binding_type(statement, resolved, environment),
    );
    body_environment
}

fn for_range_binding_type(
    statement: &ForRangeStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let start_type = expression_type(&statement.start, resolved, environment);
    let end_type = expression_type(&statement.end, resolved, environment);

    if start_type.is_unknown_or_unresolved() || end_type.is_unknown_or_unresolved() {
        return Type::Unknown;
    }

    if is_integer_type(&start_type) && same_known_type(&start_type, &end_type) {
        return start_type;
    }

    if is_integer_type(&start_type)
        && is_integer_literal_expr(&statement.end)
        && is_expression_assignable(&start_type, &statement.end, resolved, environment)
    {
        return start_type;
    }

    if is_integer_type(&end_type)
        && is_integer_literal_expr(&statement.start)
        && is_expression_assignable(&end_type, &statement.start, resolved, environment)
    {
        return end_type;
    }

    Type::Unknown
}
