//! Aggregate literal classification and construction.
//!
//! MIR retains semantic field/index paths, not ABI offsets. The backend is the
//! only layer that projects those paths onto a concrete layout.

use super::context::LoweringContext;
use super::coverage::{known_expression_type, scalar_expression_is_supported, scalar_type};
use super::{BuildError, SemanticInputs};
use crate::abi::AbiType;
use crate::ast::Expr;
use crate::mir::{
    AggregateElement, AggregateLeaf, LocalId, Origin, Place, Rvalue, ScopeId, Statement,
};

pub(super) fn literal_is_supported(expression: &Expr, semantic: SemanticInputs<'_>) -> bool {
    let Some(abi) = aggregate_abi_type(expression, semantic) else {
        return false;
    };
    literal_matches_abi(expression.without_groups(), &abi, semantic)
        || variant_matches_abi(expression.without_groups(), &abi, semantic)
}

fn literal_matches_abi(expression: &Expr, abi: &AbiType, semantic: SemanticInputs<'_>) -> bool {
    if requires_unrepresented_partial_cleanup(expression, abi, semantic) {
        return false;
    }
    match (expression, abi) {
        (Expr::StructLiteral(literal), AbiType::Struct(fields)) => {
            literal.fields.iter().all(|field| {
                let Some(abi_field) = fields.iter().find(|candidate| candidate.name == field.name)
                else {
                    return false;
                };
                value_matches_abi(&field.value, &abi_field.ty, semantic)
            })
        }
        (Expr::ArrayLiteral(literal), AbiType::Array { element, length }) => {
            usize::try_from(*length).ok() == Some(literal.elements.len())
                && literal
                    .elements
                    .iter()
                    .all(|value| value_matches_abi(value, element, semantic))
        }
        _ => false,
    }
}

fn requires_unrepresented_partial_cleanup(
    expression: &Expr,
    abi: &AbiType,
    semantic: SemanticInputs<'_>,
) -> bool {
    let values = match (expression.without_groups(), abi) {
        (Expr::StructLiteral(literal), AbiType::Struct(fields)) => literal
            .fields
            .iter()
            .filter_map(|field| {
                fields
                    .iter()
                    .find(|candidate| candidate.name == field.name)
                    .map(|abi| (&field.value, &abi.ty))
            })
            .collect::<Vec<_>>(),
        (Expr::ArrayLiteral(literal), AbiType::Array { element, .. }) => literal
            .elements
            .iter()
            .map(|value| (value, element.as_ref()))
            .collect::<Vec<_>>(),
        (expression, AbiType::Enum(enum_)) => {
            let Some((member, arguments)) = variant_member_and_arguments(expression) else {
                return false;
            };
            let Some(variant) = enum_
                .variants
                .iter()
                .find(|variant| variant.name == member.member)
            else {
                return false;
            };
            variant_payload_values(arguments, variant.payload.as_ref()).unwrap_or_default()
        }
        _ => return false,
    };

    let mut completed_owned_value = false;
    for (value, value_abi) in values {
        if completed_owned_value && expression_propagates_failure(value) {
            return true;
        }
        if requires_unrepresented_partial_cleanup(value, value_abi, semantic) {
            return true;
        }
        completed_owned_value |= value_requires_drop(value, semantic);
    }
    false
}

fn value_requires_drop(expression: &Expr, semantic: SemanticInputs<'_>) -> bool {
    let Some(ty) = known_expression_type(expression, semantic.typed_hir)
        .and_then(|ty| semantic.typed_hir.type_expr_by_id(ty))
    else {
        return false;
    };
    crate::typecheck::type_expr_is_copy(ty, semantic.resolved) == Some(false)
        && super::super::drop_plans::is_supported(
            ty,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        )
}

fn expression_propagates_failure(expression: &Expr) -> bool {
    match expression {
        Expr::Propagate(_) => true,
        Expr::Group(group) => expression_propagates_failure(&group.expression),
        Expr::ArrayLiteral(literal) => literal.elements.iter().any(expression_propagates_failure),
        Expr::StructLiteral(literal) => literal
            .fields
            .iter()
            .any(|field| expression_propagates_failure(&field.value)),
        Expr::Call(call) => call.arguments.iter().any(expression_propagates_failure),
        _ => false,
    }
}

fn variant_matches_abi(expression: &Expr, abi: &AbiType, semantic: SemanticInputs<'_>) -> bool {
    if requires_unrepresented_partial_cleanup(expression, abi, semantic) {
        return false;
    }
    let AbiType::Enum(enum_) = abi else {
        return false;
    };
    let Some((member, arguments)) = variant_member_and_arguments(expression) else {
        return false;
    };
    if semantic
        .typed_hir
        .enum_variant_target(member.member_span)
        .is_none()
    {
        return false;
    }
    let Some(variant) = enum_
        .variants
        .iter()
        .find(|variant| variant.name == member.member)
    else {
        return false;
    };
    variant_payload_values(arguments, variant.payload.as_ref()).is_some_and(|values| {
        values
            .into_iter()
            .all(|(value, abi)| value_matches_abi(value, abi, semantic))
    })
}

