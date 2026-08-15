//! Closure callable preparation for the common MIR body builder.
//!
//! Captures are semantic places projected from the environment receiver. No
//! synthetic method declaration or capture binding is created.

use super::source_model::*;
use super::{BuildError, BuildInputs, SemanticInputs, build_prepared_body, type_representation};
use crate::ast::{ClosureCaptureMode, ClosureExpr, MethodReceiverMode, TypeExpr};
use crate::mir::{
    Local, LocalId, LocalOrigin, LocalStorage, OwnershipKind, Place, ProjectionElement,
    ProjectionPath, ProjectionPathId, ReturnMode, ScopeId, ValueRepresentation,
};
use std::collections::HashMap;

pub(crate) fn build_closure_body(
    expression: &ClosureExpr,
    closure_ty: &crate::ast::ClosureTypeExpr,
    receiver_mode: MethodReceiverMode,
    return_representation: ValueRepresentation,
    return_mode: ReturnMode,
    inputs: BuildInputs<'_>,
) -> Result<crate::mir::Body, BuildError> {
    let semantic = SemanticInputs {
        resolved: inputs.resolved,
        resolved_sources: inputs.resolved_sources,
        typed_hir: inputs.typed_hir,
    };
    let (source_statements, tail) =
        scalar_body_parts(&expression.body).ok_or(BuildError::UnsupportedClaimedExpression)?;
    let contextual_return_ty = tail
        .result_type(inputs.typed_hir)
        .ok_or(BuildError::MissingTypedExpression)?;
    if value_representation(contextual_return_ty, semantic) != Some(return_representation) {
        return Err(BuildError::ClosurePreparation("return representation"));
    }
    if expression.parameters.len() != closure_ty.parameters.len() {
        return Err(BuildError::ClosurePreparation("parameter arity"));
    }
    if expression.captures.len() != closure_ty.captures.len() {
        return Err(BuildError::ClosurePreparation("capture arity"));
    }
    let return_ty = tail
        .expression()
        .and_then(|value| intrinsic_expression_type(value.span(), inputs.typed_hir))
        .filter(|ty| value_representation(*ty, semantic) == Some(return_representation))
        .unwrap_or(contextual_return_ty);

    (|| {
        let source_body = inputs
            .semantic_db
            .body_at(expression.body.span)
            .ok_or(BuildError::MissingSourceBody)?;
        let root_scope = ScopeId::from_index(0);
        let mut drop_plans = Vec::new();
        let return_type_expr = inputs
            .typed_hir
            .type_expr_by_id(return_ty)
            .ok_or(BuildError::ClosurePreparation("return type"))?;
        let return_local = local_contract(
            return_ty,
            return_type_expr,
            return_representation,
            LocalStorage::Return,
            LocalOrigin::Return,
            root_scope,
            semantic,
            &mut drop_plans,
        )?;
        let receiver_type = match receiver_mode {
            MethodReceiverMode::Owned => TypeExpr::Closure(closure_ty.clone()),
            MethodReceiverMode::ReadonlyBorrow | MethodReceiverMode::ReadwriteBorrow => {
                TypeExpr::Borrow(crate::ast::BorrowType {
                    span: expression.parameters_span,
                    is_readwrite: receiver_mode == MethodReceiverMode::ReadwriteBorrow,
                    inner: Box::new(TypeExpr::Closure(closure_ty.clone())),
                })
            }
        };
        let receiver_ty = inputs
            .typed_hir
            .type_id(&receiver_type)
            .ok_or(BuildError::MissingParameterType)?;
        let receiver_representation = if matches!(receiver_type, TypeExpr::Borrow(_)) {
            ValueRepresentation::Borrow
        } else {
            ValueRepresentation::Aggregate
        };
        let receiver = local_contract(
            receiver_ty,
            &receiver_type,
            receiver_representation,
            LocalStorage::Parameter { ordinal: 0 },
            LocalOrigin::CallableReceiver(
                inputs
                    .typed_hir
                    .expression(expression.span)
                    .ok_or(BuildError::ClosurePreparation(
                        "closure expression identity",
                    ))?
                    .id,
            ),
            root_scope,
            semantic,
            &mut drop_plans,
        )?;
        let receiver_id = LocalId::from_index(1);
        let mut locals = vec![return_local, receiver];
        let mut places = HashMap::new();
        let mut projections = Vec::new();
        let mut prologue = Vec::new();
        prepare_capture_places(
            expression,
            closure_ty,
            receiver_id,
            semantic,
            &mut places,
            &mut projections,
            &mut drop_plans,
            &mut locals,
            &mut prologue,
        )?;
        for (index, (parameter, ty)) in expression
            .parameters
            .iter()
            .zip(&closure_ty.parameters)
            .enumerate()
        {
            let symbol = inputs
                .resolved
                .local_symbol_id_at_name_span(parameter.name_span)
                .ok_or(BuildError::MissingLocalSymbol)?;
            let parameter_ty = inputs.typed_hir.binding_type_expr(symbol).unwrap_or(ty);
            let ty_id = inputs
                .typed_hir
                .type_id(parameter_ty)
                .ok_or(BuildError::MissingParameterType)?;
            let representation = type_representation(parameter_ty, semantic)
                .ok_or(BuildError::ClosurePreparation("parameter representation"))?;
            let local_id = LocalId::from_index(locals.len());
            locals.push(local_contract(
                ty_id,
                parameter_ty,
                representation,
                LocalStorage::Parameter { ordinal: index + 1 },
                LocalOrigin::Parameter(symbol),
                root_scope,
                semantic,
                &mut drop_plans,
            )?);
            places.insert(symbol, Place::local(local_id));
        }
        let body = build_prepared_body(
            &expression.body,
            source_statements,
            tail,
            contextual_return_ty,
            inputs.declared_return_ty.unwrap_or(contextual_return_ty),
            &inputs.outcome_layers,
            return_ty,
            return_representation,
            return_mode,
            source_body,
            semantic,
            locals,
            places,
            drop_plans,
            projections,
            prologue,
            None,
        )
        .map_err(|error| BuildError::ClosureBody(Box::new(error)))?;
        Ok(body)
    })()
}

