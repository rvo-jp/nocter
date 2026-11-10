//! Freeze one ordered input layout, with exact coverage and no implicit environment offset.
use super::{LocalOrigin, Query, flow::IntegrityError};
use nocter_model::ValueCode;
use std::collections::BTreeSet;
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;

pub(super) fn layout(query: &Query) -> Result<(), IntegrityError> {
    if query.source_for_code().is_none() {
        return Err(INVALID);
    }
    let body = query.body();
    let inputs = body.inputs();
    let mut seen = BTreeSet::new();
    for input in inputs.ordered() {
        let local = body.locals().get(input.local).ok_or(INVALID)?;
        if !seen.insert(input.local) || local.kind != input.operation.binding_kind() {
            return Err(INVALID);
        }
    }
    let mut statics = BTreeSet::new();
    for (_, local) in body.locals().iter() {
        if let LocalOrigin::Static(id) = local.origin
            && (!statics.insert(id)
                || local.kind != nocter_model::ValueBindingKind::ReadAlias
                || query
                    .program()
                    .static_declaration(id)
                    .is_none_or(|declaration| declaration.result() != local.data))
        {
            return Err(INVALID);
        }
    }
    let expected = body
        .locals()
        .iter()
        .filter_map(|(id, l)| l.origin.is_input().then_some(id))
        .collect();
    if seen != expected {
        return Err(INVALID);
    }
    match query.code() {
        ValueCode::Declaration(id) => {
            if inputs.environment().is_some() {
                return Err(INVALID);
            }
            let parameters = query
                .program()
                .callables()
                .get(id)
                .ok_or(INVALID)?
                .parameters();
            if parameters.len() != inputs.explicit().len() {
                return Err(INVALID);
            }
            for (&parameter, input) in parameters.iter().zip(inputs.explicit()) {
                let local = body.locals().get(input.local).ok_or(INVALID)?;
                let declared = query.program().parameters().get(parameter).ok_or(INVALID)?;
                if local.origin != LocalOrigin::Parameter(parameter)
                    || local.data != declared.data()
                    || input.operation != declared.operation()
                {
                    return Err(INVALID);
                }
            }
        }
        ValueCode::Closure(_) => {
            let environment = inputs.environment().ok_or(INVALID)?;
            if body.locals().get(environment.local).ok_or(INVALID)?.origin != LocalOrigin::Capture
                || inputs.explicit().iter().any(|i| {
                    body.locals()
                        .get(i.local)
                        .is_none_or(|l| l.origin != LocalOrigin::ClosureParameter)
                })
            {
                return Err(INVALID);
            }
        }
    }
    Ok(())
}
pub(super) fn contract(query: &Query) -> Result<(), IntegrityError> {
    layout(query)?;
    if let ValueCode::Closure(id) = query.code() {
        let schema = query.program().closure(id).ok_or(INVALID)?;
        let inputs = query.body().inputs();
        let environment = inputs.environment().ok_or(INVALID)?;
        if environment.operation != schema.contract().invocation.operation()
            || query
                .body()
                .locals()
                .get(environment.local)
                .ok_or(INVALID)?
                .data
                != schema.environment()
            || query.result_data() != schema.contract().result
            || inputs.explicit().len() != schema.contract().inputs.len()
        {
            return Err(INVALID);
        }
        for (input, declared) in inputs.explicit().iter().zip(&schema.contract().inputs) {
            if input.operation != declared.operation
                || query.body().locals().get(input.local).ok_or(INVALID)?.data != declared.data
            {
                return Err(INVALID);
            }
        }
    }
    Ok(())
}
