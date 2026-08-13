//! Concrete hidden-call specializations for typed literal expressions.

use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    Expr, LiteralDecl, LiteralShape, TypeExpr, canonical_type_expr, substitute_type_expr_parameters,
};
use crate::semantic::DefId;
use crate::source::ByteSpan;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiteralSpecialization {
    pub(crate) def_id: DefId,
    pub(crate) declaration_span: ByteSpan,
    pub(crate) expression_span: ByteSpan,
    pub(crate) target_name: String,
    pub(crate) result_type: TypeExpr,
    pub(crate) element_type: Option<TypeExpr>,
    pub(crate) argument_types: Vec<TypeExpr>,
    pub(crate) pack_segments: Vec<LiteralPackSegmentSpecialization>,
    pub(crate) substitutions: HashMap<String, TypeExpr>,
    pub(crate) shape: LiteralShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LiteralPackSegmentSpecialization {
    Value {
        parameter_index: usize,
    },
    Spread {
        iterator_parameter_index: usize,
        plan: crate::typecheck::TypecheckSequenceSpreadPlan,
    },
}

pub(crate) fn collect_literal_specializations(
    analysis: &CompileUnitAnalysis,
) -> HashMap<DefId, Vec<LiteralSpecialization>> {
    let declarations = literal_declarations(analysis);
    let mut by_declaration = HashMap::<DefId, Vec<LiteralSpecialization>>::new();

    for file in &analysis.files {
        for expression in file.syntax.literal_expressions() {
            let Some(specialization) = specialization_for_expression(
                file,
                expression,
                &declarations,
                &HashMap::new(),
                true,
            ) else {
                continue;
            };
            let entries = by_declaration.entry(specialization.def_id).or_default();
            if !entries.iter().any(|entry| {
                entry.target_name == specialization.target_name
                    && entry.argument_types == specialization.argument_types
            }) {
                entries.push(specialization);
            }
        }
    }

    by_declaration
}

pub(crate) fn literal_specialization_for_expression_span(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    expression_span: ByteSpan,
) -> Option<LiteralSpecialization> {
    let declarations = literal_declarations(analysis);
    let expression = file
        .syntax
        .literal_expressions()
        .find(|expression| expression.span() == expression_span)?;
    specialization_for_expression(file, expression, &declarations, &HashMap::new(), false)
}

pub(crate) fn literal_target_name(
    result_type: &TypeExpr,
    shape: LiteralShape,
    specialization_key: &str,
) -> String {
    let shape = match shape {
        LiteralShape::Sequence => "sequence",
        LiteralShape::String => "string",
    };
    format!(
        "{}.$literal.{shape}${:016x}",
        canonical_type_expr(result_type),
        stable_key_hash(specialization_key)
    )
}

pub(crate) fn literal_specialization_key(
    shape: LiteralShape,
    elements: &[Expr],
    facts: &crate::typecheck::TypedHir,
    substitutions: &HashMap<String, TypeExpr>,
) -> Option<String> {
    if shape == LiteralShape::String {
        return Some("string".to_string());
    }
    let mut key = String::new();
    for element in elements {
        if let Some(spread) = crate::typecheck::sequence_spread(element) {
            let plan = facts
                .sequence_spread_plan(spread.span)?
                .with_context_substitutions(substitutions)?;
            key.push_str("s:");
            key.push_str(&format!("{:?}:", plan.mode));
            key.push_str(&canonical_type_expr(&plan.iterator_type));
            key.push(';');
        } else {
            key.push_str("v;");
        }
    }
    Some(key)
}

fn stable_key_hash(key: &str) -> u64 {
    key.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

pub(crate) fn literal_element_parameter_name(index: usize) -> String {
    format!("<literal-element-{index}>")
}

fn specialization_for_expression(
    file: &FileAnalysis,
    expression: &Expr,
    declarations: &HashMap<DefId, &LiteralDecl>,
    context_substitutions: &HashMap<String, TypeExpr>,
    require_concrete: bool,
) -> Option<LiteralSpecialization> {
    let (span, shape) = match expression {
        Expr::TypedSequenceLiteral(literal) => (literal.span, LiteralShape::Sequence),
        Expr::TypedStringLiteral(literal) => (literal.span, LiteralShape::String),
        _ => return None,
    };
    let resolution = file.resolved.literal_resolution(span)?;
    let declaration = declarations.get(&resolution.literal_definition)?;
    let result_type = substitute_type_expr_parameters(
        file.typed_hir.expression_type_expr(span)?,
        context_substitutions,
    );
    let generic_parameters = literal_generic_parameters(declaration);
    let mut substitutions = HashMap::new();
    if !infer_literal_target_substitutions(
        &declaration.target,
        &result_type,
        &generic_parameters,
        &mut substitutions,
    ) {
        return None;
    }
    if require_concrete
        && substitutions
            .values()
            .any(|ty| type_contains_parameter(ty, &generic_parameters))
    {
        return None;
    }

    let (element_type, argument_types, pack_segments, specialization_key) = match shape {
        LiteralShape::Sequence => {
            let element_type = substitute_type_expr_parameters(
                &declaration.capture.as_ref()?.element_type,
                &substitutions,
            );
            let Expr::TypedSequenceLiteral(literal) = expression else {
                return None;
            };
            let mut argument_types = Vec::new();
            let mut pack_segments = Vec::new();
            let mut key = String::new();
            for element in &literal.elements {
                if let Some(spread) = crate::typecheck::sequence_spread(element) {
                    let plan = file
                        .typed_hir
                        .sequence_spread_plan(spread.span)?
                        .with_context_substitutions(&substitutions)?;
                    let iterator_parameter_index = argument_types.len();
                    argument_types.push(plan.iterator_type.clone());
                    key.push_str("s:");
                    key.push_str(&format!("{:?}:", plan.mode));
                    key.push_str(&canonical_type_expr(&plan.iterator_type));
                    key.push(';');
                    pack_segments.push(LiteralPackSegmentSpecialization::Spread {
                        iterator_parameter_index,
                        plan,
                    });
                } else {
                    let parameter_index = argument_types.len();
                    argument_types.push(element_type.clone());
                    key.push_str("v;");
                    pack_segments.push(LiteralPackSegmentSpecialization::Value { parameter_index });
                }
            }
            debug_assert_eq!(
                key,
                literal_specialization_key(
                    shape,
                    &literal.elements,
                    &file.typed_hir,
                    &substitutions,
                )?
            );
            (Some(element_type), argument_types, pack_segments, key)
        }
        LiteralShape::String => {
            let parameters = declaration
                .parameters
                .parameters
                .iter()
                .map(|parameter| substitute_type_expr_parameters(&parameter.ty, &substitutions))
                .collect();
            (None, parameters, Vec::new(), "string".to_string())
        }
    };
    Some(LiteralSpecialization {
        def_id: resolution.literal_definition,
        declaration_span: declaration.span,
        expression_span: span,
        target_name: literal_target_name(&result_type, shape, &specialization_key),
        result_type,
        element_type,
        argument_types,
        pack_segments,
        substitutions,
        shape,
    })
}

fn infer_literal_target_substitutions(
    declared_target: &TypeExpr,
    resolved_target: &TypeExpr,
    parameters: &HashSet<String>,
    substitutions: &mut HashMap<String, TypeExpr>,
) -> bool {
    // `literal_resolution` has already established nominal identity by `DefId`.
    // Imported type facts use canonical qualified names while declarations use their local
    // spelling, so repeating a textual constructor-name comparison here would reject the same
    // nominal type across a module boundary. Only the type arguments remain to be inferred.
    match (declared_target, resolved_target) {
        (TypeExpr::Reference(_), TypeExpr::Reference(_)) => true,
        (TypeExpr::Generic(declared), TypeExpr::Generic(resolved)) => {
            declared.arguments.len() == resolved.arguments.len()
                && declared
                    .arguments
                    .iter()
                    .zip(&resolved.arguments)
                    .all(|(expected, actual)| {
                        infer_substitutions(expected, actual, parameters, substitutions)
                    })
        }
        _ => false,
    }
}

fn literal_declarations(analysis: &CompileUnitAnalysis) -> HashMap<DefId, &LiteralDecl> {
    let mut declarations = HashMap::new();
    for file in &analysis.files {
        declarations.extend(file.syntax.literals());
    }
    declarations
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
            Some(previous) => canonical_type_expr(previous) == canonical_type_expr(actual),
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
        TypeExpr::Callable(callable) => {
            callable
                .parameters
                .iter()
                .any(|parameter| type_contains_parameter(&parameter.ty, parameters))
                || type_contains_parameter(&callable.return_type, parameters)
        }
        TypeExpr::Closure(closure) => {
            closure
                .captures
                .iter()
                .any(|capture| type_contains_parameter(&capture.ty, parameters))
                || closure
                    .parameters
                    .iter()
                    .any(|parameter| type_contains_parameter(parameter, parameters))
                || type_contains_parameter(&closure.return_type, parameters)
        }
        TypeExpr::Opaque(opaque) => {
            type_contains_parameter(&opaque.interface, parameters)
                || opaque
                    .associated_bindings
                    .iter()
                    .any(|binding| type_contains_parameter(&binding.value, parameters))
                || opaque
                    .witness
                    .as_ref()
                    .is_some_and(|witness| type_contains_parameter(witness, parameters))
        }
        TypeExpr::Reference(reference) => parameters.contains(&reference.name),
        TypeExpr::Generic(generic) => generic
            .arguments
            .iter()
            .any(|argument| type_contains_parameter(argument, parameters)),
        TypeExpr::Projection(projection) => type_contains_parameter(&projection.base, parameters),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;

    #[test]
    fn keys_literal_specializations_by_definition_identity() {
        let (_sources, analysis) = analyze_text(
            r#"struct Bucket<T> { length: usize }

construct Bucket<T> {
    pub default literal [](...items: T): Self {
        return Bucket<T> { length: items.len() }
    }
}

func main(): i32 {
    let values = Bucket [1, 2]
    return values.length
}
"#,
        );
        let specializations = collect_literal_specializations(&analysis);
        let (definition, entries) = specializations
            .iter()
            .next()
            .expect("literal specialization");
        assert_eq!(entries.len(), 1);
        assert_eq!(*definition, entries[0].def_id);
        assert_eq!(
            analysis
                .semantic_db
                .definition_at(entries[0].declaration_span),
            Some(*definition)
        );
    }
}
