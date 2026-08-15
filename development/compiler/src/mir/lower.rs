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
mod source_model;
pub(crate) use source_model::outcome_intrinsic_is_supported;
mod explicit_drops;
mod expressions;
mod indexes;
mod interpolation;
mod iteration;
mod literal_packs;
mod literals;
mod projections;
mod regions;
mod statements;
mod storage_types;
use context::LoweringContext;
pub(crate) use literal_packs::{LiteralPackInput, LiteralPackInputSegment};
use source_model::*;
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

    fn comparison_plan(
        self,
        operator_span: crate::source::ByteSpan,
    ) -> Option<crate::typecheck::TypecheckComparisonPlan> {
        let plan = self.typed_hir.comparison_plan(operator_span)?.clone();
        if plan.method.is_some() {
            return Some(plan);
        }

        // Generic comparison requirements become concrete only after the
        // containing callable has been specialized.  The specialized HIR has
        // already substituted its operand types; resolve the operator once at
        // the MIR boundary so source_model and construction consume the same plan.
        self.resolved_sources
            .values()
            .copied()
            .chain(std::iter::once(self.resolved))
            .find_map(|resolved| {
                crate::typecheck::specialize_comparison_plan(plan.clone(), resolved)
            })
    }

    fn conversion_plan(
        self,
        expression: &Expr,
    ) -> Option<crate::typecheck::TypecheckConversionPlan> {
        let mut expression = expression;
        let mut conversion = loop {
            if let Some(plan) = self.typed_hir.conversion_plan(expression.span()) {
                break plan.clone();
            }
            let Expr::Group(group) = expression else {
                return None;
            };
            expression = &group.expression;
        };
        let crate::typecheck::TypecheckConversionKind::BorrowCoercion(plan) = &conversion.kind
        else {
            return Some(conversion);
        };
        if plan.def_id.is_some() {
            return Some(conversion);
        }
        let specialized = self
            .resolved_sources
            .values()
            .copied()
            .chain(std::iter::once(self.resolved))
            .find_map(|resolved| {
                crate::typecheck::specialize_coercion_plan_across_resolvers(
                    plan.clone(),
                    std::iter::once(resolved),
                )
            })?;
        conversion.kind = crate::typecheck::TypecheckConversionKind::BorrowCoercion(specialized);
        Some(conversion)
    }

    fn index_plan(
        self,
        expression_span: crate::source::ByteSpan,
    ) -> Option<crate::typecheck::TypecheckIndexPlan> {
        let plan = self.typed_hir.index_plan(expression_span)?.clone();
        if plan.projection != crate::typecheck::TypecheckIndexProjection::Requirement {
            return Some(plan);
        }
        self.resolved_sources
            .values()
            .copied()
            .chain(std::iter::once(self.resolved))
            .find_map(|resolved| {
                crate::typecheck::specialize_index_plan_across_resolvers(
                    plan.clone(),
                    std::iter::once(resolved),
                )
            })
    }

    fn collection_for_plan(
        self,
        span: crate::source::ByteSpan,
    ) -> Option<crate::typecheck::TypecheckCollectionForPlan> {
        let plan = self.typed_hir.collection_for_plan(span)?.clone();
        self.resolved_sources
            .values()
            .copied()
            .chain(std::iter::once(self.resolved))
            .find_map(|resolved| {
                crate::typecheck::specialize_collection_plan(plan.clone(), resolved)
            })
    }

    fn sequence_spread_plan(
        self,
        span: crate::source::ByteSpan,
    ) -> Option<crate::typecheck::TypecheckSequenceSpreadPlan> {
        let plan = self.typed_hir.sequence_spread_plan(span)?.clone();
        self.resolved_sources
            .values()
            .copied()
            .chain(std::iter::once(self.resolved))
            .find_map(|resolved| {
                crate::typecheck::specialize_sequence_spread_plan(plan.clone(), resolved)
            })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BuildInputs<'a> {
    pub(crate) semantic_db: &'a SemanticDb,
    pub(crate) resolved: &'a ResolveOutput,
    pub(crate) resolved_sources: &'a ResolvedSources<'a>,
    pub(crate) typed_hir: &'a TypedHir,
    /// Declared callable result, including optional/fallible layers.  Expression
    /// facts carry the contextual success type, so the declaration is the
    /// authoritative source for the ABI outcome contract.
    pub(crate) declared_return_ty: Option<crate::semantic::TyId>,
}

