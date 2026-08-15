//! Compiler-owned sequence-literal packs lowered into ordinary MIR control flow.
//!
//! A literal pack is not a runtime collection. It is a specialization-time
//! sequence of value parameters and exact-size iterator parameters. This
//! module is the only place that turns that semantic input into locals, calls,
//! and loop edges; neither buildability nor the machine backend needs a
//! source-shaped pack model.

use super::BuildError;
use super::context::LoweringContext;
use super::statements::{LoopTargets, StatementLowerer};
use crate::ast::{LiteralPackForStmt, TypeExpr};
use crate::mir::{
    BinaryOperator, LocalId, LocalOrigin, LocalStorage, Operand, Origin, Place, Rvalue, ScalarType,
    ScopeId, Statement, Terminator, ValueRepresentation,
};

#[derive(Debug, Clone)]
pub(crate) struct LiteralPackInput {
    pub(crate) capture_name: String,
    pub(crate) capture_span: crate::source::ByteSpan,
    pub(crate) element_type: TypeExpr,
    pub(crate) segments: Vec<LiteralPackInputSegment>,
}

#[derive(Debug, Clone)]
pub(crate) enum LiteralPackInputSegment {
    Value {
        parameter_index: usize,
    },
    Spread {
        parameter_index: usize,
        plan: crate::typecheck::TypecheckSequenceSpreadPlan,
    },
}

impl LiteralPackInput {
    pub(super) fn runtime_types(&self) -> Vec<TypeExpr> {
        let mut types = vec![self.element_type.clone()];
        for segment in &self.segments {
            let LiteralPackInputSegment::Spread { plan, .. } = segment else {
                continue;
            };
            types.extend([
                plan.source_type.clone(),
                plan.iterator_type.clone(),
                plan.iterator_item_type.clone(),
                plan.pack_item_type.clone(),
                TypeExpr::Optional(crate::ast::OptionalType {
                    span: plan.spread_span,
                    inner: Box::new(plan.iterator_item_type.clone()),
                }),
            ]);
            for method in plan.conversion.iter().chain([&plan.exact_size, &plan.step]) {
                types.push(method.self_ty.clone());
                if method.receiver_mode != crate::ast::MethodReceiverMode::Owned {
                    types.push(TypeExpr::Borrow(crate::ast::BorrowType {
                        span: plan.spread_span,
                        is_readwrite: method.receiver_mode
                            == crate::ast::MethodReceiverMode::ReadwriteBorrow,
                        inner: Box::new(method.self_ty.clone()),
                    }));
                }
            }
        }
        types
    }
}

#[derive(Debug, Clone)]
pub(super) struct PreparedLiteralPack {
    capture_name: String,
    length: LocalId,
    element_ty: crate::semantic::TyId,
    segments: Vec<PreparedLiteralPackSegment>,
}

#[derive(Debug, Clone)]
enum PreparedLiteralPackSegment {
    Value {
        parameter: LocalId,
    },
    Spread {
        parameter: LocalId,
        plan: crate::typecheck::TypecheckSequenceSpreadPlan,
    },
}

