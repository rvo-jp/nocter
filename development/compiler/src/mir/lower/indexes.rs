//! Construction of checked fixed-array index places.

use super::context::LoweringContext;
use super::{BuildError, SemanticInputs};
use crate::abi::{AbiType, abi_value_from_type_expr_with_resolver, array_element_stride};
use crate::ast::{Expr, IndexExpr};
use crate::mir::{
    LocalId, Operand, OwnershipKind, Place, ProjectionElement, ProjectionPath, ProjectionPathId,
    ScalarType, ScopeId, ValueRepresentation,
};

pub(super) fn is_supported(index: &IndexExpr, semantic: SemanticInputs<'_>) -> bool {
    let Some(plan) = semantic.index_plan(index.span) else {
        return false;
    };
    if plan.projection == crate::typecheck::TypecheckIndexProjection::Declared {
        let object_supported = if let Some(conversion) = &plan.conversion {
            super::source_model::coercion_expression_is_supported_with_explicit_plan(
                &index.object,
                conversion,
                semantic,
            )
        } else {
            super::borrows::source_place_is_supported(&index.object, semantic)
        };
        let method_supported = plan.method.as_ref().is_some_and(|method| {
            super::storage_types::runtime_type_id_for_type_expr(&method.self_ty, semantic).is_some()
        });
        let element_supported = semantic.typed_hir.type_id(&plan.element_ty).is_some();
        let index_supported = super::source_model::scalar_expression_is_supported(
            &index.index,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        );
        return method_supported && element_supported && object_supported && index_supported;
    }
    if plan.projection == crate::typecheck::TypecheckIndexProjection::Slice {
        return view_is_supported(index, semantic)
            && semantic
                .typed_hir
                .type_id(&plan.element_ty)
                .and_then(|ty| super::source_model::value_representation(ty, semantic))
                .is_some();
    }
    if plan.projection != crate::typecheck::TypecheckIndexProjection::Array
        || plan.conversion.is_some()
        || !base_is_supported(&index.object, semantic)
    {
        return false;
    }
    let Some(index_ty) = semantic.typed_hir.type_id(&plan.index_ty) else {
        return false;
    };
    let index_is_usize =
        super::source_model::scalar_type(index_ty, semantic.typed_hir) == Some(ScalarType::Usize);
    let index_is_contextual_literal = matches!(index.index.without_groups(), Expr::IntegerLiteral(literal)
        if crate::literals::decode_integer_literal_value(&literal.value)
            .is_some_and(|value| u64::try_from(value).is_ok()));
    (index_is_usize || index_is_contextual_literal)
        && super::source_model::scalar_expression_is_supported(
            &index.index,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        )
        && array_contract(index, semantic).is_some()
}

pub(super) fn view_is_supported(index: &IndexExpr, semantic: SemanticInputs<'_>) -> bool {
    let Some(plan) = semantic.index_plan(index.span) else {
        return false;
    };
    let kind = match plan.projection {
        crate::typecheck::TypecheckIndexProjection::Str => crate::mir::ViewKind::Str,
        crate::typecheck::TypecheckIndexProjection::Slice => crate::mir::ViewKind::Slice,
        _ => return false,
    };
    let Some(source_ty) = plan
        .conversion
        .as_ref()
        .and_then(|conversion| semantic.typed_hir.type_id(&conversion.target_ty))
        .or_else(|| super::source_model::handled_outcome_success_type(&index.object, semantic))
        .or_else(|| super::source_model::known_expression_type(&index.object, semantic.typed_hir))
    else {
        return false;
    };
    let Some(index_ty) = semantic.typed_hir.type_id(&plan.index_ty) else {
        return false;
    };
    let index_is_usize =
        super::source_model::scalar_type(index_ty, semantic.typed_hir) == Some(ScalarType::Usize);
    let index_is_contextual_literal = matches!(index.index.without_groups(), Expr::IntegerLiteral(literal)
        if crate::literals::decode_integer_literal_value(&literal.value)
            .is_some_and(|value| u64::try_from(value).is_ok()));
    let conversion_supported = plan.conversion.as_ref().is_none_or(|conversion| {
        super::source_model::coercion_expression_is_supported_with_explicit_plan(
            &index.object,
            conversion,
            semantic,
        )
    });
    conversion_supported
        && (plan.conversion.is_some()
            && super::source_model::value_representation(source_ty, semantic)
                == Some(ValueRepresentation::View(kind))
            || plan.conversion.is_none()
                && super::source_model::expression_view_kind(&index.object, semantic) == Some(kind))
        && (index_is_usize || index_is_contextual_literal)
        && super::source_model::scalar_expression_is_supported(
            &index.index,
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
        )
}

