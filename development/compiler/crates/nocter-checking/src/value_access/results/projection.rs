//! Discharge result identities without changing source-selected permissions, moves or control IDs.
use super::Evidence;
use crate::value_access::{
    Failure, Query,
    ir::{ArgumentAction, BoundaryAction, ExpressionKind, ValueType},
};
use nocter_declarations::value_access::Program;
use nocter_model::{ValueAccess, ValueFunctionOrigin};
use std::sync::Arc;

fn value(ty: ValueType, program: &Program, evidence: &Evidence) -> Result<ValueType, Failure> {
    let Some(access) = ty.access() else {
        return Ok(ty);
    };
    Ok(ValueType::known(ValueAccess::new(
        evidence.data(program, access.data())?,
        access.mode(),
    ))
    .with_continuation(ty.may_complete()))
}
pub(super) fn project(query: &mut Query, evidence: &Arc<Evidence>) -> Result<(), Failure> {
    let program = query.program.clone();
    query.body.locals = query.body.locals.try_map(|_, local| {
        let mut local = local.clone();
        local.data = evidence.data(&program, local.data)?;
        Ok::<_, Failure>(local)
    })?;
    query.body.places = query.body.places.try_map(|_, place| {
        let mut place = place.clone();
        place.data = evidence.data(&program, place.data)?;
        Ok::<_, Failure>(place)
    })?;
    query.body.expressions = query.body.expressions.try_map(|_, expression| {
        let mut expression = expression.clone();
        expression.ty = value(expression.ty, &program, evidence)?;
        if let ExpressionKind::Boundary {
            source,
            action: BoundaryAction::CopyPayload,
        } = expression.kind
        {
            copy_proof(query, source, evidence)?;
        }

        if let ExpressionKind::Call {
            target, arguments, ..
        } = &mut expression.kind
        {
            if let ValueFunctionOrigin::Result(id) = *target {
                *target = evidence
                    .producers
                    .get(&id)
                    .ok_or(Failure::InconsistentSyntax)?
                    .origin;
            }
            for argument in arguments {
                if argument.action == ArgumentAction::CopyPayload {
                    copy_proof(query, argument.value, evidence)?;
                }
                if let Some(binding) = &mut argument.binding {
                    binding.witness = evidence.witness(binding.witness)?;
                }
            }
        }
        Ok::<_, Failure>(expression)
    })?;
    query.body.blocks = query.body.blocks.try_map(|_, block| {
        let mut block = block.clone();
        block.ty = value(block.ty, &program, evidence)?;
        Ok::<_, Failure>(block)
    })?;
    query.result_data = evidence.data(&program, query.result_data)?;
    query.equalities.clear();
    query.results_closed = true;
    query.result_evidence = Some(evidence.clone());
    Ok(())
}

/// Conditional Copy supply of a result is not admitted until its exact producer schema proves it.
fn copy_proof(
    query: &Query,
    source: nocter_model::AccessExpressionId,
    evidence: &Evidence,
) -> Result<(), Failure> {
    let data = query
        .body
        .expressions
        .get(source)
        .and_then(|e| e.ty.access())
        .ok_or(Failure::InconsistentSyntax)?
        .data();
    if super::is_result(query.program(), data)
        && query
            .program()
            .data_copyable(evidence.data(query.program(), data)?)
            != Some(true)
    {
        return Err(Failure::InconsistentSyntax);
    }
    Ok(())
}
