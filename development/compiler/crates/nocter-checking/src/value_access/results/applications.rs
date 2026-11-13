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
        let template = query
            .program()
            .application_template(application)
            .ok_or(INVALID)?;
        let result = query
            .program()
            .function_contract(FunctionWitness {
                origin: *target,
                invocation,
            })
            .ok_or(INVALID)?
            .result;
        if !matches!(query.program().data_schema(result), Some(PayloadSchema::Function(actual)) if actual == template)
            || result_invocation != template.invocation
        {
            return Err(INVALID);
        }
        return Ok(Some(CallResult {
            data,
            created: Some(application),
        }));
    }
    let result = query
        .program()
        .function_result(
            FunctionWitness {
                origin: *target,
                invocation,
            },
            |parameter| {
                arguments
                    .iter()
                    .filter_map(|argument| argument.binding.as_ref())
                    .find(|binding| binding.parameter == parameter)
                    .map(|binding| binding.witness)
            },
        )
        .ok_or(INVALID)?;
    if result != data {
        return Err(INVALID);
    }
    Ok(Some(CallResult {
        data,
        created: None,
    }))
}

pub(super) fn callee(
    query: &Query,
    id: AccessExpressionId,
) -> Result<FunctionWitness, flow::IntegrityError> {
    let ExpressionKind::Call {
        target,
        kind,
        arguments,
    } = &query.body().expressions().get(id).ok_or(INVALID)?.kind
    else {
        return Err(INVALID);
    };
    Ok(FunctionWitness {
        origin: *target,
        invocation: kind.invocation(arguments).ok_or(INVALID)?,
    })
}
