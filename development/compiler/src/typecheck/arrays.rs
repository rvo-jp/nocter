use super::diagnostics::{
    array_literal_element_type_mismatch_diagnostic, index_target_type_mismatch_diagnostic,
    index_value_type_mismatch_diagnostic,
};
use super::expressions::expression_type;
use super::indexing::{IndexAccess, IndexRejection, select_index_expression};
use super::model::{Type, TypeEnvironment, same_known_type};
use super::numeric::integer_literal_value;
use crate::ast::{ArrayLiteralExpr, Expr, IndexExpr};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

pub(super) fn check_array_literal_elements(
    sources: &SourceMap,
    array: &ArrayLiteralExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let mut first_known: Option<(&Expr, Type)> = None;

    for element in &array.elements {
        let element_type = expression_type(element, resolved, environment);
        if element_type.is_unknown_or_unresolved() {
            continue;
        }

        let Some((first_element, first_type)) = &first_known else {
            first_known = Some((element, element_type));
            continue;
        };

        if !same_known_type(first_type, &element_type) {
            diagnostics.push(array_literal_element_type_mismatch_diagnostic(
                sources,
                element,
                &element_type,
                first_element,
                first_type,
            ));
            return;
        }
    }
}

pub(super) fn array_literal_type(
    array: &ArrayLiteralExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    let element = infer_array_literal_element_type(&array.elements, resolved, environment)
        .unwrap_or(Type::Unknown);

    Type::Array {
        element: Box::new(element),
        length: array.elements.len().to_string(),
    }
}

fn infer_array_literal_element_type(
    elements: &[Expr],
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<Type> {
    let mut inferred: Option<Type> = None;

    for element in elements {
        let element_type = expression_type(element, resolved, environment);
        if element_type.is_unknown_or_unresolved() {
            continue;
        }

        match &inferred {
            Some(current) if !same_known_type(current, &element_type) => return None,
            Some(_) => {}
            None => inferred = Some(element_type),
        }
    }

    inferred
}

pub(super) fn check_index_expression(
    sources: &SourceMap,
    index: &IndexExpr,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
    environment: &TypeEnvironment,
) {
    let target = expression_type(&index.object, resolved, environment);
    let index_type = expression_type(&index.index, resolved, environment);
    match select_index_expression(index, IndexAccess::Readonly, resolved, environment) {
        Ok(_) => {}
        Err(IndexRejection::InvalidIndex) => {
            if !index_type.is_unknown_or_unresolved() {
                diagnostics.push(index_value_type_mismatch_diagnostic(
                    sources,
                    index,
                    &index_type,
                ));
            }
        }
        Err(IndexRejection::UnsupportedTarget | IndexRejection::RequiresReadwrite) => {
            if !target.is_unknown_or_unresolved() {
                diagnostics.push(index_target_type_mismatch_diagnostic(
                    sources, index, &target,
                ));
            }
        }
        Err(IndexRejection::AmbiguousCoercion) => diagnostics.push(
            super::diagnostics::ambiguous_index_coercion_diagnostic(sources, index, &target),
        ),
    }
}

pub(super) fn index_expression_type(
    index: &IndexExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Type {
    select_index_expression(index, IndexAccess::Readonly, resolved, environment)
        .map(|selected| selected.element_type)
        .unwrap_or(Type::Unknown)
}

pub(super) fn array_length_matches(expected: &str, actual: usize) -> bool {
    integer_literal_value(expected).is_some_and(|value| value == actual as u128)
}
