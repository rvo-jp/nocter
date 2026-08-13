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

mod body_builder;
mod coverage;
mod expressions;
mod projections;
mod statements;
use body_builder::ControlFlowBuilder;
use coverage::*;
use expressions::{lower_call, lower_conditional_to_place, lower_expression_to_place};
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
    InvalidScalarConstant,
    MissingLocalSymbol,
    MissingParameterType,
    MissingCallTarget,
    MissingOpenBlock,
    OpenBlockNotTerminated,
    BlockAlreadyTerminated,
    UnterminatedReservedBlock,
    UnsupportedClaimedExpression,
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

pub(crate) fn try_build_scalar_body_with_return_mode(
    block: &Block,
    parameters: &[Parameter],
    return_scalar: ScalarType,
    return_mode: ReturnMode,
    inputs: BuildInputs<'_>,
) -> Option<Result<Body, BuildError>> {
    let semantic = SemanticInputs {
        resolved: inputs.resolved,
        resolved_sources: inputs.resolved_sources,
        typed_hir: inputs.typed_hir,
    };
    let (source_statements, tail) = scalar_body_parts(block)?;
    if !source_statements
        .iter()
        .all(|statement| statement.is_supported(semantic))
        || !tail.is_supported(semantic)
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

    let return_ty = tail.result_type(inputs.typed_hir)?;
    if scalar_type(return_ty, inputs.typed_hir) != Some(return_scalar) {
        return None;
    }
    Some((|| {
        let source_body = inputs
            .semantic_db
            .body_at(block.span)
            .ok_or(BuildError::MissingSourceBody)?;
        let return_local = LocalId::from_index(0);
        let root_scope = ScopeId::from_index(0);
        let mut scopes = vec![Scope::root(block.span)];
        let mut locals = vec![Local::scalar(
            return_ty,
            return_scalar,
            LocalStorage::Return,
            LocalOrigin::Return,
            root_scope,
        )];
        let mut locals_by_symbol = HashMap::new();
        for (index, parameter) in parameters.iter().enumerate() {
            let ty = inputs
                .typed_hir
                .type_id(&parameter.ty)
                .ok_or(BuildError::MissingParameterType)?;
            let symbol = inputs
                .resolved
                .local_symbol_id_at_name_span(parameter.name_span)
                .ok_or(BuildError::MissingLocalSymbol)?;
            let local = LocalId::from_index(locals.len());
            let storage = LocalStorage::Parameter { ordinal: index };
            let origin = LocalOrigin::Parameter(symbol);
            locals.push(
                match parameter_representation(parameter, semantic)
                    .ok_or(BuildError::UnsupportedClaimedExpression)?
                {
                    super::ValueRepresentation::Scalar(scalar) => {
                        Local::scalar(ty, scalar, storage, origin, root_scope)
                    }
                    super::ValueRepresentation::Aggregate => Local::aggregate(
                        ty,
                        if crate::typecheck::type_expr_is_copy(&parameter.ty, inputs.resolved)
                            .unwrap_or(false)
                        {
                            OwnershipKind::Copy
                        } else {
                            OwnershipKind::Move
                        },
                        storage,
                        origin,
                        root_scope,
                    ),
                    super::ValueRepresentation::Borrow => {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    }
                },
            );
            locals_by_symbol.insert(symbol, local);
        }
        let mut projections = Vec::new();
        let mut control_flow = ControlFlowBuilder::new(root_scope);
        let mut loop_regions = Vec::new();
        StatementLowerer::new(
            semantic,
            &mut locals,
            &mut locals_by_symbol,
            &mut projections,
            &mut control_flow,
            &mut loop_regions,
            &mut scopes,
        )
        .lower(&source_statements, root_scope)?;
        if let Some(if_) = tail.conditional() {
            lower_conditional_to_place(
                return_local,
                if_,
                return_ty,
                return_scalar,
                semantic,
                &locals_by_symbol,
                &mut locals,
                &mut projections,
                &mut control_flow,
                &mut scopes,
                root_scope,
            )?;
            control_flow.terminate(Terminator::Return)?;
        } else if let Some(Expr::Call(call)) = tail.expression() {
            let source = inputs
                .typed_hir
                .expression(call.span)
                .ok_or(BuildError::MissingTypedExpression)?
                .id;
            let (callee, arguments, returns_never) = lower_call(
                call,
                semantic,
                &locals_by_symbol,
                &mut locals,
                &mut projections,
                &mut control_flow,
                &mut scopes,
                root_scope,
            )?;
            if returns_never {
                control_flow.emit_never_call(source, callee, arguments)?;
            } else {
                control_flow.emit_returning_call(source, callee, arguments, return_local)?;
                control_flow.terminate(Terminator::Return)?;
            }
        } else {
            let expression = tail
                .expression()
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            lower_expression_to_place(
                return_local,
                expression,
                return_ty,
                return_scalar,
                semantic,
                &locals_by_symbol,
                &mut locals,
                &mut projections,
                &mut control_flow,
                &mut scopes,
                root_scope,
            )?;
            control_flow.terminate(Terminator::Return)?;
        }
        let blocks = control_flow.finish()?;
        let body = Body {
            source_body,
            source_span: block.span,
            return_local,
            return_mode,
            root_scope,
            scopes,
            locals,
            entry: BasicBlockId::from_index(0),
            blocks,
            loop_regions,
            loans: Vec::new(),
            projections,
        };
        super::finalize(body).map_err(BuildError::InvalidMir)
    })())
}

fn parameter_representation(
    parameter: &Parameter,
    semantic: SemanticInputs<'_>,
) -> Option<super::ValueRepresentation> {
    let ty = semantic.typed_hir.type_id(&parameter.ty)?;
    if let Some(scalar) = scalar_type(ty, semantic.typed_hir) {
        return Some(super::ValueRepresentation::Scalar(scalar));
    }
    let aggregate = matches!(
        crate::abi::abi_value_from_type_expr_with_resolver(
            &parameter.ty,
            semantic.resolved,
            |source| semantic.resolver_for(source),
        )
        .ok()?
        .ty,
        crate::abi::AbiType::Struct(_)
            | crate::abi::AbiType::Array { .. }
            | crate::abi::AbiType::Enum(_)
    );
    (aggregate
        && crate::typecheck::type_expr_is_copy(&parameter.ty, semantic.resolved) == Some(true))
    .then_some(super::ValueRepresentation::Aggregate)
}

#[cfg(test)]
#[path = "lower/tests.rs"]
mod tests;