pub(crate) fn prepare_typed_hir(
    typed_hir: &TypedHir,
    substitutions: &HashMap<String, crate::ast::TypeExpr>,
    parameters: &[Parameter],
    return_ty: &crate::ast::TypeExpr,
    literal_pack: Option<&LiteralPackInput>,
) -> TypedHir {
    let mut types = parameters
        .iter()
        .map(|parameter| parameter.ty.clone())
        .chain(std::iter::once(return_ty.clone()))
        .collect::<Vec<_>>();
    // Index projections normalize contextual integer literals to `usize` in
    // MIR even when no authored signature mentions the type. Retain that
    // builtin in every prepared type arena so projection construction never
    // depends on an unrelated source occurrence.
    types.push(crate::ast::TypeExpr::Reference(crate::ast::TypeReference {
        span: return_ty.span(),
        name: "usize".to_string(),
    }));
    if let Some(pack) = literal_pack {
        types.extend(pack.runtime_types());
    }
    let specialized = typed_hir.specialized(substitutions);
    types.extend(specialized.runtime_fact_types());
    specialized.with_additional_types(types)
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
    UnloadedImportedCall {
        span: crate::source::ByteSpan,
        path: String,
    },
    UnspecializedGenericCall {
        span: crate::source::ByteSpan,
    },
    MissingOpenBlock,
    OpenBlockNotTerminated,
    BlockAlreadyTerminated,
    UnterminatedReservedBlock,
    UnsupportedClaimedExpression,
    UnsupportedSource {
        span: crate::source::ByteSpan,
        construct: &'static str,
        help: &'static str,
    },
    Context {
        operation: &'static str,
        source: Box<BuildError>,
    },
    ClosurePreparation(&'static str),
    ClosureBody(Box<BuildError>),
    InvalidMir(Vec<ValidationError>),
}

impl BuildError {
    pub(super) fn context(self, operation: &'static str) -> Self {
        Self::Context {
            operation,
            source: Box::new(self),
        }
    }
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
            declared_return_ty: None,
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
    Some(build_body(
        block,
        parameters,
        super::CallableReturnContract {
            representation: super::ValueRepresentation::Scalar(return_scalar),
            mode: return_mode,
        },
        inputs,
    ))
}

#[cfg(test)]
fn try_build_body_with_return_mode(
    block: &Block,
    parameters: &[Parameter],
    return_representation: super::ValueRepresentation,
    return_mode: ReturnMode,
    inputs: BuildInputs<'_>,
) -> Option<Result<Body, BuildError>> {
    Some(build_body(
        block,
        parameters,
        super::CallableReturnContract {
            representation: return_representation,
            mode: return_mode,
        },
        inputs,
    ))
}

pub(crate) fn build_body(
    block: &Block,
    parameters: &[Parameter],
    return_contract: super::CallableReturnContract,
    inputs: BuildInputs<'_>,
) -> Result<Body, BuildError> {
    build_body_with_literal_pack(block, parameters, return_contract, inputs, None)
}

pub(crate) fn build_literal_body(
    block: &Block,
    parameters: &[Parameter],
    return_contract: super::CallableReturnContract,
    inputs: BuildInputs<'_>,
    literal_pack: LiteralPackInput,
) -> Result<Body, BuildError> {
    build_body_with_literal_pack(
        block,
        parameters,
        return_contract,
        inputs,
        Some(literal_pack),
    )
}

fn build_body_with_literal_pack(
    block: &Block,
    parameters: &[Parameter],
    return_contract: super::CallableReturnContract,
    inputs: BuildInputs<'_>,
    literal_pack: Option<LiteralPackInput>,
) -> Result<Body, BuildError> {
    let super::CallableReturnContract {
        representation: return_representation,
        mode: return_mode,
    } = return_contract;
    let semantic = SemanticInputs {
        resolved: inputs.resolved,
        resolved_sources: inputs.resolved_sources,
        typed_hir: inputs.typed_hir,
    };
    let Some((mut source_statements, mut tail)) = scalar_body_parts(block) else {
        return Err(BuildError::UnsupportedClaimedExpression.context("normalize source body"));
    };
    if return_representation == super::ValueRepresentation::Unit
        && let Some(if_) = tail.conditional()
    {
        source_statements.push(ScalarStatement::If(if_));
        tail = ScalarTail::ImplicitUnit(block.span);
    }
    if return_representation == super::ValueRepresentation::Unit {
        match tail {
            // Parser-level result expressions and statement-form control flow
            // have the same effect semantics in a unit body. Normalize every
            // branching family here so they share statement lowering and the
            // function receives one implicit return edge.
            ScalarTail::Expression(Expr::IfIs(if_is)) => {
                source_statements.push(ScalarStatement::IfIs(if_is));
                tail = ScalarTail::ImplicitUnit(block.span);
            }
            ScalarTail::Expression(Expr::Match(match_)) => {
                source_statements.push(ScalarStatement::Match(match_));
                tail = ScalarTail::ImplicitUnit(block.span);
            }
            _ => {}
        }
    }
    let contextual_return_ty = if matches!(tail, ScalarTail::ImplicitUnit(_))
        && return_representation != super::ValueRepresentation::Unit
    {
        source_model::terminal_return_type(block, inputs.typed_hir)
            .or(inputs.declared_return_ty)
            .ok_or(BuildError::MissingTypedExpression)?
    } else {
        tail.result_type(inputs.typed_hir)
            .ok_or(BuildError::MissingTypedExpression)?
    };
    let declared_return_ty = inputs.declared_return_ty.unwrap_or(contextual_return_ty);
    let declared_payload_ty = inputs
        .typed_hir
        .type_expr_by_id(declared_return_ty)
        .map(|ty| {
            crate::outcomes::outcome_shape_with_resolver(ty, inputs.resolved, |source| {
                inputs.resolved_sources.get(&source).copied()
            })
        })
        .filter(|shape| !shape.layers.is_empty())
        .and_then(|shape| inputs.typed_hir.type_id(&shape.payload));
    let return_ty = if let Some(payload_ty) = declared_payload_ty {
        payload_ty
    } else if return_representation == super::ValueRepresentation::Aggregate {
        contextual_return_ty
    } else {
        tail.expression()
            .and_then(|expression| source_model::handled_outcome_success_type(expression, semantic))
            .or_else(|| {
                tail.expression()
                    .filter(|expression| {
                        source_model::conversion_plan_for_expression(expression, inputs.typed_hir)
                            .is_none_or(|conversion| {
                                !matches!(
                                    conversion.kind,
                                    crate::typecheck::TypecheckConversionKind::BorrowCoercion(_)
                                )
                            })
                    })
                    .and_then(|expression| {
                        intrinsic_expression_type(expression.span(), inputs.typed_hir)
                    })
                    .filter(|ty| value_representation(*ty, semantic) == Some(return_representation))
            })
            .unwrap_or(contextual_return_ty)
    };
    let return_ty = storage_types::normalized_storage_type(return_ty, semantic);
    (|| {
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
                let ownership = if super::drop_plans::is_copy(
                    return_type_expr,
                    inputs.resolved,
                    inputs.resolved_sources,
                ) == Some(true)
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
            super::ValueRepresentation::Borrow => {
                let readwrite = inputs
                    .typed_hir
                    .type_expr_by_id(return_ty)
                    .and_then(|ty| borrow_readwrite(ty, inputs.resolved))
                    .or_else(|| tail.expression().and_then(expression_borrow_readwrite))
                    .ok_or(BuildError::UnsupportedClaimedExpression)
                    .map_err(|error| error.context("classify borrowed return representation"))?;
                Local::borrow(
                    return_ty,
                    readwrite,
                    LocalStorage::Return,
                    LocalOrigin::Return,
                    root_scope,
                )
            }
            super::ValueRepresentation::Error => Local::error(
                return_ty,
                LocalStorage::Return,
                LocalOrigin::Return,
                root_scope,
            ),
        };
        let return_is_outcome = inputs
            .typed_hir
            .type_expr_by_id(declared_return_ty)
            .is_some_and(|ty| {
                !crate::outcomes::outcome_shape_with_resolver(ty, inputs.resolved, |source| {
                    inputs.resolved_sources.get(&source).copied()
                })
                .layers
                .is_empty()
            });
        if return_local_contract.ownership == OwnershipKind::Move && !return_is_outcome {
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
                .ok_or(BuildError::UnsupportedClaimedExpression)
                .map_err(|error| error.context("build return drop plan"))?,
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
                .type_id(&parameter.ty)
                .ok_or(BuildError::MissingParameterType)?;
            let ty = storage_types::normalized_storage_type(ty, semantic);
            let local = LocalId::from_index(locals.len());
            let storage = LocalStorage::Parameter { ordinal: index };
            let origin = LocalOrigin::Parameter(symbol);
            let mut local_contract = match parameter_representation(parameter, semantic)
                .ok_or(BuildError::UnsupportedClaimedExpression)
                .map_err(|error| error.context("classify parameter representation"))?
            {
                super::ValueRepresentation::Unit => Local::unit(ty, storage, origin, root_scope),
                super::ValueRepresentation::Scalar(scalar) => {
                    Local::scalar(ty, scalar, storage, origin, root_scope)
                }
                super::ValueRepresentation::Aggregate => {
                    let ownership = if super::drop_plans::is_copy(
                        &parameter.ty,
                        inputs.resolved,
                        inputs.resolved_sources,
                    ) == Some(true)
                    {
                        OwnershipKind::Copy
                    } else {
                        OwnershipKind::Move
                    };
                    Local::aggregate(ty, ownership, storage, origin, root_scope)
                }
                super::ValueRepresentation::Borrow => {
                    let readwrite = borrow_readwrite(&parameter.ty, inputs.resolved)
                        .ok_or(BuildError::UnsupportedClaimedExpression)
                        .map_err(|error| error.context("classify borrowed parameter"))?;
                    Local::borrow(ty, readwrite, storage, origin, root_scope)
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
                    .ok_or(BuildError::UnsupportedClaimedExpression)
                    .map_err(|error| error.context("build parameter drop plan"))?,
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
            declared_return_ty,
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
            literal_pack,
        )
    })()
}

#[allow(clippy::too_many_arguments)]
fn build_prepared_body(
    block: &Block,
    source_statements: Vec<ScalarStatement<'_>>,
    tail: ScalarTail<'_>,
    _contextual_return_ty: crate::semantic::TyId,
    declared_return_ty: crate::semantic::TyId,
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
    literal_pack: Option<LiteralPackInput>,
) -> Result<Body, BuildError> {
    let return_local = LocalId::from_index(0);
    let root_scope = ScopeId::from_index(0);
    let outcome_contract = outcome_contract(declared_return_ty, semantic)
        .map_err(|error| error.context("build body outcome contract"))?;
    let mut context = LoweringContext::new(
        semantic,
        outcome_contract.clone(),
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
    if let Some(literal_pack) = literal_pack {
        literal_packs::prepare(&mut context, literal_pack, root_scope)
            .map_err(|error| error.context("prepare literal pack"))?;
    }
    let source_exits = StatementLowerer::new(&mut context)
        .lower(&source_statements, root_scope)
        .map_err(|error| error.context("lower body statements"))?;
    if !source_exits {
        if return_mode == ReturnMode::Fallible
            && tail.expression().is_some_and(|expression| {
                source_model::failure_value_is_supported(expression, semantic)
            })
        {
            context
                .lower_failure_return(
                    tail.expression()
                        .ok_or(BuildError::UnsupportedClaimedExpression)?,
                    root_scope,
                )
                .map_err(|error| error.context("lower tail failure return"))?;
        } else if context.outcome_contract.as_ref().is_some_and(|contract| {
            contract.payload_representation == super::ValueRepresentation::Aggregate
        }) && tail.expression().is_some_and(|expression| {
            !source_model::expression_has_outcome_value(expression, semantic)
        }) {
            context
                .lower_direct_outcome_return(
                    tail.expression()
                        .ok_or(BuildError::UnsupportedClaimedExpression)?,
                    root_scope,
                )
                .map_err(|error| error.context("lower tail outcome success"))?;
        } else if context.outcome_contract.is_some()
            && tail.expression().is_some_and(|expression| {
                source_model::expression_has_outcome_value(expression, semantic)
            })
        {
            let source = context
                .lower_aggregate_operand(
                    tail.expression()
                        .ok_or(BuildError::UnsupportedClaimedExpression)?,
                    root_scope,
                )
                .map_err(|error| error.context("lower stored tail outcome operand"))?;
            context
                .control_flow
                .terminate(Terminator::ReturnOutcome { source })?;
        } else if tail.expression().is_some_and(|expression| {
            source_model::outcome_return_expression_is_supported(
                expression,
                declared_return_ty,
                semantic,
            )
        }) {
            context
                .lower_direct_outcome_return(
                    tail.expression()
                        .ok_or(BuildError::UnsupportedClaimedExpression)?,
                    root_scope,
                )
                .map_err(|error| error.context("lower tail outcome return"))?;
        } else if return_representation == super::ValueRepresentation::Unit
            && tail.conditional().is_none()
        {
            if let Some(expression) = tail.expression() {
                statements::StatementLowerer::new(&mut context)
                    .lower(&[ScalarStatement::Expression(expression)], root_scope)?;
            }
            if context.control_flow.current_block().is_ok() {
                context.control_flow.terminate(Terminator::Return)?;
            }
        } else if let Some(if_) = tail.conditional()
            && source_model::outcome_return_conditional_is_supported(
                if_,
                declared_return_ty,
                semantic,
            )
        {
            let exits = StatementLowerer::new(&mut context)
                .lower(&[ScalarStatement::If(if_)], root_scope)?;
            if !exits {
                return Err(BuildError::UnsupportedClaimedExpression
                    .context("lower terminal outcome conditional"));
            }
        } else if let Some(if_) = tail.conditional() {
            expressions::lower_conditional_to_place(
                &mut context,
                return_local,
                if_,
                return_ty,
                return_representation,
                root_scope,
            )
            .map_err(|error| error.context("lower tail conditional"))?;
            context.control_flow.terminate(Terminator::Return)?;
        } else if let Some(Expr::Call(call)) = tail.expression()
            && semantic
                .typed_hir
                .expression(call.span)
                .is_some_and(|expression| expression.diverges)
        {
            let source = semantic
                .typed_hir
                .expression(call.span)
                .ok_or(BuildError::MissingTypedExpression)?
                .id;
            let (callee, arguments, returns_never) = context.lower_call(call, root_scope)?;
            if !returns_never {
                return Err(
                    BuildError::UnsupportedClaimedExpression.context("lower diverging tail call")
                );
            }
            context
                .control_flow
                .emit_never_call(source, callee, arguments)?;
        } else {
            let expression = tail
                .expression()
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            if context.outcome_contract.is_some()
                && source_model::expression_has_outcome_value(expression, semantic)
            {
                let source = context
                    .lower_aggregate_operand(expression, root_scope)
                    .map_err(|error| error.context("lower terminal stored outcome operand"))?;
                context
                    .control_flow
                    .terminate(Terminator::ReturnOutcome { source })?;
            } else {
                match return_representation {
                    super::ValueRepresentation::Unit => {
                        unreachable!("unit returns terminate above")
                    }
                    super::ValueRepresentation::Scalar(_)
                    | super::ValueRepresentation::View(_)
                    | super::ValueRepresentation::Borrow
                    | super::ValueRepresentation::Error => context
                        .lower_value_to_place(
                            return_local,
                            expression,
                            return_ty,
                            return_representation,
                            root_scope,
                        )
                        .map_err(|error| error.context("lower tail value"))?,
                    super::ValueRepresentation::Aggregate => {
                        context
                            .lower_value_to_place(
                                return_local,
                                expression,
                                return_ty,
                                return_representation,
                                root_scope,
                            )
                            .map_err(|error| error.context("lower aggregate tail value"))?;
                    }
                }
            }
            if context.control_flow.current_block().is_ok() {
                context.control_flow.terminate(Terminator::Return)?;
            }
        }
    }
    let parts = context
        .finish()
        .map_err(|error| error.context("finish MIR body"))?;
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
    let body = super::finalize(body).map_err(BuildError::InvalidMir)?;
    if return_representation == super::ValueRepresentation::Error
        && super::static_error_payload(&body).is_none()
    {
        return Err(BuildError::UnsupportedClaimedExpression
            .context("require a static checked-MIR error helper body"));
    }
    Ok(body)
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
        payload_borrow_readwrite: borrow_readwrite(&shape.payload, semantic.resolved),
    }))
}