#[allow(clippy::too_many_arguments)]
fn local_contract(
    ty: crate::semantic::TyId,
    type_expr: &TypeExpr,
    representation: ValueRepresentation,
    storage: LocalStorage,
    origin: LocalOrigin,
    scope: ScopeId,
    semantic: SemanticInputs<'_>,
    drop_plans: &mut Vec<crate::mir::DropPlan>,
) -> Result<Local, BuildError> {
    let ownership = if let TypeExpr::Borrow(borrow) = type_expr {
        OwnershipKind::Borrowed {
            readwrite: borrow.is_readwrite,
        }
    } else if super::super::drop_plans::is_copy(
        type_expr,
        semantic.resolved,
        semantic.resolved_sources,
    ) == Some(true)
    {
        OwnershipKind::Copy
    } else {
        OwnershipKind::Move
    };
    let mut local = match representation {
        ValueRepresentation::Unit => Local::unit(ty, storage, origin, scope),
        ValueRepresentation::Scalar(scalar) => Local::scalar(ty, scalar, storage, origin, scope),
        ValueRepresentation::View(kind) => Local::view(ty, kind, storage, origin, scope),
        ValueRepresentation::Aggregate => Local::aggregate(ty, ownership, storage, origin, scope),
        ValueRepresentation::Borrow => {
            let OwnershipKind::Borrowed { readwrite } = ownership else {
                return Err(BuildError::ClosurePreparation("borrow local contract"));
            };
            Local::borrow(ty, readwrite, storage, origin, scope)
        }
        ValueRepresentation::Error => Local::error(ty, storage, origin, scope),
    };
    if local.ownership == OwnershipKind::Move {
        local.drop_plan = Some(
            super::super::drop_plans::build(
                type_expr,
                semantic.resolved,
                semantic.resolved_sources,
                semantic.typed_hir,
                drop_plans,
            )
            .ok_or(BuildError::ClosurePreparation("owned local drop plan"))?,
        );
    }
    Ok(local)
}