pub(super) fn lower_place(
    context: &mut LoweringContext<'_>,
    index: &IndexExpr,
    scope: ScopeId,
) -> Result<(Place, ValueRepresentation), BuildError> {
    let view_supported = view_is_supported(index, context.semantic);
    if !is_supported(index, context.semantic) && !view_supported {
        return Err(BuildError::UnsupportedClaimedExpression.context("validate indexed place"));
    }
    if view_supported {
        return lower_view_index_place(context, index, scope);
    }
    if context
        .semantic
        .index_plan(index.span)
        .is_some_and(|plan| plan.projection == crate::typecheck::TypecheckIndexProjection::Declared)
    {
        return lower_declared_index_place(context, index, scope)
            .map_err(|error| error.context("lower declared indexed place"));
    }
    let (base, parent) = lower_base(context, &index.object)?;
    let contract =
        array_contract(index, context.semantic).ok_or(BuildError::UnsupportedClaimedExpression)?;
    let index_ty = context
        .semantic
        .typed_hir
        .index_plan(index.span)
        .and_then(|plan| context.semantic.typed_hir.type_id(&plan.index_ty))
        .ok_or(BuildError::MissingTypedExpression)?;
    let index_operand = context.lower_operand(&index.index, index_ty, ScalarType::Usize, scope)?;
    let element = ProjectionElement::Index {
        index: index_operand,
        length: contract.length,
        stride: contract.stride,
    };
    let id = if let Some(existing) = context.projections.iter().find(|projection| {
        projection.base == base
            && projection.parent == parent
            && projection.element == element
            && projection.ty == contract.ty
    }) {
        existing.id
    } else {
        let id = ProjectionPathId::from_index(context.projections.len());
        context.projections.push(ProjectionPath {
            id,
            base,
            parent,
            element,
            ty: contract.ty,
            representation: contract.representation,
            ownership: contract.ownership,
            drop_plan: if contract.ownership == OwnershipKind::Move
                && contract.representation == ValueRepresentation::Aggregate
            {
                Some(
                    super::super::drop_plans::build(
                        &contract.type_expr,
                        context.semantic.resolved,
                        context.semantic.resolved_sources,
                        context.semantic.typed_hir,
                        &mut context.drop_plans,
                    )
                    .ok_or(BuildError::UnsupportedClaimedExpression)?,
                )
            } else {
                None
            },
        });
        id
    };
    Ok((Place::projected(base, id), contract.representation))
}

fn lower_declared_index_place(
    context: &mut LoweringContext<'_>,
    index: &IndexExpr,
    scope: ScopeId,
) -> Result<(Place, ValueRepresentation), BuildError> {
    let plan = context
        .semantic
        .index_plan(index.span)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let method = plan.method.ok_or(BuildError::MissingCallTarget)?;
    let receiver = if let Some(conversion) = &plan.conversion {
        let ty = context
            .semantic
            .typed_hir
            .type_id(&conversion.target_ty)
            .ok_or(BuildError::MissingMethodReceiverType)?;
        let local = context.local_for_type(
            ty,
            crate::mir::LocalOrigin::Desugared(index.object.span()),
            scope,
        )?;
        context.lower_planned_coercion_to_local(local, &index.object, conversion, scope)?;
        crate::mir::CallArgument {
            operand: Operand::Copy(Place::local(local)),
            ty,
            representation: ValueRepresentation::Borrow,
        }
    } else {
        context
            .lower_protocol_receiver(
                &method,
                &index.object,
                scope,
                crate::mir::Origin::Desugared(index.object.span()),
            )
            .map_err(|error| error.context("lower declared index receiver"))?
    };
    let index_argument = context
        .lower_call_argument(&index.index, scope)
        .map_err(|error| error.context("lower declared index argument"))?;
    let element_ty = context
        .semantic
        .typed_hir
        .type_id(&plan.element_ty)
        .ok_or(BuildError::MissingTypedExpression)?;
    let representation = super::source_model::value_representation(element_ty, context.semantic)
        .ok_or(BuildError::UnsupportedClaimedExpression)
        .map_err(|error| error.context("resolve declared index element representation"))?;
    let readwrite = method.receiver_mode == crate::ast::MethodReceiverMode::ReadwriteBorrow;
    let result_type = crate::ast::TypeExpr::Borrow(crate::ast::BorrowType {
        span: index.span,
        is_readwrite: readwrite,
        inner: Box::new(plan.element_ty.clone()),
    });
    let result_ty = context
        .semantic
        .typed_hir
        .type_id(&result_type)
        .ok_or(BuildError::MissingTypedExpression)?;
    let result = LocalId::from_index(context.locals.len());
    context.locals.push(crate::mir::Local::borrow(
        result_ty,
        readwrite,
        crate::mir::LocalStorage::Local,
        crate::mir::LocalOrigin::Desugared(index.span),
        scope,
    ));
    let receiver_ty =
        super::storage_types::runtime_type_id_for_type_expr(&method.self_ty, context.semantic)
            .ok_or(BuildError::MissingSpecializedReceiverType)?;
    let origin = context
        .semantic
        .typed_hir
        .expression(index.span)
        .ok_or(BuildError::MissingTypedExpression)?
        .id;
    context.control_flow.emit_returning_call(
        origin,
        crate::mir::CallInstance::specialized(
            context
                .semantic
                .resolved
                .callable_bodies
                .canonical_definition(method.def_id),
            Some(receiver_ty),
            Vec::new(),
        ),
        vec![receiver, index_argument],
        result,
    )?;
    let projection = ProjectionPathId::from_index(context.projections.len());
    context.projections.push(ProjectionPath {
        id: projection,
        base: result,
        parent: None,
        element: ProjectionElement::Dereference,
        ty: element_ty,
        representation,
        ownership: if super::super::drop_plans::is_copy(
            &plan.element_ty,
            context.semantic.resolved,
            context.semantic.resolved_sources,
        ) == Some(true)
        {
            OwnershipKind::Copy
        } else {
            OwnershipKind::Move
        },
        drop_plan: None,
    });
    Ok((Place::projected(result, projection), representation))
}

