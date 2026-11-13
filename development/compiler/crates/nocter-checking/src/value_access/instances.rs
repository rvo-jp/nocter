//! Metadata-only instantiation of a frozen typed call; never lookup, body typing or const proof.
use super::{Argument, CallKind, flow};
use nocter_declarations::value_access::{
    CodeInstanceKey, CodeTemplateKey, ConcreteWitness, FunctionWitness, Program,
};
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
    let bindings = call_bindings(arguments, |witness| {
        caller
            .resolve(program, witness)
            .map(ConcreteWitness::witness)
            .map_err(|_| invalid)
    })?;
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
        .filter_map(
            |(id, expression)| match select_call(program, caller, &expression.kind) {
                Ok(None) => None,
                Ok(Some(key)) => Some(Ok((id, key))),
                Err(error) => Some(Err(error)),
            },
        )
        .collect()
}

pub(super) fn select_call(
    program: &Program,
    caller: &CodeInstanceKey,
    expression: &super::ExpressionKind,
) -> Result<Option<CodeInstanceKey>, flow::IntegrityError> {
    let super::ExpressionKind::Call {
        target,
        kind,
        arguments,
    } = expression
    else {
        return Ok(None);
    };
    if !completed(arguments) {
        return Ok(None);
    }
    instantiate_call(program, caller, *target, *kind, arguments).map(Some)
}

/// A selected condition edge; a parameter is unresolved and grants no executable/capability fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallTemplate {
    Code(CodeTemplateKey),
    Parameter(FunctionWitness),
}

pub(super) fn instantiate_template_call(
    program: &Program,
    caller: &CodeTemplateKey,
    target: ValueFunctionOrigin,
    kind: CallKind,
    arguments: &[Argument],
) -> Result<CallTemplate, flow::IntegrityError> {
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
    let bindings = call_bindings(arguments, |witness| {
        caller.resolve(program, witness).map_err(|_| invalid)
    })?;
    if matches!(selected.origin, ValueFunctionOrigin::Parameter(_)) {
        // The source operation reference retains its complete argument metadata. No actual
        // callee frame can be named until this selected parameter has a code witness.
        return Ok(CallTemplate::Parameter(selected));
    }
    program
        .bind_template(selected.origin.code().ok_or(invalid)?, bindings)
        .map(CallTemplate::Code)
        .map_err(|_| invalid)
}

fn call_bindings(
    arguments: &[Argument],
    resolve: impl Fn(FunctionWitness) -> Result<FunctionWitness, flow::IntegrityError>,
) -> Result<Vec<(nocter_model::ValueParameter, FunctionWitness)>, flow::IntegrityError> {
    arguments
        .iter()
        .filter_map(|argument| argument.binding.as_ref())
        .map(|binding| Ok((binding.parameter, resolve(binding.witness)?)))
        .collect()
}
