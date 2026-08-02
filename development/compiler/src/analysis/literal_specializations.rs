//! Concrete hidden-call specializations for typed literal expressions.

use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    Expr, Item, LiteralDecl, LiteralShape, TypeExpr, substitute_type_expr_parameters,
    type_expr_display_lossy, visit_file_expressions,
};
use crate::source::ByteSpan;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiteralSpecialization {
    pub(crate) declaration_span: ByteSpan,
    pub(crate) expression_span: ByteSpan,
    pub(crate) target_name: String,
    pub(crate) result_type: TypeExpr,
    pub(crate) element_type: Option<TypeExpr>,
    pub(crate) argument_types: Vec<TypeExpr>,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
    pub(crate) shape: LiteralShape,
}

pub(crate) fn collect_literal_specializations(
    analysis: &CompileUnitAnalysis,
) -> HashMap<ByteSpan, Vec<LiteralSpecialization>> {
    let declarations = literal_declarations(analysis);
    let mut by_declaration = HashMap::<ByteSpan, Vec<LiteralSpecialization>>::new();

    for file in &analysis.files {
        visit_file_expressions(&file.ast, &mut |expression| {
            let Some(specialization) =
                specialization_for_expression(file, expression, &declarations, &HashMap::new())
            else {
                return;
            };
            let entries = by_declaration
                .entry(specialization.declaration_span)
                .or_default();
            if !entries.iter().any(|entry| {
                entry.target_name == specialization.target_name
                    && entry.argument_types == specialization.argument_types
            }) {
                entries.push(specialization);
            }
        });
    }

    by_declaration
}

pub(crate) fn literal_target_name(
    result_type: &TypeExpr,
    shape: LiteralShape,
    argument_count: usize,
) -> String {
    let shape = match shape {
        LiteralShape::Sequence => "sequence",
        LiteralShape::String => "string",
    };
    format!(
        "{}.$literal.{shape}${argument_count}",
        type_expr_display_lossy(result_type)
    )
}

pub(crate) fn literal_element_parameter_name(index: usize) -> String {
    format!("<literal-element-{index}>")
}

fn specialization_for_expression(
    file: &FileAnalysis,
    expression: &Expr,
    declarations: &HashMap<ByteSpan, &LiteralDecl>,
    context_substitutions: &HashMap<String, TypeExpr>,
) -> Option<LiteralSpecialization> {
    let (span, shape, argument_count) = match expression {
        Expr::TypedSequenceLiteral(literal) => {
            (literal.span, LiteralShape::Sequence, literal.elements.len())
        }
        Expr::TypedStringLiteral(literal) => (literal.span, LiteralShape::String, 1),
        _ => return None,
    };
    let resolution = file.resolved.literal_resolution(span)?;
    let declaration = declarations.get(&resolution.literal_declaration_span)?;
    let result_type = substitute_type_expr_parameters(
        file.typecheck_facts.expression_type_expr(span)?,
        context_substitutions,
    );
    let generic_parameters = literal_generic_parameters(declaration);
    let mut substitutions = HashMap::new();
    if !infer_substitutions(
        &declaration.target,
        &result_type,
        &generic_parameters,
        &mut substitutions,
    ) {
        return None;
    }
    if substitutions
        .values()
        .any(|ty| type_contains_parameter(ty, &generic_parameters))
    {
        return None;
    }

    let (element_type, argument_types) = match shape {
        LiteralShape::Sequence => {
            let element_type = substitute_type_expr_parameters(
                &declaration.capture.as_ref()?.element_type,
                &substitutions,
            );
            (
                Some(element_type.clone()),
                vec![element_type; argument_count],
            )
        }
        LiteralShape::String => {
            let parameters = declaration
                .parameters
                .parameters
                .iter()
                .map(|parameter| substitute_type_expr_parameters(&parameter.ty, &substitutions))
                .collect();
            (None, parameters)
        }
    };
    Some(LiteralSpecialization {
        declaration_span: declaration.span,
        expression_span: span,
        target_name: literal_target_name(&result_type, shape, argument_count),
        result_type,
        element_type,
        argument_types,
        substitutions,
        shape,
    })
}

