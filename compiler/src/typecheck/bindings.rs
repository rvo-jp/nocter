use super::diagnostics::{
    binding_type_mismatch_diagnostic, optional_let_else_fallthrough_diagnostic,
    optional_let_else_non_optional_diagnostic,
};
use super::model::{Type, TypeEnvironment};
use super::operations::is_expression_assignable;
use super::returns::block_guarantees_return_or_never;
use super::type_expr::type_expr_to_type_in_environment;
use crate::ast::BindingStmt;
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn check_optional_let_else_statement(
    sources: &SourceMap,
    statement: &BindingStmt,
    initializer_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !initializer_type.is_unknown() && !matches!(initializer_type, Type::Optional(_)) {
        diagnostics.push(optional_let_else_non_optional_diagnostic(
            sources,
            statement,
            initializer_type,
        ));
    }

    if let Some(else_block) = &statement.else_block
        && !block_guarantees_return_or_never(else_block, resolved, environment)
    {
        diagnostics.push(optional_let_else_fallthrough_diagnostic(
            sources, statement, else_block,
        ));
    }
}

pub(super) fn continuing_binding_type(
    statement: &BindingStmt,
    initializer_type: Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let inferred = if statement.else_block.is_some() {
        match initializer_type {
            Type::Optional(inner) => *inner,
            Type::Unknown => Type::Unknown,
            _ => Type::Unknown,
        }
    } else {
        initializer_type
    };

    if let Some(ty) = &statement.ty {
        return type_expr_to_type_in_environment(ty, resolved, environment);
    }

    inferred
}

pub(super) fn check_binding_annotation(
    sources: &SourceMap,
    statement: &BindingStmt,
    initializer_type: &Type,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let Some(annotation) = &statement.ty else {
        return;
    };

    let binding_type = type_expr_to_type_in_environment(annotation, resolved, environment);
    let expected_initializer = if statement.else_block.is_some() {
        Type::Optional(Box::new(binding_type.clone()))
    } else {
        binding_type.clone()
    };

    if initializer_type.is_unknown_or_unresolved()
        || expected_initializer.is_unknown_or_unresolved()
        || expected_initializer.first_unsized_part().is_some()
    {
        return;
    }

    if !is_expression_assignable(
        &expected_initializer,
        &statement.initializer,
        resolved,
        environment,
    ) {
        diagnostics.push(binding_type_mismatch_diagnostic(
            sources,
            statement,
            &binding_type,
            initializer_type,
        ));
    }
}
