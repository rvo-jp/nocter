use super::expressions::expression_type;
use super::model::{Type, TypeEnvironment, same_known_type};
use super::numeric::{is_integer_literal_expr, is_integer_type};
use super::operations::is_expression_assignable;
use super::type_expr::{
    type_expr_display_lossy, type_expr_to_type_in_environment, type_expr_to_type_with_substitutions,
};
use crate::ast::{
    Expr, ForRangeStmt, FunctionDecl, GenericParamList, IfIsStmt, ImplDecl, LiteralDecl,
    LiteralPackForStmt, MethodDecl, Parameter, SwitchArm, TypeExpr,
};
use crate::resolve::{ResolveOutput, TypeSymbolKind};
use std::collections::HashMap;

pub(super) fn environment_for_parameters_in_impl(
    parameters: &[Parameter],
    resolved: &ResolveOutput,
    impl_: &ImplDecl,
) -> TypeEnvironment {
    let mut environment = TypeEnvironment::with_self_type(impl_self_type(impl_, resolved));
    define_impl_generic_parameters(impl_, &mut environment);
    define_parameters_in_environment(parameters, resolved, &mut environment);
    environment
}

pub(super) fn environment_for_function(
    function: &FunctionDecl,
    resolved: &ResolveOutput,
) -> TypeEnvironment {
    let mut environment = match function_self_type(function, resolved) {
        Some(self_type) => TypeEnvironment::with_self_type(self_type),
        None => TypeEnvironment::default(),
    };
    environment.define_generic_parameters(
        function
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone()),
    );
    define_parameters_in_environment(&function.parameters.parameters, resolved, &mut environment);
    environment
}

pub(super) fn environment_for_literal(
    literal: &LiteralDecl,
    resolved: &ResolveOutput,
) -> TypeEnvironment {
    let generic_names = literal_target_generic_names(&literal.target);
    let substitutions = generic_names
        .iter()
        .map(|name| (name.clone(), Type::Parameter(name.clone())))
        .collect();
    let self_type =
        type_expr_to_type_with_substitutions(&literal.target, resolved, None, &substitutions);
    let mut environment = TypeEnvironment::with_self_type(self_type);
    environment.define_generic_parameters(generic_names);
    define_parameters_in_environment(&literal.parameters.parameters, resolved, &mut environment);
    if let Some(capture) = &literal.capture {
        let element_type =
            type_expr_to_type_in_environment(&capture.element_type, resolved, &environment);
        environment.define_literal_pack(capture.name.clone(), element_type);
    }
    environment
}

fn literal_target_generic_names(target: &TypeExpr) -> Vec<String> {
    let TypeExpr::Generic(generic) = target else {
        return Vec::new();
    };
    generic
        .arguments
        .iter()
        .filter_map(|argument| match argument {
            TypeExpr::Reference(reference) => Some(reference.name.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn function_self_type(
    function: &FunctionDecl,
    resolved: &ResolveOutput,
) -> Option<Type> {
    let owner = function.owner.as_ref()?;
    Some(
        resolved
            .type_symbol_by_name(&owner.name)
            .map(|symbol| Type::Named(symbol.canonical_name.clone()))
            .unwrap_or_else(|| Type::Unresolved(owner.name.clone())),
    )
}

pub(super) fn environment_for_method(
    method: &MethodDecl,
    resolved: &ResolveOutput,
    impl_: &ImplDecl,
) -> TypeEnvironment {
    let mut environment = TypeEnvironment::with_self_type(impl_self_type(impl_, resolved));
    define_impl_generic_parameters(impl_, &mut environment);
    let receiver = method.receiver.implicit_parameter();
    let receiver_type = type_expr_to_type_in_environment(&receiver.ty, resolved, &environment);
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
    type_expr_to_type_with_substitutions(
        &impl_.target_ty,
        resolved,
        None,
        &generic_parameter_substitutions(&impl_.generics),
    )
}

pub(super) fn impl_member_name(impl_: &ImplDecl, member_name: &str) -> String {
    format!(
        "{}.{}",
        type_expr_display_lossy(&impl_.target_ty),
        member_name
    )
}

pub(super) fn generic_parameter_substitutions(
    generics: &GenericParamList,
) -> HashMap<String, Type> {
    generics
        .parameters
        .iter()
        .map(|parameter| {
            (
                parameter.name.clone(),
                Type::Parameter(parameter.name.clone()),
            )
        })
        .collect()
}

fn define_impl_generic_parameters(impl_: &ImplDecl, environment: &mut TypeEnvironment) {
    environment.define_generic_parameters(
        impl_
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone()),
    );
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

pub(super) fn environment_for_if_is_binding(
    statement: &IfIsStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut then_environment = environment.clone();
    if let Some(payload) = statement
        .payload
        .as_ref()
        .and_then(|payload| payload.binding())
    {
        let target_type = expression_type(&statement.expression, resolved, environment);
        then_environment.define(
            payload.name.clone(),
            enum_pattern_payload_type(
                &statement.enum_name,
                &statement.variant_name,
                Some(&target_type),
                resolved,
                environment,
            )
            .unwrap_or(Type::Unknown),
        );
    }
    then_environment
}

pub(super) fn environment_for_switch_arm(
    arm: &SwitchArm,
    target: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut arm_environment = environment.clone();
    if let Some(payload) = arm.payload.as_ref().and_then(|payload| payload.binding()) {
        let target_type = expression_type(target, resolved, environment);
        arm_environment.define(
            payload.name.clone(),
            enum_pattern_payload_type(
                &arm.enum_name,
                &arm.variant_name,
                Some(&target_type),
                resolved,
                environment,
            )
            .unwrap_or(Type::Unknown),
        );
    }
    arm_environment
}

fn enum_pattern_payload_type(
    enum_name: &str,
    variant_name: &str,
    target_type: Option<&Type>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
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

    let substitutions = target_type
        .map(|ty| generic_substitutions_for_enum_owner(symbol, ty))
        .unwrap_or_default();
    Some(type_expr_to_type_with_substitutions(
        &payload.ty,
        resolved,
        environment.self_type(),
        &substitutions,
    ))
}

fn generic_substitutions_for_enum_owner(
    enum_symbol: &crate::resolve::TypeSymbol,
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

pub(super) fn environment_for_literal_pack_binding(
    statement: &LiteralPackForStmt,
    environment: &TypeEnvironment,
) -> TypeEnvironment {
    let mut body_environment = environment.clone();
    let element_type = environment
        .literal_pack_element(&statement.pack_name)
        .cloned()
        .unwrap_or(Type::Unknown);
    body_environment.define(statement.name.clone(), element_type);
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
