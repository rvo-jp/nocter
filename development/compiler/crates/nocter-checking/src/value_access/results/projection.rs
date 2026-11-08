//! Project result identities and freeze pending value supply before initialization.
use super::Evidence;
use crate::value_access::{
    Failure, Query,
    ir::{ExpressionKind, ValueType},
};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::Program;
use nocter_model::{ValueAccess, ValueFunctionOrigin};

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
pub(super) fn project(
    context: &DeclarationContext<'_>,
    query: &mut Query,
    evidence: &Evidence,
) -> Result<(), Failure> {
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
    query.body.expressions = query.body.expressions.try_map(|id, expression| {
        let mut expression = expression.clone();
        expression.ty = value(expression.ty, &program, evidence)?;
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
                if let Some(binding) = &mut argument.binding {
                    binding.witness = evidence.witness(binding.witness)?;
                }
            }
        }
        crate::value_access::supply::resolve(context, query, id, &mut expression)?;
        Ok::<_, Failure>(expression)
    })?;
    query.body.blocks = query.body.blocks.try_map(|_, block| {
        let mut block = block.clone();
        block.ty = value(block.ty, &program, evidence)?;
        Ok::<_, Failure>(block)
    })?;
    query.result_data = evidence.data(&program, query.result_data)?;
    query.equalities.clear();
    Ok(())
}
