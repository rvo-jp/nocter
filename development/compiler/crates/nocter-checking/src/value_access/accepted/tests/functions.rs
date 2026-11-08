//! Source-backed static witnesses, invocation views and ordinary environment preparation.
use super::support::{source_case, steps};
use super::*;
use crate::value_access::{ArgumentAction, ExpressionKind};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{ValueInputOperation, ValueInvocation};

#[test]
fn mut_view_keeps_static_dispatch_and_prepares_the_environment_before_explicit_inputs() {
    let unit = accepted(&source_case("a63-function-mut-view.nct"));
    let query = named(&unit, "run");
    let calls = query
        .typed()
        .body()
        .expressions()
        .iter()
        .filter_map(|(_, expression)| {
            if let ExpressionKind::Call {
                target,
                ref arguments,
                ..
            } = expression.kind
            {
                Some((target, arguments))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        unit.program().symbols().spelling(
            unit.program()
                .callables()
                .get(match calls[0].0 {
                    nocter_model::ValueFunctionOrigin::Declaration(id) => id,
                    nocter_model::ValueFunctionOrigin::Parameter(_)
                    | nocter_model::ValueFunctionOrigin::Result(_) => panic!("concrete target"),
                })
                .unwrap()
                .name()
        ),
        Some("update")
    );
    assert_eq!(calls[0].1.len(), 2);
    for argument in calls[0].1 {
        assert_eq!(argument.operation, ValueInputOperation::Edit);
        assert!(matches!(argument.action, ArgumentAction::ReserveUpdate(_)));
    }
    let trace = steps(query);
    let views = trace
        .iter()
        .filter_map(|step| match step {
            Step::AdaptFunction { from, payload, .. } => Some((from, payload)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(views.len(), 1);
    let Some(PayloadSchema::Function(from)) = unit.program().data_schema(views[0].0.data()) else {
        panic!("named source");
    };
    let Some(PayloadSchema::Function(to)) = unit.program().data_schema(views[0].1.data()) else {
        panic!("named target");
    };
    assert_eq!(from.origin, to.origin);
    assert_eq!(
        (from.invocation, to.invocation),
        (ValueInvocation::Read, ValueInvocation::Edit)
    );
    let constructions = trace
        .iter()
        .filter(|step| match step {
            Step::Construct { payload, .. } => matches!(
                unit.program().data_schema(payload.data()),
                Some(PayloadSchema::Function(_))
            ),
            _ => false,
        })
        .count();
    assert_eq!(constructions, 1);
}

#[test]
fn environment_preparation_preserves_the_explicit_take_input_and_its_commit_index() {
    let unit = accepted(&source_case("a65-function-take-input.nct"));
    let query = named(&unit, "run");
    let arguments = query
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(_, expression)| {
            if let ExpressionKind::Call { ref arguments, .. } = expression.kind {
                Some(arguments)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(arguments[0].operation, ValueInputOperation::Read);
    assert!(matches!(
        arguments[0].action,
        ArgumentAction::ReserveRead(_)
    ));
    assert_eq!(arguments[1].operation, ValueInputOperation::Take);
    assert!(matches!(
        arguments[1].action,
        ArgumentAction::ReserveTransfer(_)
    ));
    let invoked = steps(query)
        .into_iter()
        .filter_map(|step| match step {
            Step::Invoke { index, payload, .. } => Some((index, payload.disposal())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(invoked, [(1, Disposal::OwnedError)]);
}

#[test]
fn consuming_copy_view_snapshots_before_later_destruction_of_the_original() {
    let unit = accepted(&source_case("a70-function-snapshot-before-drop.nct"));
    let trace = steps(named(&unit, "run"));
    let prepared = trace
        .iter()
        .position(|step| {
            matches!(
                step,
                Step::Construct {
                    destination: Holder::Prepared { index: 0, .. },
                    ..
                }
            )
        })
        .unwrap();
    let dropped = trace
        .iter()
        .position(|step| match step {
            Step::Dispose { source, payload } if matches!(source.holder(), Holder::Local(_)) => {
                matches!(
                    unit.program().data_schema(payload.data()),
                    Some(PayloadSchema::Function(_))
                )
            }
            _ => false,
        })
        .unwrap();
    let invoked = trace
        .iter()
        .position(|step| matches!(step, Step::Invoke { index: 0, .. }))
        .unwrap();
    assert!(prepared < dropped && dropped < invoked);
    let disposals = trace
        .iter()
        .filter(|step| match step {
            Step::Dispose { source, payload } if matches!(source.holder(), Holder::Local(_)) => {
                matches!(
                    unit.program().data_schema(payload.data()),
                    Some(PayloadSchema::Function(_))
                )
            }
            _ => false,
        })
        .count();
    assert_eq!(disposals, 1);
}

#[test]
fn interrupted_input_releases_its_reserved_environment_without_invocation_or_result() {
    let unit = accepted(&source_case("a69-function-call-interruption.nct"));
    let query = named(&unit, "run");
    assert!(query.graph().points().iter().any(|(_, point)| {
        point
            .actions
            .iter()
            .any(|action| matches!(action.operation, Operation::CancelCall { prepared: 1, .. }))
    }));
    let trace = steps(query);
    assert!(
        !trace
            .iter()
            .any(|step| matches!(step, Step::Invoke { .. } | Step::Receive { .. }))
    );
    let disposals = trace
        .iter()
        .filter(|step| match step {
            Step::Dispose { payload, .. } => matches!(
                unit.program().data_schema(payload.data()),
                Some(PayloadSchema::Function(_))
            ),
            _ => false,
        })
        .count();
    assert_eq!(disposals, 1);
}