pub(super) fn prepare(
    context: &mut LoweringContext<'_>,
    input: LiteralPackInput,
    scope: ScopeId,
) -> Result<(), BuildError> {
    let usize_ty = context
        .semantic
        .typed_hir
        .type_id(&TypeExpr::Reference(crate::ast::TypeReference {
            span: input.capture_span,
            name: "usize".to_string(),
        }))
        .ok_or(BuildError::MissingTypedExpression)?;
    let element_ty = context
        .semantic
        .typed_hir
        .type_id(&input.element_type)
        .ok_or(BuildError::MissingTypedExpression)?;
    let length = LocalId::from_index(context.locals.len());
    context.locals.push(crate::mir::Local::scalar(
        usize_ty,
        ScalarType::Usize,
        LocalStorage::Local,
        LocalOrigin::Desugared(input.capture_span),
        scope,
    ));
    let fixed_count = input
        .segments
        .iter()
        .filter(|segment| matches!(segment, LiteralPackInputSegment::Value { .. }))
        .count() as u64;
    context.control_flow.push_statement(Statement::Assign {
        destination: Place::local(length),
        value: Rvalue::Use(Operand::Constant(crate::mir::Constant {
            ty: usize_ty,
            scalar: ScalarType::Usize,
            value: u128::from(fixed_count),
        })),
        origin: Origin::Desugared(input.capture_span),
    })?;

    let mut segments = Vec::with_capacity(input.segments.len());
    for segment in input.segments {
        let parameter_index = match &segment {
            LiteralPackInputSegment::Value { parameter_index }
            | LiteralPackInputSegment::Spread {
                parameter_index, ..
            } => *parameter_index,
        };
        let parameter = parameter_local(context, parameter_index)?;
        match segment {
            LiteralPackInputSegment::Value { .. } => {
                segments.push(PreparedLiteralPackSegment::Value { parameter });
            }
            LiteralPackInputSegment::Spread { plan, .. } => {
                let segment_length = context.local_for_type(
                    usize_ty,
                    LocalOrigin::Desugared(plan.spread_span),
                    scope,
                )?;
                let receiver = super::iteration::protocol_local_receiver(
                    context,
                    parameter,
                    &plan.exact_size,
                    scope,
                    Origin::Desugared(plan.spread_span),
                )?;
                let instance = super::iteration::protocol_instance(context, &plan.exact_size)?;
                let origin = context
                    .semantic
                    .resolved
                    .semantic_db
                    .expression_at(plan.source_span)
                    .or_else(|| {
                        context
                            .semantic
                            .resolved
                            .semantic_db
                            .expression_at(plan.spread_span)
                    })
                    .ok_or(BuildError::MissingTypedExpression)?;
                context.control_flow.emit_returning_call(
                    origin,
                    instance,
                    vec![receiver],
                    segment_length,
                )?;
                context.control_flow.push_statement(Statement::Assign {
                    destination: Place::local(length),
                    value: Rvalue::Binary {
                        operator: BinaryOperator::Add,
                        left: Operand::Copy(Place::local(length)),
                        right: Operand::Copy(Place::local(segment_length)),
                        ty: usize_ty,
                    },
                    origin: Origin::Desugared(plan.spread_span),
                })?;
                segments.push(PreparedLiteralPackSegment::Spread { parameter, plan });
            }
        }
    }
    context.literal_pack = Some(PreparedLiteralPack {
        capture_name: input.capture_name,
        length,
        element_ty,
        segments,
    });
    Ok(())
}

fn parameter_local(
    context: &LoweringContext<'_>,
    parameter_index: usize,
) -> Result<LocalId, BuildError> {
    context
        .locals
        .iter()
        .enumerate()
        .find_map(|(index, local)| {
            matches!(local.storage, LocalStorage::Parameter { ordinal } if ordinal == parameter_index)
                .then(|| LocalId::from_index(index))
        })
        .ok_or(BuildError::MissingParameterType)
}

pub(super) fn length_operand(
    context: &LoweringContext<'_>,
    call: &crate::ast::CallExpr,
) -> Option<Operand> {
    let pack = context.literal_pack.as_ref()?;
    let crate::ast::Expr::Member(member) = call.callee.without_groups() else {
        return None;
    };
    if member.member != "len" || !call.arguments.is_empty() {
        return None;
    }
    let crate::ast::Expr::Identifier(identifier) = member.object.without_groups() else {
        return None;
    };
    (identifier.name == pack.capture_name).then(|| Operand::Copy(Place::local(pack.length)))
}

