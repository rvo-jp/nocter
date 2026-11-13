//! Distinguish a newly reserved factory application from transport of a selected result descriptor.
use crate::value_access::{ExpressionKind, Query, flow};
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema};
use nocter_model::{AccessExpressionId, TypeId, ValueApplication, ValueFunctionOrigin};
#[cfg(test)]
mod tests;

pub(super) struct CallResult {
    pub(super) data: TypeId,
    pub(super) created: Option<ValueApplication>,
}
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;

pub(super) fn call_result(
    query: &Query,
    id: AccessExpressionId,
) -> Result<Option<CallResult>, flow::IntegrityError> {
    let expression = query.body().expressions().get(id).ok_or(INVALID)?;
    let ExpressionKind::Call {
        target,
        kind,
        arguments,
    } = &expression.kind
    else {
        return Err(INVALID);
    };
    let Some(access) = expression.ty.access() else {
        return Ok(None);
    };
    let data = access.data();
    let invocation = kind.invocation(arguments).ok_or(INVALID)?;
    if let Some(PayloadSchema::Function(FunctionWitness {
        origin: ValueFunctionOrigin::Application(application),
        invocation: result_invocation,
    })) = query.program().data_schema(data)
        && application.owner == query.code()
        && application.call == id
    {
        if *target != ValueFunctionOrigin::Declaration(application.callee)
            || query
                .program()
                .codes()
                .get(application.callee)
                .and_then(nocter_declarations::value_access::Code::result_contract)
                .is_none_or(|contract| contract.invocation != result_invocation)
        {
            return Err(INVALID);
        }
        return Ok(Some(CallResult {
            data,
            created: Some(application),
        }));
    }
    let contract = query
        .program()
        .function_contract(FunctionWitness {
            origin: *target,
            invocation,
        })
        .ok_or(INVALID)?;
    if contract.result != data {
        return Err(INVALID);
    }
    Ok(Some(CallResult {
        data,
        created: None,
    }))
}