fn lower_view_index_place(
    context: &mut LoweringContext<'_>,
    index: &IndexExpr,
    scope: ScopeId,
) -> Result<(Place, ValueRepresentation), BuildError> {
    let plan = context
        .semantic
        .index_plan(index.span)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let kind = match plan.projection {
        crate::typecheck::TypecheckIndexProjection::Str => crate::mir::ViewKind::Str,
        crate::typecheck::TypecheckIndexProjection::Slice => crate::mir::ViewKind::Slice,
        _ => return Err(BuildError::UnsupportedClaimedExpression),
    };
    let source_ty = plan
        .conversion
        .as_ref()
        .and_then(|conversion| context.semantic.typed_hir.type_id(&conversion.target_ty))
        .or_else(|| {
            super::source_model::handled_outcome_success_type(&index.object, context.semantic)
        })
        .or_else(|| super::source_model::expression_value_type(&index.object, context.semantic))
        .ok_or(BuildError::MissingTypedExpression)?;
    let source = if let Some(conversion) = &plan.conversion {
        let local = context.local_for_type(
            source_ty,
            crate::mir::LocalOrigin::Desugared(index.object.span()),
            scope,
        )?;
        context
            .lower_planned_coercion_to_local(local, &index.object, conversion, scope)
            .map_err(|error| error.context("lower indexed view conversion"))?;
        Place::local(local)
    } else {
        let operand = context
            .lower_view_operand(&index.object, source_ty, kind, scope)
            .map_err(|error| error.context("lower indexed view source"))?;
        match operand {
            Operand::Copy(source) => source,
            operand => {
                let origin = context
                    .semantic
                    .typed_hir
                    .expression(index.object.span())
                    .map_or(
                        crate::mir::LocalOrigin::Desugared(index.object.span()),
                        |expression| crate::mir::LocalOrigin::Temporary(expression.id),
                    );
                let local = LocalId::from_index(context.locals.len());
                context.locals.push(crate::mir::Local::view(
                    source_ty,
                    kind,
                    crate::mir::LocalStorage::Local,
                    origin,
                    scope,
                ));
                context
                    .control_flow
                    .push_statement(crate::mir::Statement::Assign {
                        destination: Place::local(local),
                        value: crate::mir::Rvalue::Use(operand),
                        origin: crate::mir::Origin::Desugared(index.object.span()),
                    })?;
                Place::local(local)
            }
        }
    };
    let checked_index_ty = context
        .semantic
        .typed_hir
        .type_id(&plan.index_ty)
        .ok_or(BuildError::MissingTypedExpression)?;
    let (index_ty, index_scalar) =
        if super::source_model::scalar_type(checked_index_ty, context.semantic.typed_hir)
            == Some(ScalarType::Usize)
        {
            (checked_index_ty, ScalarType::Usize)
        } else if matches!(index.index.without_groups(), Expr::IntegerLiteral(_)) {
            let usize_ty = context
                .semantic
                .typed_hir
                .type_id(&crate::ast::TypeExpr::Reference(
                    crate::ast::TypeReference {
                        span: index.index.span(),
                        name: "usize".to_string(),
                    },
                ))
                .ok_or(BuildError::MissingTypedExpression)?;
            (usize_ty, ScalarType::Usize)
        } else {
            return Err(BuildError::UnsupportedClaimedExpression);
        };
    let index_operand = context
        .lower_operand(&index.index, index_ty, index_scalar, scope)
        .map_err(|error| error.context("lower indexed view offset"))?;
    let ty = context
        .semantic
        .typed_hir
        .type_id(&plan.element_ty)
        .ok_or(BuildError::MissingTypedExpression)?;
    let representation = super::source_model::value_representation(ty, context.semantic)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let ownership = if representation == ValueRepresentation::Aggregate
        && super::super::drop_plans::is_copy(
            &plan.element_ty,
            context.semantic.resolved,
            context.semantic.resolved_sources,
        ) != Some(true)
    {
        OwnershipKind::Move
    } else {
        OwnershipKind::Copy
    };
    let drop_plan =
        if representation == ValueRepresentation::Aggregate && ownership == OwnershipKind::Move {
            Some(
                super::super::drop_plans::build(
                    &plan.element_ty,
                    context.semantic.resolved,
                    context.semantic.resolved_sources,
                    context.semantic.typed_hir,
                    &mut context.drop_plans,
                )
                .ok_or(BuildError::UnsupportedClaimedExpression)?,
            )
        } else {
            None
        };
    let id = ProjectionPathId::from_index(context.projections.len());
    context.projections.push(ProjectionPath {
        id,
        base: source.local,
        parent: source.projection,
        element: ProjectionElement::ViewIndex {
            index: index_operand,
        },
        ty,
        representation,
        ownership,
        drop_plan,
    });
    Ok((Place::projected(source.local, id), representation))
}

