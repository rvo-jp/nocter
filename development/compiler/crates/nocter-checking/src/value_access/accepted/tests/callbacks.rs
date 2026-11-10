//! Source-to-unit evidence for formal witness binding and environment ownership.
use super::support::{source_case, steps};
use super::*;
use crate::value_access::{Argument, ArgumentAction, ExpressionKind};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{ValueFunctionOrigin as Origin, ValueInputOperation, ValueInvocation};

fn calls(query: &flow::Query) -> Vec<(Origin, &[Argument])> {
    query
        .typed()
        .body()
        .expressions()
        .iter()
        .filter_map(|(_, expression)| match &expression.kind {
            ExpressionKind::Call {
                target, arguments, ..
            } => Some((*target, arguments.as_ref())),
            _ => None,
        })
        .collect()
}

#[test]
fn source_calls_bind_exact_named_and_formal_witnesses_without_erasure() {
    let unit = accepted(&source_case("a82-callback-forward-read.nct"));
    let run = calls(named(&unit, "run"));
    let binding = run[0].1[0].binding.as_ref().unwrap();
    let Origin::Declaration(concrete) = binding.witness.origin else {
        panic!("named binding");
    };
    assert_eq!(
        unit.program().symbols().spelling(
            unit.program()
                .codes()
                .get(concrete)
                .unwrap()
                .name()
                .unwrap()
        ),
        Some("increment")
    );
    let forward = calls(named(&unit, "forward"));
    let forwarded = forward[0].1[0].binding.as_ref().unwrap();
    assert_eq!(
        forwarded.witness.origin,
        Origin::Parameter(binding.parameter)
    );
    let apply = calls(named(&unit, "apply"));
    assert_eq!(apply[0].0, Origin::Parameter(forwarded.parameter));
    assert_eq!(apply[0].1[0].operation, ValueInputOperation::Read);
    assert!(matches!(
        apply[0].1[0].action,
        ArgumentAction::ReserveRead(_)
    ));
    assert!(apply[0].1[0].binding.is_none());
    let formal = unit.program().parameters().get(binding.parameter).unwrap();
    assert_eq!(unit.program().data_copyable(formal.data()), Some(false));
}

#[test]
fn equal_signatures_preserve_distinct_call_site_witnesses_and_positions() {
    let unit = accepted(&source_case("a89-callback-two-witnesses.nct"));
    let run = calls(named(&unit, "run"));
    let left = run[0].1[0].binding.as_ref().unwrap();
    let right = run[0].1[1].binding.as_ref().unwrap();
    assert_ne!(left.parameter, right.parameter);
    assert_ne!(left.witness.origin, right.witness.origin);
    let body_calls = calls(named(&unit, "apply"));
    assert!(
        body_calls
            .iter()
            .any(|(target, _)| *target == Origin::Parameter(left.parameter))
    );
    assert!(
        body_calls
            .iter()
            .any(|(target, _)| *target == Origin::Parameter(right.parameter))
    );
}

#[test]
fn recursive_forwarding_keeps_a_finite_identity_edge() {
    let unit = accepted(&source_case("a88-callback-recursive-forward.nct"));
    let query = named(&unit, "apply");
    let all = calls(query);
    let (_, arguments) = all
        .iter()
        .find(|(target, _)| matches!(target, Origin::Declaration(_)))
        .unwrap();
    let binding = arguments[0].binding.as_ref().unwrap();
    assert_eq!(binding.witness.origin, Origin::Parameter(binding.parameter));
    assert_eq!(all.len(), 2);
}

#[test]
fn consuming_formal_invocation_uses_its_symbolic_environment_once() {
    let unit = accepted(&source_case("a79-callback-once-take.nct"));
    let query = named(&unit, "apply");
    let all = calls(query);
    assert_eq!(all[0].1[0].operation, ValueInputOperation::Take);
    assert!(matches!(
        all[0].1[0].action,
        ArgumentAction::ReserveTransfer { .. }
    ));
    let trace = steps(query);
    let invoked = trace
        .iter()
        .filter_map(|step| match step {
            Step::Invoke { index, payload, .. }
                if payload.disposal() == Disposal::FunctionEnvironment =>
            {
                Some(*index)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(invoked, [0]);
    assert!(!trace.iter().any(|step| matches!(step, Step::Dispose { payload, .. } if payload.disposal() == Disposal::FunctionEnvironment)));
}

#[test]
fn formal_adapter_preserves_origin_and_destruction_obligation() {
    let unit = accepted(&source_case("a84-callback-forward-adapter.nct"));
    let trace = steps(named(&unit, "forward"));
    let views = trace
        .iter()
        .filter_map(|step| match step {
            Step::AdaptFunction { from, payload, .. } => Some((from, payload)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].0.disposal(), Disposal::FunctionEnvironment);
    assert_eq!(views[0].1.disposal(), Disposal::FunctionEnvironment);
    let Some(PayloadSchema::Function(from)) = unit.program().data_schema(views[0].0.data()) else {
        panic!("formal source");
    };
    let Some(PayloadSchema::Function(to)) = unit.program().data_schema(views[0].1.data()) else {
        panic!("formal target");
    };
    assert_eq!(from.origin, to.origin);
    assert!(matches!(from.origin, Origin::Parameter(_)));
    assert_eq!(
        (from.invocation, to.invocation),
        (ValueInvocation::Read, ValueInvocation::Take)
    );
    let all = calls(named(&unit, "forward"));
    assert_eq!(all[0].1[0].binding.as_ref().unwrap().witness, to);
}

#[test]
fn early_return_retains_symbolic_cleanup_without_forcing_invocation() {
    let unit = accepted(&source_case("a94-callback-take-cleanup-on-return.nct"));
    let trace = steps(named(&unit, "apply"));
    assert!(trace.iter().any(|step| matches!(step, Step::Dispose { payload, .. } if payload.disposal() == Disposal::FunctionEnvironment)));
    assert!(trace.iter().any(|step| matches!(step, Step::Invoke { payload, .. } if payload.disposal() == Disposal::FunctionEnvironment)));
}

#[test]
fn terminating_input_needs_no_unreachable_environment_binding_or_invocation() {
    let unit = accepted(&source_case("a96-callback-terminated-witness.nct"));
    let query = named(&unit, "run");
    let all = calls(query);
    assert!(all[0].1[0].binding.is_none());
    assert_eq!(all[0].1[0].action, ArgumentAction::Interrupted);
    assert!(
        !steps(query)
            .iter()
            .any(|step| matches!(step, Step::Invoke { .. } | Step::Receive { .. }))
    );
}
