use super::copyability::implicit_non_copy_struct_value_source;
use super::diagnostics::{binding_type_mismatch_diagnostic, non_copy_struct_binding_diagnostic};
use super::model::{Type, TypeEnvironment};
use super::operations::is_expression_assignable;
use super::type_expr::type_expr_to_type_in_environment;
use crate::ast::BindingStmt;
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn continuing_binding_type(
    statement: &BindingStmt,
    initializer_type: Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    if let Some(ty) = &statement.ty {
        return type_expr_to_type_in_environment(ty, resolved, environment);
    }

    initializer_type
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
    let expected_initializer = binding_type.clone();

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

pub(super) fn check_binding_initializer_copyability(
    sources: &SourceMap,
    statement: &BindingStmt,
    initializer_type: &Type,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let binding_type =
        continuing_binding_type(statement, initializer_type.clone(), resolved, environment);
    if binding_type.is_unknown_or_unresolved() || binding_type.first_unsized_part().is_some() {
        return;
    }

    let expected_initializer = binding_type;
    if expected_initializer.is_unknown_or_unresolved()
        || !is_expression_assignable(
            &expected_initializer,
            &statement.initializer,
            resolved,
            environment,
        )
    {
        return;
    }

    if let Some((source_name, type_name)) =
        implicit_non_copy_struct_value_source(&statement.initializer, resolved, environment)
    {
        diagnostics.push(non_copy_struct_binding_diagnostic(
            sources,
            statement,
            &source_name,
            &type_name,
        ));
    }
}
