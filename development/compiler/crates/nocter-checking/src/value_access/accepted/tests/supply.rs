//! Source-backed phase ordering and snapshots at the selected input preparation.
use super::support::{source_case, steps};
use super::*;
use crate::value_access::{ArgumentAction, BoundaryAction, ExpressionKind, Query as TypedQuery};

fn pending(query: &TypedQuery) -> bool {
    query
        .body()
        .expressions()
        .iter()
        .any(|(_, expression)| match &expression.kind {
            ExpressionKind::Boundary {
                action: BoundaryAction::PendingValue,
                ..
            } => true,
            ExpressionKind::Call { arguments, .. } => arguments
                .iter()
                .any(|argument| matches!(argument.action, ArgumentAction::PendingValue(_))),
            _ => false,
        })
}

#[test]
fn pending_supply_closes_before_initialization_without_rebuilding_control() {
    for case in [
        "a106-callable-result-copy.nct",
        "a108-callable-result-take-input.nct",
        "a121-result-snapshot-before-drop.nct",
    ] {
        let (sources, tree) = input(&source_case(case));
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let context = headers.bind(bound(&sources, &tree)).unwrap();
        let mut queries = context
            .program()
            .codes()
            .iter()
            .map(|(id, _)| (id.into(), TypedQuery::prepare(&context, id).unwrap()))
            .collect::<BTreeMap<_, _>>();
        crate::value_access::types::canonicalize(&context, &mut queries).unwrap();
        let mut controls = queries
            .into_iter()
            .map(|(id, query)| (id, flow::Control::prepare(Arc::new(query)).unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert!(
            controls.values().any(|control| pending(&control.typed)),
            "{case}"
        );
        let graphs = controls
            .iter()
            .map(|(&id, control)| (id, control.graph.clone()))
            .collect::<BTreeMap<_, _>>();
        crate::value_access::results::close(&context, &mut controls).unwrap();
        let writes = flow::call_writes::Catalog::prepare(controls.values()).unwrap();
        for (id, control) in controls {
            assert!(!pending(&control.typed), "{case}");
            assert_eq!(control.graph, graphs[&id]);
            let complete = control.finish(&writes).unwrap();
            assert!(complete.initialization().violations().is_empty(), "{case}");
            assert_eq!(complete.access().status(), flow::access::Status::Analyzed);
            assert!(complete.access().violations().is_empty(), "{case}");
        }
    }
}

fn snapshot(query: &flow::Query) -> (usize, Vec<Step>) {
    let (call, input) = query
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            let ExpressionKind::Call { arguments, .. } = &expression.kind else {
                return None;
            };
            let first = arguments.first()?;
            matches!(first.action, ArgumentAction::CopyPlace(_)).then_some((id, first.value))
        })
        .unwrap();
    let mut preparations = 0;
    for (point, block) in query.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            assert!(!matches!(event.operation, Operation::Evaluate(id) if id == input));
            if event.operation != (Operation::PrepareArgument { call, argument: 0 }) {
                continue;
            }
            preparations += 1;
            let alternatives = query
                .access()
                .resources()
                .at(Location { point, action })
                .collect::<Vec<_>>();
            assert!(!alternatives.is_empty());
            for transition in alternatives {
                let snapshots = transition.steps().iter().filter(|step| matches!(step,
                    Step::Construct { destination: Holder::Prepared { call: selected, index: 0 }, .. } if *selected == call
                )).count();
                assert_eq!(snapshots, 1);
            }
        }
    }
    assert_eq!(preparations, 1);
    let trace = steps(query);
    let snapshot = trace.iter().position(|step| matches!(step,
        Step::Construct { destination: Holder::Prepared { call: selected, index: 0 }, .. } if *selected == call
    )).unwrap();
    (snapshot, trace)
}

#[test]
fn a_direct_copy_input_snapshots_once_before_a_later_argument_updates_its_source() {
    let unit = accepted(&source_case("a120-copy-input-before-update.nct"));
    let (snapshot, trace) = snapshot(named(&unit, "run"));
    let nested_result = trace
        .iter()
        .position(|step| matches!(step, Step::Receive { .. }))
        .unwrap();
    assert!(snapshot < nested_result);
}

#[test]
fn a_returned_copy_environment_snapshots_once_before_a_later_argument_drops_its_source() {
    let unit = accepted(&source_case("a121-result-snapshot-before-drop.nct"));
    let (snapshot, trace) = snapshot(named(&unit, "run"));
    let dropped = trace
        .iter()
        .position(|step| {
            matches!(step,
                Step::Dispose { source, .. } if matches!(source.holder(), Holder::Local(_))
            )
        })
        .unwrap();
    assert!(snapshot < dropped);
}
