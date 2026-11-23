//! Occurrence-aware condition selection and separate frozen definition-contract metadata.
use super::{Argument, CallArguments, CallKind, flow};
use nocter_declarations::value_access::{
    CodeInstanceKey, CodeTemplateKey, FunctionConstraint, FunctionWitness, Program,
};
use nocter_model::ValueFunctionOrigin;

pub(super) fn call_contract(
    program: &Program,
    witness: FunctionWitness,
    arguments: &CallArguments,
) -> Option<FunctionConstraint> {
    let Some(application) = arguments.generic_application() else {
        return program.function_contract(witness);
    };
    let ValueFunctionOrigin::Declaration(code) = witness.origin else {
        return None;
    };
    program.applied_callable_contract(code, application)
}

/// Condition checking also inspects dormant source contracts. A reached commit must use the
/// retained source recipe; only an absent commit may use a definition-contract hypothesis.
/// Neither branch grants an executable call, value, body or const-capability receipt.
pub(super) fn condition_call(
    query: &super::Query,
    caller: &CodeInstanceKey,
    id: nocter_model::AccessExpressionId,
    control: Option<&super::InstanceControl>,
    work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<Option<CodeInstanceKey>, super::InstantiationFailure> {
    use super::InstantiationFailure as Failure;
    if caller.code() != query.code() {
        return Err(Failure::WrongOwner);
    }
    let expression = query
        .body()
        .expressions()
        .get(id)
        .ok_or(Failure::MissingCall)?;
    if !is_completed_call(&expression.kind) {
        return Ok(None);
    }
    work.step().map_err(|_| Failure::WorkLimit)?;
    let evidence = query.result_evidence.as_ref().ok_or(Failure::Unresolved)?;
    let site = nocter_model::ValueApplication {
        owner: caller.code(),
        call: id,
    };
    if evidence.invocation(site).is_some() {
        if let Some(control) = control {
            return super::results::instantiate_controlled_invocation(
                query.program(),
                evidence,
                caller,
                site,
                control,
                work,
            )
            .map(Some);
        }
        return super::results::instantiate_invocation(
            query.program(),
            evidence,
            caller,
            site,
            work,
        )
        .map(Some);
    }
    select_contract_call(query.program(), caller, &expression.kind).map_err(|_| Failure::Unresolved)
}

/// Capability/discovery consumers must inspect every complete original candidate frame.
/// Selector and executable admission remain with their consumers.
pub(super) fn condition_calls(
    query: &super::Query,
    caller: &CodeInstanceKey,
    id: nocter_model::AccessExpressionId,
    control: Option<&super::InstanceControl>,
    work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<Option<Box<[CodeInstanceKey]>>, super::InstantiationFailure> {
    match condition_call(query, caller, id, control, work) {
        Ok(Some(frame)) => Ok(Some(vec![frame].into())),
        Ok(None) => Ok(None),
        Err(super::InstantiationFailure::Unresolved) if control.is_some() => {
            let evidence = query
                .result_evidence
                .as_ref()
                .ok_or(super::InstantiationFailure::Unresolved)?;
            let site = nocter_model::ValueApplication {
                owner: caller.code(),
                call: id,
            };
            // Dormant contract hypotheses have no original committed invocation recipe.
            // Their unresolved context is not a missing-call integrity failure.
            if evidence.invocation(site).is_none() {
                return Err(super::InstantiationFailure::Unresolved);
            }
            let domain = super::results::instantiate_controlled_invocation_domain(
                query.program(),
                evidence,
                caller,
                site,
                control.ok_or(super::InstantiationFailure::Unresolved)?,
                work,
            )?;
            let mut frames = std::collections::BTreeSet::new();
            for case in domain.cases() {
                work.step()
                    .map_err(|_| super::InstantiationFailure::WorkLimit)?;
                frames.insert(case.frame().clone());
            }
            if frames.is_empty() {
                return Err(super::InstantiationFailure::Unresolved);
            }
            Ok(Some(frames.into_iter().collect()))
        }
        Err(failure) => Err(failure),
    }
}

/// Close a dormant definition contract without claiming occurrence or creation evidence.
/// Reached calls use the original-recipe resolver instead.
/// # Errors
/// Rejects symbolic/missing witnesses, incompatible selected views or incomplete formal coverage.
fn instantiate_contract_call(
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
        caller.resolve(program, witness).map_err(|_| invalid)
    })?;
    program
        .bind_instance(&selected, bindings)
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

pub(super) fn is_completed_call(expression: &super::ExpressionKind) -> bool {
    matches!(expression, super::ExpressionKind::Call { arguments, .. } if completed(arguments))
}

fn select_contract_call(
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
    instantiate_contract_call(program, caller, *target, *kind, arguments).map(Some)
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

fn call_bindings<W>(
    arguments: &[Argument],
    resolve: impl Fn(FunctionWitness) -> Result<W, flow::IntegrityError>,
) -> Result<Vec<(nocter_model::ValueParameter, W)>, flow::IntegrityError> {
    arguments
        .iter()
        .filter_map(|argument| argument.binding.as_ref())
        .map(|binding| Ok((binding.parameter, resolve(binding.witness)?)))
        .collect()
}