fn base_is_supported(expression: &Expr, semantic: SemanticInputs<'_>) -> bool {
    match expression.without_groups() {
        Expr::Identifier(identifier) => semantic
            .resolved
            .local_symbol_for_identifier(identifier)
            .is_some(),
        Expr::Member(member) => super::projections::field_is_supported(member, semantic),
        _ => false,
    }
}

fn lower_base(
    context: &mut LoweringContext<'_>,
    expression: &Expr,
) -> Result<(LocalId, Option<ProjectionPathId>), BuildError> {
    match expression.without_groups() {
        Expr::Identifier(identifier) => {
            let symbol = context
                .semantic
                .resolved
                .local_symbol_for_identifier(identifier)
                .ok_or(BuildError::MissingLocalSymbol)?;
            let place = *context
                .places_by_symbol
                .get(&symbol.id)
                .ok_or(BuildError::MissingLocalSymbol)?;
            Ok((place.local, place.projection))
        }
        Expr::Member(member) => {
            let (place, representation) = super::projections::lower_borrow_field_place(
                member,
                context.semantic,
                &context.places_by_symbol,
                &mut context.projections,
                &mut context.drop_plans,
            )?;
            if representation != ValueRepresentation::Aggregate {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            Ok((place.local, place.projection))
        }
        _ => Err(BuildError::UnsupportedClaimedExpression),
    }
}

struct ArrayContract {
    length: u64,
    stride: u32,
    ty: crate::semantic::TyId,
    type_expr: crate::ast::TypeExpr,
    representation: ValueRepresentation,
    ownership: OwnershipKind,
}

fn array_contract(index: &IndexExpr, semantic: SemanticInputs<'_>) -> Option<ArrayContract> {
    let plan = semantic.index_plan(index.span)?;
    let abi =
        abi_value_from_type_expr_with_resolver(&plan.target_ty, semantic.resolved, |source| {
            semantic.resolver_for(source)
        })
        .ok()?;
    let AbiType::Array { element, length } = abi.ty else {
        return None;
    };
    let stride = u32::try_from(array_element_stride(&element).ok()?).ok()?;
    let ty = semantic.typed_hir.type_id(&plan.element_ty)?;
    let representation = super::source_model::value_representation(ty, semantic)?;
    let ownership = if super::super::drop_plans::is_copy(
        &plan.element_ty,
        semantic.resolved,
        semantic.resolved_sources,
    )? {
        OwnershipKind::Copy
    } else {
        OwnershipKind::Move
    };
    Some(ArrayContract {
        length,
        stride,
        ty,
        type_expr: plan.element_ty.clone(),
        representation,
        ownership,
    })
}
