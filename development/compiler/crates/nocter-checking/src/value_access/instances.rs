//! Metadata-only instantiation of a frozen typed call; never lookup, body typing or const proof.
use super::{Argument, CallKind, flow};
use nocter_declarations::value_access::{CodeInstanceKey, FunctionWitness, Program};
use nocter_model::ValueFunctionOrigin;

/// Bind the selected code and exact callable arguments through the caller's concrete witnesses.
/// # Errors
/// Rejects symbolic/missing witnesses, incompatible selected views or incomplete formal coverage.
pub fn instantiate_call(
    program: &Program,
    caller: &CodeInstanceKey,
    target: ValueFunctionOrigin,
    kind: CallKind,
    arguments: &[Argument],
) -> Result<CodeInstanceKey, flow::IntegrityError> {
    let invalid = flow::IntegrityError::InconsistentTypedBody;
    let selected = caller
        .resolve(
            program,
            FunctionWitness {
                origin: target,
                invocation: kind.invocation(arguments).ok_or(invalid)?,
            },
        )
        .map_err(|_| invalid)?;
    let bindings = arguments
        .iter()
        .filter_map(|argument| argument.binding.as_ref())
        .map(|binding| {
            Ok((
                binding.parameter,
                caller
                    .resolve(program, binding.witness)
                    .map_err(|_| invalid)?
                    .witness(),
            ))
        })
        .collect::<Result<Vec<_>, flow::IntegrityError>>()?;
    program
        .bind_code(selected.code(), bindings)
        .map_err(|_| invalid)
}

/// A completed call is distinct from values prepared before an interrupted later argument.
pub(super) fn completed(arguments: &[Argument]) -> bool {
    !arguments.iter().any(|argument| {
        matches!(
            argument.action,
            super::ArgumentAction::Interrupted | super::ArgumentAction::Unproduced
        )
    })
}

/// Selected instance edges in an existing typed body, shared by discovery and required admission.
pub(super) fn calls(
    program: &Program,
    caller: &CodeInstanceKey,
    body: &super::Query,
) -> Result<Vec<(nocter_model::AccessExpressionId, CodeInstanceKey)>, flow::IntegrityError> {
    if caller.code() != body.code() || !std::ptr::eq(program, body.program()) {
        return Err(flow::IntegrityError::InconsistentTypedBody);
    }
    body.body()
        .expressions()
        .iter()
        .filter_map(|(id, expression)| {
            let super::ExpressionKind::Call {
                target,
                kind,
                arguments,
            } = &expression.kind
            else {
                return None;
            };
            completed(arguments).then(|| {
                instantiate_call(program, caller, *target, *kind, arguments).map(|key| (id, key))
            })
        })
        .collect()
}