fn variant_member_and_arguments(expression: &Expr) -> Option<(&crate::ast::MemberExpr, &[Expr])> {
    match expression.without_groups() {
        Expr::Call(call) => match call.callee.without_groups() {
            Expr::Member(member) => Some((member, call.arguments.as_slice())),
            _ => None,
        },
        Expr::Member(member) => Some((member, &[])),
        _ => None,
    }
}

fn variant_payload_values<'a>(
    arguments: &'a [Expr],
    payload: Option<&'a AbiType>,
) -> Option<Vec<(&'a Expr, &'a AbiType)>> {
    match (arguments, payload) {
        ([], None) => Some(Vec::new()),
        ([argument], Some(payload)) => Some(vec![(argument, payload)]),
        (arguments, Some(AbiType::Struct(fields))) if arguments.len() == fields.len() => Some(
            arguments
                .iter()
                .zip(fields.iter())
                .map(|(argument, field)| (argument, &field.ty))
                .collect(),
        ),
        _ => None,
    }
}

fn value_matches_abi(expression: &Expr, abi: &AbiType, semantic: SemanticInputs<'_>) -> bool {
    if let Some(ty) = known_expression_type(expression, semantic.typed_hir)
        && scalar_type(ty, semantic.typed_hir).is_some()
    {
        return scalar_expression_is_supported(
            expression,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        );
    }
    literal_matches_abi(expression.without_groups(), abi, semantic)
}

pub(super) fn lower_literal(
    context: &mut LoweringContext<'_>,
    destination: LocalId,
    expression: &Expr,
    scope: ScopeId,
) -> Result<(), BuildError> {
    let semantic = context.semantic;
    let abi =
        aggregate_abi_type(expression, semantic).ok_or(BuildError::UnsupportedClaimedExpression)?;
    let mut leaves = Vec::new();
    let value = if let AbiType::Enum(enum_) = &abi {
        let (member, arguments) = variant_member_and_arguments(expression)
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let variant = context
            .semantic
            .typed_hir
            .enum_variant_target(member.member_span)
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let abi_variant = enum_
            .variants
            .iter()
            .find(|variant| variant.name == member.member)
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        let values = variant_payload_values(arguments, abi_variant.payload.as_ref())
            .ok_or(BuildError::UnsupportedClaimedExpression)?;
        for (index, (argument, payload_abi)) in values.into_iter().enumerate() {
            let mut path = vec![AggregateElement::VariantPayload(index)];
            lower_literal_leaves(
                context,
                argument,
                payload_abi,
                scope,
                &mut path,
                &mut leaves,
            )?;
        }
        Rvalue::Variant { variant, leaves }
    } else {
        lower_literal_leaves(
            context,
            expression.without_groups(),
            &abi,
            scope,
            &mut Vec::new(),
            &mut leaves,
        )?;
        Rvalue::Aggregate { leaves }
    };
    let origin = context
        .semantic
        .typed_hir
        .expression(expression.span())
        .map(|expression| Origin::Expression(expression.id))
        .ok_or(BuildError::MissingTypedExpression)?;
    context.control_flow.push_statement(Statement::Assign {
        destination: Place::local(destination),
        value,
        origin,
    })
}

fn lower_literal_leaves(
    context: &mut LoweringContext<'_>,
    expression: &Expr,
    abi: &AbiType,
    scope: ScopeId,
    path: &mut Vec<AggregateElement>,
    leaves: &mut Vec<AggregateLeaf>,
) -> Result<(), BuildError> {
    if let Some(ty) = known_expression_type(expression, context.semantic.typed_hir)
        && let Some(scalar) = scalar_type(ty, context.semantic.typed_hir)
    {
        leaves.push(AggregateLeaf {
            path: path.clone(),
            ty,
            scalar,
            operand: context.lower_operand(expression, ty, scalar, scope)?,
        });
        return Ok(());
    }

    match (expression.without_groups(), abi) {
        (Expr::StructLiteral(literal), AbiType::Struct(fields)) => {
            for field in &literal.fields {
                let index = fields
                    .iter()
                    .position(|candidate| candidate.name == field.name)
                    .ok_or(BuildError::UnsupportedClaimedExpression)?;
                path.push(AggregateElement::Field(index));
                lower_literal_leaves(
                    context,
                    &field.value,
                    &fields[index].ty,
                    scope,
                    path,
                    leaves,
                )?;
                path.pop();
            }
        }
        (Expr::ArrayLiteral(literal), AbiType::Array { element, length }) => {
            if usize::try_from(*length).ok() != Some(literal.elements.len()) {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            for (index, value) in literal.elements.iter().enumerate() {
                path.push(AggregateElement::Index(index));
                lower_literal_leaves(context, value, element, scope, path, leaves)?;
                path.pop();
            }
        }
        _ => return Err(BuildError::UnsupportedClaimedExpression),
    }
    Ok(())
}

fn aggregate_abi_type(expression: &Expr, semantic: SemanticInputs<'_>) -> Option<AbiType> {
    let ty = known_expression_type(expression, semantic.typed_hir)?;
    let ty = semantic.typed_hir.type_expr_by_id(ty)?;
    crate::abi::abi_value_from_type_expr_with_resolver(ty, semantic.resolved, |source| {
        semantic.resolver_for(source)
    })
    .ok()
    .map(|value| value.ty)
}
