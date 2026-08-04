use super::context::LoweringContext;
use super::functions::lower_scope_end_drop_instructions;
use crate::ast::Expr;
use crate::diagnostics::Diagnostic;
use crate::ir::{FallibleFailureMode, Instruction};
use crate::outcomes::{OutcomeLayer, outcome_shape_with_resolver};

/// Builds the failure path for postfix `?` from the semantic outcome layer.
///
/// Single-layer optional and fallible calls share the compact backend call ABI, but their cleanup
/// exits differ: fallible propagation must preserve the error payload, while optional propagation
/// returns `none`. Keeping that decision here prevents individual expression lowerers from
/// guessing from the payload representation.
pub(in crate::ir::lower) fn propagating_outcome_mode(
    operand: &Expr,
    context: &LoweringContext,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    let ty = context
        .expression_type_expr(operand.span())
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "E8008",
                "propagated expression is missing its resolved outcome type",
            )]
        })?;
    let (_, resolved) = context.resolved_calls().ok_or_else(|| {
        vec![Diagnostic::error(
            "E8008",
            "propagated expression is missing resolution context",
        )]
    })?;
    let shape =
        outcome_shape_with_resolver(&ty, resolved, |source| context.resolved_source(source));
    let Some(layer) = shape.layers.first().copied() else {
        return Err(vec![Diagnostic::error(
            "E8008",
            "propagated expression does not have an outcome layer",
        )]);
    };

    propagating_outcome_mode_for_layer(layer, context)
}

pub(in crate::ir::lower) fn propagating_outcome_mode_for_layer(
    layer: OutcomeLayer,
    context: &LoweringContext,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    match layer {
        OutcomeLayer::Optional => {
            let cleanup = lower_scope_end_drop_instructions(context)?;
            if cleanup.is_empty() {
                return Ok(FallibleFailureMode::Propagate);
            }
            let instructions = optional_propagation_instructions(cleanup);
            Ok(FallibleFailureMode::Handle { instructions })
        }
        OutcomeLayer::Fallible => {
            let cleanup_context = context.with_reserved_error_local_abi_words();
            let instructions = lower_scope_end_drop_instructions(&cleanup_context)?;
            if instructions.is_empty() {
                return Ok(FallibleFailureMode::Propagate);
            }
            let (code, message) = context.next_error_local_locations()?;
            Ok(FallibleFailureMode::PropagateWithCleanup {
                code,
                message,
                instructions,
            })
        }
    }
}

pub(in crate::ir::lower) fn stored_optional_propagation_instructions(
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    lower_scope_end_drop_instructions(context).map(optional_propagation_instructions)
}

fn optional_propagation_instructions(mut cleanup: Vec<Instruction>) -> Vec<Instruction> {
    cleanup.push(Instruction::ReturnOptionalNone);
    cleanup
}