fn prepare_capture_places(
    expression: &ClosureExpr,
    closure_ty: &crate::ast::ClosureTypeExpr,
    receiver: LocalId,
    semantic: SemanticInputs<'_>,
    places: &mut HashMap<crate::resolve::LocalSymbolId, Place>,
    projections: &mut Vec<ProjectionPath>,
    drop_plans: &mut Vec<crate::mir::DropPlan>,
    locals: &mut Vec<Local>,
    prologue: &mut Vec<crate::mir::Statement>,
) -> Result<(), BuildError> {
    let abi = crate::abi::abi_value_from_type_expr_with_resolver(
        &TypeExpr::Closure(closure_ty.clone()),
        semantic.resolved,
        |source| semantic.resolver_for(source),
    )
    .map_err(|_| BuildError::ClosurePreparation("environment ABI"))?;
    let crate::abi::AbiType::Struct(fields) = abi.ty else {
        return Err(BuildError::ClosurePreparation("environment ABI kind"));
    };
    let layout = crate::abi::layout_struct(&fields)
        .map_err(|_| BuildError::ClosurePreparation("environment layout"))?;
    for (index, (capture, capture_ty)) in expression
        .captures
        .iter()
        .zip(&closure_ty.captures)
        .enumerate()
    {
        if capture.name != capture_ty.name || capture.mode != capture_ty.mode {
            return Err(BuildError::ClosurePreparation("capture plan mismatch"));
        }
        let symbol = semantic
            .resolved
            .local_symbol_id_at_name_span(capture.name_span)
            .ok_or(BuildError::MissingLocalSymbol)?;
        let ty = semantic
            .typed_hir
            .type_id(&capture_ty.ty)
            .ok_or(BuildError::ClosurePreparation("capture field type"))?;
        let representation = type_representation(&capture_ty.ty, semantic)
            .or_else(|| {
                matches!(capture_ty.ty, TypeExpr::Borrow(_)).then_some(ValueRepresentation::Borrow)
            })
            .ok_or(BuildError::ClosurePreparation("capture representation"))?;
        let ownership = match capture.mode {
            ClosureCaptureMode::ReadonlyBorrow => OwnershipKind::Borrowed { readwrite: false },
            ClosureCaptureMode::ReadwriteBorrow => OwnershipKind::Borrowed { readwrite: true },
            ClosureCaptureMode::Move
                if super::super::drop_plans::is_copy(
                    &capture_ty.ty,
                    semantic.resolved,
                    semantic.resolved_sources,
                ) == Some(true) =>
            {
                OwnershipKind::Copy
            }
            ClosureCaptureMode::Move => OwnershipKind::Move,
        };
        let drop_plan = if ownership == OwnershipKind::Move {
            Some(
                super::super::drop_plans::build(
                    &capture_ty.ty,
                    semantic.resolved,
                    semantic.resolved_sources,
                    semantic.typed_hir,
                    drop_plans,
                )
                .ok_or(BuildError::ClosurePreparation("capture drop plan"))?,
            )
        } else {
            None
        };
        let field = ProjectionPathId::from_index(projections.len());
        projections.push(ProjectionPath {
            id: field,
            base: receiver,
            parent: None,
            element: ProjectionElement::Field {
                offset: u32::try_from(layout.fields[index].offset)
                    .map_err(|_| BuildError::ClosurePreparation("capture field offset"))?,
            },
            ty,
            representation,
            ownership,
            drop_plan,
        });
        let field_place = Place::projected(receiver, field);
        if matches!(
            capture.mode,
            ClosureCaptureMode::ReadonlyBorrow | ClosureCaptureMode::ReadwriteBorrow
        ) {
            let TypeExpr::Borrow(borrow) = &capture_ty.ty else {
                return Err(BuildError::ClosurePreparation("borrow capture type"));
            };
            let temporary = LocalId::from_index(locals.len());
            locals.push(Local::borrow(
                ty,
                borrow.is_readwrite,
                LocalStorage::Local,
                LocalOrigin::Desugared(capture.span),
                ScopeId::from_index(0),
            ));
            prologue.push(crate::mir::Statement::Assign {
                destination: Place::local(temporary),
                value: crate::mir::Rvalue::Use(if borrow.is_readwrite {
                    crate::mir::Operand::Move(field_place)
                } else {
                    crate::mir::Operand::Copy(field_place)
                }),
                origin: crate::mir::Origin::Desugared(capture.span),
            });
            let inner_ty = semantic
                .typed_hir
                .type_id(&borrow.inner)
                .ok_or(BuildError::ClosurePreparation("capture target type"))?;
            let inner_representation = type_representation(&borrow.inner, semantic).ok_or(
                BuildError::ClosurePreparation("capture target representation"),
            )?;
            let dereference = ProjectionPathId::from_index(projections.len());
            projections.push(ProjectionPath {
                id: dereference,
                base: temporary,
                parent: None,
                element: ProjectionElement::Dereference,
                ty: inner_ty,
                representation: inner_representation,
                ownership: if inner_representation == ValueRepresentation::Aggregate {
                    OwnershipKind::Borrowed { readwrite: false }
                } else {
                    OwnershipKind::Copy
                },
                drop_plan: None,
            });
            places.insert(symbol, Place::projected(temporary, dereference));
        } else {
            places.insert(symbol, field_place);
        }
    }
    Ok(())
}