fn parameter_representation(
    parameter: &Parameter,
    semantic: SemanticInputs<'_>,
) -> Option<super::ValueRepresentation> {
    type_representation(&parameter.ty, semantic)
}

fn borrow_readwrite(
    ty: &crate::ast::TypeExpr,
    resolved: &crate::resolve::ResolveOutput,
) -> Option<bool> {
    match ty {
        crate::ast::TypeExpr::Borrow(borrow) => Some(borrow.is_readwrite),
        _ => crate::typecheck::type_expr_borrow_readwrite(ty, resolved),
    }
}

fn expression_borrow_readwrite(expression: &crate::ast::Expr) -> Option<bool> {
    match expression.without_groups() {
        crate::ast::Expr::Borrow(borrow) => Some(borrow.is_readwrite),
        crate::ast::Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
            expression_borrow_readwrite(&unary.operand)
        }
        _ => None,
    }
}

fn type_representation(
    type_expr: &crate::ast::TypeExpr,
    semantic: SemanticInputs<'_>,
) -> Option<super::ValueRepresentation> {
    if let Some(representation) = semantic
        .typed_hir
        .type_id(type_expr)
        .and_then(|ty| value_representation(ty, semantic))
        && representation != super::ValueRepresentation::Aggregate
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
    if let Some(kind) = abi.integer_type() {
        return Some(super::ValueRepresentation::Scalar(match kind {
            crate::integer::IntegerType::I32 => ScalarType::I32,
            crate::integer::IntegerType::U8 => ScalarType::U8,
            crate::integer::IntegerType::Usize => ScalarType::Usize,
            kind => ScalarType::Integer(kind),
        }));
    }
    match abi {
        crate::abi::AbiType::Bool => Some(super::ValueRepresentation::Scalar(ScalarType::Bool)),
        crate::abi::AbiType::Pointer => Some(super::ValueRepresentation::Scalar(ScalarType::Usize)),
        crate::abi::AbiType::StrView => {
            Some(super::ValueRepresentation::View(super::ViewKind::Str))
        }
        crate::abi::AbiType::SliceView => {
            Some(super::ValueRepresentation::View(super::ViewKind::Slice))
        }
        crate::abi::AbiType::Borrow => Some(super::ValueRepresentation::Borrow),
        crate::abi::AbiType::Struct(_)
        | crate::abi::AbiType::Array { .. }
        | crate::abi::AbiType::Enum(_)
        | crate::abi::AbiType::Outcome { .. }
            if super::drop_plans::is_copy(
                type_expr,
                semantic.resolved,
                semantic.resolved_sources,
            ) == Some(true)
                || super::drop_plans::is_supported(
                    type_expr,
                    semantic.resolved,
                    semantic.resolved_sources,
                    semantic.typed_hir,
                ) =>
        {
            Some(super::ValueRepresentation::Aggregate)
        }
        _ => None,
    }
}

pub(crate) use closures::build_closure_body;

#[cfg(test)]
#[path = "lower/tests.rs"]
mod tests;
