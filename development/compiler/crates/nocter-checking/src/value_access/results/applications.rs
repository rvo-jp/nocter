//! Distinguish a newly reserved factory application from transport of a selected result descriptor.
use crate::value_access::{ExpressionKind, Query, flow};
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema, Program};
use nocter_model::{
    AccessExpressionId, TypeId, ValueApplication, ValueAppliedResult, ValueFunctionOrigin,
};
use std::collections::BTreeSet;
#[cfg(test)]
mod tests;

pub(super) struct CallResult {
    pub(super) data: TypeId,
    pub(super) created: BTreeSet<ValueAppliedResult>,
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
    let witness = FunctionWitness {
        origin: *target,
        invocation,
    };
    let template =
        crate::value_access::instances::call_contract(query.program(), witness, arguments)
            .ok_or(INVALID)?
            .result;
    if arguments.generic_application().is_some() {
        return (template == data)
            .then_some(Some(CallResult {
                data,
                created: BTreeSet::new(),
            }))
            .ok_or(INVALID);
    }
    let site = ValueApplication {
        owner: query.code(),
        call: id,
    };
    let created = created_results(query.program(), data, template, site)?;
    if !created.is_empty() {
        return Ok(Some(CallResult { data, created }));
    }
    let result = query
        .program()
        .function_result(witness, |parameter| {
            arguments
                .iter()
                .filter_map(|argument| argument.binding.as_ref())
                .find(|binding| binding.parameter == parameter)
                .map(|binding| binding.witness)
        })
        .ok_or(INVALID)?;
    if result != data {
        return Err(INVALID);
    }
    Ok(Some(CallResult {
        data,
        created: BTreeSet::new(),
    }))
}

fn created_results(
    program: &Program,
    data: TypeId,
    template: TypeId,
    site: ValueApplication,
) -> Result<BTreeSet<ValueAppliedResult>, flow::IntegrityError> {
    let mut pending = vec![(data, template)];
    let mut seen = BTreeSet::new();
    let mut created = BTreeSet::new();
    while let Some((actual, expected)) = pending.pop() {
        if actual == expected || !seen.insert((actual, expected)) {
            continue;
        }
        match (program.data_schema(actual), program.data_schema(expected)) {
            (Some(PayloadSchema::Function(actual)), Some(PayloadSchema::Function(expected))) => {
                let ValueFunctionOrigin::Application(result) = actual.origin else {
                    // Anonymous direct-input results can already have been substituted.
                    return Ok(BTreeSet::new());
                };
                if result.application != site {
                    return Ok(BTreeSet::new());
                }
                if program.application_template(result) != Some(expected)
                    || actual.invocation != expected.invocation
                {
                    return Err(INVALID);
                }
                created.insert(result);
            }
            (Some(PayloadSchema::Aggregate(actual)), Some(PayloadSchema::Aggregate(expected))) => {
                use nocter_declarations::value_access::AggregateForm;
                let (AggregateForm::Tuple(actual), AggregateForm::Tuple(expected)) =
                    (actual.form(), expected.form())
                else {
                    return Err(INVALID);
                };
                if actual.len() != expected.len() {
                    return Err(INVALID);
                }
                pending.extend(actual.iter().copied().zip(expected.iter().copied()));
            }
            (Some(PayloadSchema::Array(actual)), Some(PayloadSchema::Array(expected)))
                if actual.length() == expected.length() =>
            {
                pending.push((actual.element(), expected.element()));
            }
            (Some(PayloadSchema::Optional(actual)), Some(PayloadSchema::Optional(expected)))
            | (Some(PayloadSchema::Fallible(actual)), Some(PayloadSchema::Fallible(expected))) => {
                pending.push((actual, expected));
            }
            _ => return Err(INVALID),
        }
    }
    Ok(created)
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