pub(super) fn lower(
    context: &mut LoweringContext<'_>,
    statement: &LiteralPackForStmt,
    parent_scope: ScopeId,
) -> Result<bool, BuildError> {
    let pack = context
        .literal_pack
        .clone()
        .filter(|pack| pack.capture_name == statement.pack_name)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let item_symbol = context
        .semantic
        .resolved
        .local_symbol_id_at_name_span(statement.name_span)
        .ok_or(BuildError::MissingLocalSymbol)?;
    let exit = context.control_flow.reserve_block(parent_scope);

    for segment in &pack.segments {
        match segment {
            PreparedLiteralPackSegment::Value { parameter } => {
                let iteration_scope = context.child_scope(parent_scope, statement.body.span);
                let iteration = context.control_flow.reserve_block(iteration_scope);
                let following = context.control_flow.reserve_block(parent_scope);
                context
                    .control_flow
                    .terminate(Terminator::Goto { target: iteration })?;
                context.control_flow.select_block(iteration)?;
                let item = context.local_for_type(
                    pack.element_ty,
                    LocalOrigin::Binding(item_symbol),
                    iteration_scope,
                )?;
                context
                    .places_by_symbol
                    .insert(item_symbol, Place::local(item));
                let operand = operand_for_local(context, *parameter);
                context.control_flow.push_statement(Statement::Assign {
                    destination: Place::local(item),
                    value: Rvalue::Use(operand),
                    origin: Origin::Desugared(statement.pack_span),
                })?;
                let body = super::source_model::scalar_loop_block_statements(
                    &statement.body,
                    context.semantic.resolved,
                    context.semantic.resolved_sources,
                    context.semantic.typed_hir,
                )
                .ok_or(BuildError::UnsupportedClaimedExpression)
                .map_err(|error| error.context("classify literal-pack value body"))?;
                let exits = StatementLowerer::new(context)
                    .lower_in_context(
                        &body,
                        Some(LoopTargets {
                            break_target: exit,
                            continue_target: following,
                        }),
                        iteration_scope,
                    )
                    .map_err(|error| error.context("lower literal-pack value body"))?;
                if !exits {
                    context
                        .control_flow
                        .terminate(Terminator::Goto { target: following })?;
                }
                context.control_flow.select_block(following)?;
            }
            PreparedLiteralPackSegment::Spread { parameter, plan } => {
                lower_spread_segment(
                    context,
                    statement,
                    *parameter,
                    plan,
                    pack.element_ty,
                    item_symbol,
                    parent_scope,
                    exit,
                )
                .map_err(|error| error.context("lower literal-pack spread segment"))?;
            }
        }
    }
    context
        .control_flow
        .terminate(Terminator::Goto { target: exit })?;
    context.control_flow.select_block(exit)?;
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
fn lower_spread_segment(
    context: &mut LoweringContext<'_>,
    statement: &LiteralPackForStmt,
    iterator: LocalId,
    plan: &crate::typecheck::TypecheckSequenceSpreadPlan,
    pack_item_ty: crate::semantic::TyId,
    item_symbol: crate::resolve::LocalSymbolId,
    parent_scope: ScopeId,
    pack_exit: crate::mir::BasicBlockId,
) -> Result<(), BuildError> {
    let iterator_item_ty = context
        .semantic
        .typed_hir
        .type_id(&plan.iterator_item_type)
        .ok_or(BuildError::MissingTypedExpression)?;
    let optional_ty = context
        .semantic
        .typed_hir
        .type_id(&TypeExpr::Optional(crate::ast::OptionalType {
            span: plan.spread_span,
            inner: Box::new(plan.iterator_item_type.clone()),
        }))
        .ok_or(BuildError::MissingTypedExpression)?;
    let loop_scope = context.child_scope(parent_scope, statement.span);
    let iteration_scope = context.child_scope(loop_scope, statement.body.span);
    let yielded = context.local_for_type(
        iterator_item_ty,
        LocalOrigin::Desugared(statement.name_span),
        iteration_scope,
    )?;
    let item = if plan.mode == crate::typecheck::TypecheckSequenceSpreadMode::Copy {
        context.local_for_type(
            pack_item_ty,
            LocalOrigin::Binding(item_symbol),
            iteration_scope,
        )?
    } else {
        yielded
    };
    context
        .places_by_symbol
        .insert(item_symbol, Place::local(item));
    let outcome = context.aggregate_temporary(
        optional_ty,
        LocalOrigin::Desugared(plan.spread_span),
        loop_scope,
    )?;
    let header = context.control_flow.reserve_block(loop_scope);
    let body = context.control_flow.reserve_block(iteration_scope);
    let following = context.control_flow.reserve_block(parent_scope);
    context
        .control_flow
        .terminate(Terminator::Goto { target: header })?;
    context.control_flow.select_block(header)?;
    let receiver = super::iteration::protocol_local_receiver(
        context,
        iterator,
        &plan.step,
        loop_scope,
        Origin::Desugared(plan.spread_span),
    )?;
    let instance = super::iteration::protocol_instance(context, &plan.step)?;
    let origin = context
        .semantic
        .resolved
        .semantic_db
        .expression_at(plan.source_span)
        .or_else(|| {
            context
                .semantic
                .resolved
                .semantic_db
                .expression_at(plan.spread_span)
        })
        .ok_or(BuildError::MissingTypedExpression)?;
    context
        .control_flow
        .emit_returning_call(origin, instance, vec![receiver], outcome)?;
    let condition = context.control_flow.current_block()?;
    context.control_flow.terminate(Terminator::InspectOutcome {
        origin: Origin::Desugared(plan.spread_span),
        source: operand_for_local(context, outcome),
        layer: crate::outcomes::OutcomeLayer::Optional,
        destination: Place::local(yielded),
        success: body,
        failure: following,
        failure_payload: None,
    })?;
    context.control_flow.select_block(body)?;
    if plan.mode == crate::typecheck::TypecheckSequenceSpreadMode::Copy {
        let source = dereferenced_place(context, yielded, pack_item_ty)?;
        context.control_flow.push_statement(Statement::Assign {
            destination: Place::local(item),
            value: Rvalue::Use(Operand::Copy(source)),
            origin: Origin::Desugared(plan.spread_span),
        })?;
    }
    let statements = super::source_model::scalar_loop_block_statements(
        &statement.body,
        context.semantic.resolved,
        context.semantic.resolved_sources,
        context.semantic.typed_hir,
    )
    .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let exits = StatementLowerer::new(context).lower_in_context(
        &statements,
        Some(LoopTargets {
            break_target: pack_exit,
            continue_target: header,
        }),
        iteration_scope,
    )?;
    if !exits {
        context
            .control_flow
            .terminate(Terminator::Goto { target: header })?;
    }
    context.loop_regions.push(crate::mir::LoopRegion {
        header,
        condition,
        body,
        continue_target: header,
        exit: following,
    });
    context.control_flow.select_block(following)
}

fn dereferenced_place(
    context: &mut LoweringContext<'_>,
    local: LocalId,
    ty: crate::semantic::TyId,
) -> Result<Place, BuildError> {
    if context.locals[local.index()].representation != ValueRepresentation::Borrow {
        return Err(BuildError::UnsupportedClaimedExpression);
    }
    let projection = crate::mir::ProjectionPathId::from_index(context.projections.len());
    context.projections.push(crate::mir::ProjectionPath {
        id: projection,
        base: local,
        parent: None,
        element: crate::mir::ProjectionElement::Dereference,
        ty,
        representation: super::source_model::value_representation(ty, context.semantic)
            .ok_or(BuildError::MissingTypedExpression)?,
        ownership: crate::mir::OwnershipKind::Copy,
        drop_plan: None,
    });
    Ok(Place::projected(local, projection))
}

fn operand_for_local(context: &LoweringContext<'_>, local: LocalId) -> Operand {
    if context.locals[local.index()].ownership == crate::mir::OwnershipKind::Move {
        Operand::Move(Place::local(local))
    } else {
        Operand::Copy(Place::local(local))
    }
}
