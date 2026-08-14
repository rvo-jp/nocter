//! Scalar body selection and control-flow construction from typed HIR.

use super::ids::{BasicBlockId, LocalId};
use super::locals::{Local, LocalOrigin, LocalStorage, OwnershipKind, ScalarType};
use super::model::{Body, ReturnMode, Terminator};
#[cfg(test)]
use super::validate;
use super::validate::ValidationError;
use super::{Scope, ScopeId};
use crate::ast::{Block, Expr, Parameter};
use crate::resolve::{ResolveOutput, ResolvedSources};
use crate::semantic::SemanticDb;
use crate::typecheck::TypedHir;
use std::collections::HashMap;

mod aggregates;
mod body_builder;
mod borrows;
mod closures;
mod context;
mod coverage;
mod explicit_drops;
mod expressions;
mod indexes;
mod interpolation;
mod iteration;
mod literals;
mod projections;
mod regions;
mod statements;
use context::LoweringContext;
use coverage::*;
use statements::StatementLowerer;

#[derive(Debug, Clone, Copy)]
struct SemanticInputs<'a> {
    resolved: &'a ResolveOutput,
    resolved_sources: &'a ResolvedSources<'a>,
    typed_hir: &'a TypedHir,
}

impl<'a> SemanticInputs<'a> {
    fn resolver_for(self, source: crate::source::SourceId) -> Option<&'a ResolveOutput> {
        self.resolved_sources.get(&source).copied()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BuildInputs<'a> {
    pub(crate) semantic_db: &'a SemanticDb,
    pub(crate) resolved: &'a ResolveOutput,
    pub(crate) resolved_sources: &'a ResolvedSources<'a>,
    pub(crate) typed_hir: &'a TypedHir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BuildError {
    MissingSourceBody,
    MissingTypedExpression,
    MissingSpecializedReceiverType,
    MissingMethodReceiverType,
    MissingCallExpression,
    InvalidScalarConstant,
    MissingLocalSymbol,
    MissingParameterType,
    MissingCallTarget,
    MissingOpenBlock,
    OpenBlockNotTerminated,
    BlockAlreadyTerminated,
    UnterminatedReservedBlock,
    UnsupportedClaimedExpression,
    ClosurePreparation(&'static str),
    ClosureBody(Box<BuildError>),
    InvalidMir(Vec<ValidationError>),
}

#[cfg(test)]
fn try_build_scalar_body(
    block: &Block,
    parameters: &[Parameter],
    return_scalar: ScalarType,
    semantic_db: &SemanticDb,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> Option<Result<Body, BuildError>> {
    let resolved_sources = ResolvedSources::new();
    try_build_scalar_body_with_return_mode(
        block,
        parameters,
        return_scalar,
        ReturnMode::Plain,
        BuildInputs {
            semantic_db,
            resolved,
            resolved_sources: &resolved_sources,
            typed_hir,
        },
    )
}

#[cfg(test)]
fn try_build_scalar_body_with_return_mode(
    block: &Block,
    parameters: &[Parameter],
    return_scalar: ScalarType,
    return_mode: ReturnMode,
    inputs: BuildInputs<'_>,
) -> Option<Result<Body, BuildError>> {
    try_build_body_with_return_mode(
        block,
        parameters,
        super::ValueRepresentation::Scalar(return_scalar),
        return_mode,
        inputs,
    )
}

pub(crate) fn try_build_body_with_return_mode(
    block: &Block,
    parameters: &[Parameter],
    return_representation: super::ValueRepresentation,
    return_mode: ReturnMode,
    inputs: BuildInputs<'_>,
) -> Option<Result<Body, BuildError>> {
    let semantic = SemanticInputs {
        resolved: inputs.resolved,
        resolved_sources: inputs.resolved_sources,
        typed_hir: inputs.typed_hir,
    };
    let (mut source_statements, mut tail) = scalar_body_parts(block)?;
    if coverage::block_contains_repeating_explicit_drop(block) {
        return None;
    }
    if return_representation == super::ValueRepresentation::Unit
        && let Some(if_) = tail.conditional()
    {
        source_statements.push(ScalarStatement::If(if_));
        tail = ScalarTail::ImplicitUnit(block.span);
    }
    let contextual_return_ty = if matches!(tail, ScalarTail::ImplicitUnit(_))
        && return_representation != super::ValueRepresentation::Unit
    {
        coverage::terminal_return_type(block, inputs.typed_hir)?
    } else {
        tail.result_type(inputs.typed_hir)?
    };
    if value_representation(contextual_return_ty, semantic) != Some(return_representation)
        || !source_statements
            .iter()
            .all(|statement| statement.is_supported(semantic))
        || !(return_representation == super::ValueRepresentation::Unit
            && match tail {
                ScalarTail::ImplicitUnit(_) | ScalarTail::UnitReturn(_) => true,
                ScalarTail::Expression(expression) | ScalarTail::Return(expression) => {
                    ScalarStatement::Expression(expression).is_supported(semantic)
                }
                ScalarTail::Conditional(_) => false,
            }
            || matches!(tail, ScalarTail::ImplicitUnit(_))
                && return_representation != super::ValueRepresentation::Unit
                && coverage::terminal_return_type(block, inputs.typed_hir).is_some()
            || tail.expression().is_some_and(|expression| {
                inputs
                    .typed_hir
                    .expression(expression.span())
                    .is_some_and(|expression| expression.diverges)
            })
            || tail.expression().is_some_and(|expression| {
                value_expression_is_supported(expression, return_representation, semantic)
                    || coverage::outcome_return_expression_is_supported(
                        expression,
                        contextual_return_ty,
                        semantic,
                    )
                    || return_mode == ReturnMode::Fallible
                        && coverage::failure_value_is_supported(expression, semantic)
            })
            || tail.conditional().is_some_and(|conditional| {
                (matches!(
                    return_representation,
                    super::ValueRepresentation::Scalar(_) | super::ValueRepresentation::View(_)
                ) || matches!(
                    return_representation,
                    super::ValueRepresentation::Aggregate | super::ValueRepresentation::Unit
                )) && value_conditional_is_supported(conditional, return_representation, semantic)
                    || coverage::outcome_return_conditional_is_supported(
                        conditional,
                        contextual_return_ty,
                        semantic,
                    )
            }))
        || !parameters.iter().all(|parameter| {
            inputs
                .resolved
                .local_symbol_id_at_name_span(parameter.name_span)
                .is_some()
                && inputs.typed_hir.type_id(&parameter.ty).is_some()
                && parameter_representation(parameter, semantic).is_some()
        })
    {
        return None;
    }
    let return_ty = tail
        .expression()
        .and_then(|expression| intrinsic_expression_type(expression.span(), inputs.typed_hir))
        .filter(|ty| value_representation(*ty, semantic) == Some(return_representation))
        .unwrap_or(contextual_return_ty);

    Some((|| {
        let source_body = inputs
            .semantic_db
            .body_at(block.span)
            .ok_or(BuildError::MissingSourceBody)?;
        let root_scope = ScopeId::from_index(0);
        let mut drop_plans = Vec::new();
        let mut return_local_contract = match return_representation {
            super::ValueRepresentation::Unit => Local::unit(
                return_ty,
                LocalStorage::Return,
                LocalOrigin::Return,
                root_scope,
            ),
            super::ValueRepresentation::Scalar(scalar) => Local::scalar(
                return_ty,
                scalar,
                LocalStorage::Return,
                LocalOrigin::Return,
                root_scope,
            ),
            super::ValueRepresentation::View(kind) => Local::view(
                return_ty,
                kind,
                LocalStorage::Return,
                LocalOrigin::Return,
                root_scope,
            ),
            super::ValueRepresentation::Aggregate => {
                let return_type_expr = inputs
                    .typed_hir
                    .type_expr_by_id(return_ty)
                    .ok_or(BuildError::MissingTypedExpression)?;
                let ownership =
                    if crate::typecheck::type_expr_is_copy(return_type_expr, inputs.resolved)
                        == Some(true)
                    {
                        OwnershipKind::Copy
                    } else {
                        OwnershipKind::Move
                    };
                Local::aggregate(
                    return_ty,
                    ownership,
                    LocalStorage::Return,
                    LocalOrigin::Return,
                    root_scope,
                )
            }
            super::ValueRepresentation::Borrow | super::ValueRepresentation::Error => {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
        };
        if return_local_contract.ownership == OwnershipKind::Move {
            let return_type_expr = inputs
                .typed_hir
                .type_expr_by_id(return_ty)
                .ok_or(BuildError::MissingTypedExpression)?;
            return_local_contract.drop_plan = Some(
                super::drop_plans::build(
                    return_type_expr,
                    inputs.resolved,
                    inputs.resolved_sources,
                    inputs.typed_hir,
                    &mut drop_plans,
                )
                .ok_or(BuildError::UnsupportedClaimedExpression)?,
            );
        }
        let mut locals = vec![return_local_contract];
        let mut places_by_symbol = HashMap::new();
        for (index, parameter) in parameters.iter().enumerate() {
            let symbol = inputs
                .resolved
                .local_symbol_id_at_name_span(parameter.name_span)
                .ok_or(BuildError::MissingLocalSymbol)?;
            let ty = inputs
                .typed_hir
                .binding_type_expr(symbol)
                .and_then(|ty| inputs.typed_hir.type_id(ty))
                .ok_or(BuildError::MissingParameterType)?;
            let local = LocalId::from_index(locals.len());
            let storage = LocalStorage::Parameter { ordinal: index };
            let origin = LocalOrigin::Parameter(symbol);
            let mut local_contract = match parameter_representation(parameter, semantic)
                .ok_or(BuildError::UnsupportedClaimedExpression)?
            {
                super::ValueRepresentation::Unit => Local::unit(ty, storage, origin, root_scope),
                super::ValueRepresentation::Scalar(scalar) => {
                    Local::scalar(ty, scalar, storage, origin, root_scope)
                }
                super::ValueRepresentation::Aggregate => {
                    let ownership =
                        if crate::typecheck::type_expr_is_copy(&parameter.ty, inputs.resolved)
                            .unwrap_or(false)
                        {
                            OwnershipKind::Copy
                        } else {
                            OwnershipKind::Move
                        };
                    Local::aggregate(ty, ownership, storage, origin, root_scope)
                }
                super::ValueRepresentation::Borrow => {
                    let crate::ast::TypeExpr::Borrow(borrow) = &parameter.ty else {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    };
                    Local::borrow(ty, borrow.is_readwrite, storage, origin, root_scope)
                }
                super::ValueRepresentation::Error => Local::error(ty, storage, origin, root_scope),
                super::ValueRepresentation::View(kind) => {
                    Local::view(ty, kind, storage, origin, root_scope)
                }
            };
            if local_contract.ownership == OwnershipKind::Move {
                local_contract.drop_plan = Some(
                    super::drop_plans::build(
                        &parameter.ty,
                        inputs.resolved,
                        inputs.resolved_sources,
                        inputs.typed_hir,
                        &mut drop_plans,
                    )
                    .ok_or(BuildError::UnsupportedClaimedExpression)?,
                );
            }
            locals.push(local_contract);
            places_by_symbol.insert(symbol, crate::mir::Place::local(local));
        }
        build_prepared_body(
            block,
            source_statements,
            tail,
            contextual_return_ty,
            return_ty,
            return_representation,
            return_mode,
            source_body,
            semantic,
            locals,
            places_by_symbol,
            drop_plans,
            Vec::new(),
            Vec::new(),
        )
    })())
}

#[allow(clippy::too_many_arguments)]
fn build_prepared_body(
    block: &Block,
    source_statements: Vec<ScalarStatement<'_>>,
    tail: ScalarTail<'_>,
    contextual_return_ty: crate::semantic::TyId,
    return_ty: crate::semantic::TyId,
    return_representation: super::ValueRepresentation,
    return_mode: ReturnMode,
    source_body: crate::semantic::BodyId,
    semantic: SemanticInputs<'_>,
    locals: Vec<Local>,
    places_by_symbol: HashMap<crate::resolve::LocalSymbolId, crate::mir::Place>,
    drop_plans: Vec<super::DropPlan>,
    projections: Vec<super::ProjectionPath>,
    prologue: Vec<super::Statement>,
) -> Result<Body, BuildError> {
    let return_local = LocalId::from_index(0);
    let root_scope = ScopeId::from_index(0);
    let mut context = LoweringContext::new(
        semantic,
        locals,
        places_by_symbol,
        drop_plans,
        projections,
        root_scope,
        Scope::root(block.span),
    );
    for statement in prologue {
        context.control_flow.push_statement(statement)?;
    }
    let source_exits = StatementLowerer::new(&mut context).lower(&source_statements, root_scope)?;
    if !source_exits {
        if return_mode == ReturnMode::Fallible
            && tail.expression().is_some_and(|expression| {
                coverage::failure_value_is_supported(expression, semantic)
            })
        {
            context.lower_failure_return(
                tail.expression()
                    .ok_or(BuildError::UnsupportedClaimedExpression)?,
                root_scope,
            )?;
        } else if tail.expression().is_some_and(|expression| {
            coverage::outcome_return_expression_is_supported(
                expression,
                contextual_return_ty,
                semantic,
            )
        }) {
            context.lower_direct_outcome_return(
                tail.expression()
                    .ok_or(BuildError::UnsupportedClaimedExpression)?,
                root_scope,
            )?;
        } else if return_representation == super::ValueRepresentation::Unit
            && tail.conditional().is_none()
        {
            if let Some(expression) = tail.expression() {
                statements::StatementLowerer::new(&mut context)
                    .lower(&[ScalarStatement::Expression(expression)], root_scope)?;
            }
            context.control_flow.terminate(Terminator::Return)?;
        } else if let Some(if_) = tail.conditional()
            && coverage::outcome_return_conditional_is_supported(
                if_,
                contextual_return_ty,
                semantic,
            )
        {
            let exits = StatementLowerer::new(&mut context)
                .lower(&[ScalarStatement::If(if_)], root_scope)?;
            if !exits {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
        } else if let Some(if_) = tail.conditional() {
            expressions::lower_conditional_to_place(
                &mut context,
                return_local,
                if_,
                return_ty,
                return_representation,
                root_scope,
            )?;
            context.control_flow.terminate(Terminator::Return)?;
        } else if let Some(Expr::Call(call)) = tail.expression() {
            let source = semantic
                .typed_hir
                .expression(call.span)
                .ok_or(BuildError::MissingTypedExpression)?
                .id;
            let (callee, arguments, returns_never) = context.lower_call(call, root_scope)?;
            if returns_never {
                context
                    .control_flow
                    .emit_never_call(source, callee, arguments)?;
            } else {
                context.control_flow.emit_returning_call(
                    source,
                    callee,
                    arguments,
                    return_local,
                )?;
                context.control_flow.terminate(Terminator::Return)?;
            }
        } else {
            let expression = tail
                .expression()
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            match return_representation {
                super::ValueRepresentation::Unit => unreachable!("unit returns terminate above"),
                super::ValueRepresentation::Scalar(return_scalar) => context
                    .lower_expression_to_place(
                        return_local,
                        expression,
                        return_ty,
                        return_scalar,
                        root_scope,
                    )?,
                super::ValueRepresentation::View(kind) => context.lower_view_expression_to_place(
                    return_local,
                    expression,
                    return_ty,
                    kind,
                    root_scope,
                )?,
                super::ValueRepresentation::Aggregate => {
                    let return_type_expr = semantic
                        .typed_hir
                        .type_expr_by_id(contextual_return_ty)
                        .ok_or(BuildError::MissingTypedExpression)?;
                    let returns_stored_outcome = matches!(
                        crate::abi::abi_value_from_type_expr_with_resolver(
                            return_type_expr,
                            semantic.resolved,
                            |source| semantic.resolved_sources.get(&source).copied(),
                        )
                        .map(|value| value.ty),
                        Ok(crate::abi::AbiType::Outcome { .. })
                    );
                    if returns_stored_outcome {
                        let source = context.lower_aggregate_operand(expression)?;
                        context
                            .control_flow
                            .terminate(Terminator::ReturnOutcome { source })?;
                    } else {
                        context.lower_value_to_place(
                            return_local,
                            expression,
                            return_ty,
                            return_representation,
                            root_scope,
                        )?;
                    }
                }
                super::ValueRepresentation::Borrow | super::ValueRepresentation::Error => {
                    return Err(BuildError::UnsupportedClaimedExpression);
                }
            }
            if context.control_flow.current_block().is_ok() {
                context.control_flow.terminate(Terminator::Return)?;
            }
        }
    }
    let parts = context.finish()?;
    let outcome_contract = outcome_contract(contextual_return_ty, semantic)?;
    let body = Body {
        source_body,
        source_span: block.span,
        return_local,
        return_mode,
        outcome_contract,
        root_scope,
        scopes: parts.scopes,
        locals: parts.locals,
        entry: BasicBlockId::from_index(0),
        blocks: parts.blocks,
        loop_regions: parts.loop_regions,
        allocation_regions: parts.allocation_regions,
        allocation_overrides: parts.allocation_overrides,
        loans: parts.loans,
        projections: parts.projections,
        drop_plans: parts.drop_plans,
    };
    super::finalize(body).map_err(BuildError::InvalidMir)
}

fn outcome_contract(
    result_ty: crate::semantic::TyId,
    semantic: SemanticInputs<'_>,
) -> Result<Option<super::OutcomeContract>, BuildError> {
    let result = semantic
        .typed_hir
        .type_expr_by_id(result_ty)
        .ok_or(BuildError::MissingTypedExpression)?;
    let shape = crate::outcomes::outcome_shape_with_resolver(result, semantic.resolved, |source| {
        semantic.resolver_for(source)
    });
    if shape.layers.is_empty() {
        return Ok(None);
    }
    let payload_ty = semantic
        .typed_hir
        .type_id(&shape.payload)
        .ok_or(BuildError::MissingTypedExpression)?;
    let payload_representation = value_representation(payload_ty, semantic)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    Ok(Some(super::OutcomeContract {
        layers: shape.layers,
        payload_ty,
        payload_representation,
    }))
}

fn parameter_representation(
    parameter: &Parameter,
    semantic: SemanticInputs<'_>,
) -> Option<super::ValueRepresentation> {
    type_representation(&parameter.ty, semantic)
}

fn type_representation(
    type_expr: &crate::ast::TypeExpr,
    semantic: SemanticInputs<'_>,
) -> Option<super::ValueRepresentation> {
    let ty = semantic.typed_hir.type_id(type_expr)?;
    if let Some(scalar) = scalar_type(ty, semantic.typed_hir) {
        return Some(super::ValueRepresentation::Scalar(scalar));
    }
    if let Some(representation @ super::ValueRepresentation::View(_)) =
        value_representation(ty, semantic)
    {
        return Some(representation);
    }
    if matches!(type_expr, crate::ast::TypeExpr::Borrow(_)) {
        return Some(super::ValueRepresentation::Borrow);
    }
    if matches!(type_expr, crate::ast::TypeExpr::Reference(reference) if reference.name == "void") {
        return Some(super::ValueRepresentation::Unit);
    }
    let abi = crate::abi::abi_value_from_type_expr_with_resolver(
        type_expr,
        semantic.resolved,
        |source| semantic.resolver_for(source),
    )
    .ok()?
    .ty;
    let aggregate = matches!(
        abi,
        crate::abi::AbiType::Struct(_)
            | crate::abi::AbiType::Array { .. }
            | crate::abi::AbiType::Enum(_)
            | crate::abi::AbiType::Outcome { .. }
    );
    (aggregate
        && (crate::typecheck::type_expr_is_copy(type_expr, semantic.resolved) == Some(true)
            || super::drop_plans::is_supported(
                type_expr,
                semantic.resolved,
                semantic.resolved_sources,
                semantic.typed_hir,
            )))
    .then_some(super::ValueRepresentation::Aggregate)
}

pub(crate) use closures::try_build_closure_body;

#[cfg(test)]
#[path = "lower/tests.rs"]
mod tests;