fn literal_declarations(analysis: &CompileUnitAnalysis) -> HashMap<ByteSpan, &LiteralDecl> {
    analysis
        .files
        .iter()
        .flat_map(|file| file.ast.items.iter())
        .filter_map(|item| match item {
            Item::Literal(literal) => Some((literal.span, literal)),
            _ => None,
        })
        .collect()
}

fn literal_generic_parameters(literal: &LiteralDecl) -> HashSet<String> {
    match &literal.target {
        TypeExpr::Generic(generic) => generic
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                TypeExpr::Reference(reference) => Some(reference.name.clone()),
                _ => None,
            })
            .collect(),
        _ => HashSet::new(),
    }
}

fn infer_substitutions(
    expected: &TypeExpr,
    actual: &TypeExpr,
    parameters: &HashSet<String>,
    substitutions: &mut HashMap<String, TypeExpr>,
) -> bool {
    if let TypeExpr::Reference(reference) = expected
        && parameters.contains(&reference.name)
    {
        return match substitutions.get(&reference.name) {
            Some(previous) => type_expr_display_lossy(previous) == type_expr_display_lossy(actual),
            None => {
                substitutions.insert(reference.name.clone(), actual.clone());
                true
            }
        };
    }
    match (expected, actual) {
        (TypeExpr::Reference(expected), TypeExpr::Reference(actual)) => {
            expected.name == actual.name
        }
        (TypeExpr::Generic(expected), TypeExpr::Generic(actual)) => {
            expected.name == actual.name
                && expected.arguments.len() == actual.arguments.len()
                && expected
                    .arguments
                    .iter()
                    .zip(&actual.arguments)
                    .all(|(expected, actual)| {
                        infer_substitutions(expected, actual, parameters, substitutions)
                    })
        }
        (TypeExpr::Pointer(expected), TypeExpr::Pointer(actual)) => {
            infer_substitutions(&expected.inner, &actual.inner, parameters, substitutions)
        }
        (TypeExpr::Borrow(expected), TypeExpr::Borrow(actual)) => {
            expected.is_readwrite == actual.is_readwrite
                && infer_substitutions(&expected.inner, &actual.inner, parameters, substitutions)
        }
        (TypeExpr::View(expected), TypeExpr::View(actual)) => {
            expected.is_readwrite == actual.is_readwrite
                && infer_substitutions(
                    &expected.element,
                    &actual.element,
                    parameters,
                    substitutions,
                )
        }
        (TypeExpr::Array(expected), TypeExpr::Array(actual)) => {
            expected.length.value == actual.length.value
                && infer_substitutions(
                    &expected.element,
                    &actual.element,
                    parameters,
                    substitutions,
                )
        }
        (TypeExpr::Optional(expected), TypeExpr::Optional(actual)) => {
            infer_substitutions(&expected.inner, &actual.inner, parameters, substitutions)
        }
        (TypeExpr::Fallible(expected), TypeExpr::Fallible(actual)) => {
            infer_substitutions(
                &expected.success,
                &actual.success,
                parameters,
                substitutions,
            ) && infer_substitutions(&expected.error, &actual.error, parameters, substitutions)
        }
        _ => false,
    }
}

fn type_contains_parameter(ty: &TypeExpr, parameters: &HashSet<String>) -> bool {
    match ty {
        TypeExpr::Reference(reference) => parameters.contains(&reference.name),
        TypeExpr::Generic(generic) => generic
            .arguments
            .iter()
            .any(|argument| type_contains_parameter(argument, parameters)),
        TypeExpr::Pointer(pointer) => type_contains_parameter(&pointer.inner, parameters),
        TypeExpr::Borrow(borrow) => type_contains_parameter(&borrow.inner, parameters),
        TypeExpr::View(view) => type_contains_parameter(&view.element, parameters),
        TypeExpr::Array(array) => type_contains_parameter(&array.element, parameters),
        TypeExpr::Optional(optional) => type_contains_parameter(&optional.inner, parameters),
        TypeExpr::Fallible(fallible) => {
            type_contains_parameter(&fallible.success, parameters)
                || type_contains_parameter(&fallible.error, parameters)
        }
    }
}
