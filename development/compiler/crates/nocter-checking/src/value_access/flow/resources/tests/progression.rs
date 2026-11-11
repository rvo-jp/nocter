//! Logical prefix coverage/backing; source execution is qualified by the shared array corpus.
use super::*;
use crate::value_access::{
    ExpressionKind, Referent,
    accepted::{SourceUnit, Unit},
    flow::{Location, Operation, Query},
    tests::{bound, input},
};

fn unit() -> Unit {
    let body = "(take values: [error; 3]): void { var index: usize = 0\n\
                while index < 3 { let selected = values[index]^\nindex += 1 } }";
    let (sources, tree) = input(&format!("func work{body}\nfunc other{body}"));
    SourceUnit::prepare(bound(&sources, &tree))
        .unwrap()
        .into_unit()
}
fn query<'a>(unit: &'a Unit, name: &str) -> &'a Query {
    let code = unit
        .program()
        .resolve(unit.program().symbols().get(name).unwrap())
        .unwrap();
    unit.bodies().find(|(id, _)| *id == code).unwrap().1
}
fn selection(query: &Query) -> (RetainedIndex, ArrayFrontier) {
    for (point, body) in query.graph().points().iter() {
        for (action, item) in body.actions.iter().enumerate() {
            if let Operation::Evaluate(id) = item.operation
                && let ExpressionKind::Move(Referent::Evaluated(selection)) =
                    query.typed().body().expressions().get(id).unwrap().kind
            {
                let frontier = query
                    .numeric_at(Location { point, action })
                    .unwrap()
                    .prefix_frontier(selection)
                    .unwrap();
                return (
                    RetainedIndex::for_selection(query.typed(), selection).unwrap(),
                    frontier,
                );
            }
        }
    }
    panic!("missing source-proved prefix transfer");
}
fn operand(state: &mut State, typed: &TypedQuery, index: RetainedIndex, kind: BuiltinType) {
    state.begin(index.selection()).unwrap();
    state
        .construct(
            Holder::Expression(index.operand()),
            payload(typed.program(), typed.program().data_type(kind).unwrap()).unwrap(),
            &mut Vec::new(),
        )
        .unwrap();
}
fn advance(
    state: &mut State,
    typed: &TypedQuery,
    index: RetainedIndex,
    frontier: ArrayFrontier,
) -> Vec<Step> {
    operand(state, typed, index, BuiltinType::Usize);
    let mut steps = Vec::new();
    state
        .advance_array(typed, index, frontier, Holder::Return, &mut steps)
        .unwrap();
    assert!(state.ending(index.selection()).unwrap().is_empty());
    state.end(index.selection()).unwrap();
    steps
}

#[test]
fn repeated_prefix_delivery_replaces_one_unframed_endpoint_after_the_payload() {
    let unit = unit();
    let query = query(&unit, "work");
    let typed = query.typed();
    let (index, frontier) = selection(query);
    let endpoint = Holder::ArrayFrontier(frontier);
    let mut state = State::entry(typed).unwrap();
    for iteration in 0..3 {
        let steps = advance(&mut state, typed, index, frontier);
        assert!(
            matches!(steps.first(), Some(Step::MoveArrayElement { index: actual, destination: Holder::Return, .. }) if *actual == index)
        );
        assert!(
            matches!(steps.last(), Some(Step::ConsolidateArrayPrefix { frontier: actual, .. }) if *actual == frontier)
        );
        let middle = &steps[1..steps.len() - 1];
        if iteration == 0 {
            assert!(
                matches!(middle, [Step::Move { destination, .. }] if destination.holder() == endpoint)
            );
        } else {
            assert!(
                matches!(middle, [Step::Dispose { source, .. }, Step::Move { destination, .. }] if source.holder() == endpoint && destination.holder() == endpoint)
            );
        }
        assert!(state.contains(endpoint));
        assert!(!state.contains(Holder::IndexSnapshot(index)));
        assert_eq!(state.condition().owned().count(), 3);
        assert!(state.condition().owned().any(|(holder, holes)| holder
            == index.array(typed).unwrap().holder()
            && holes
                == [Missing::ArrayPrefix {
                    array: Path::default(),
                    frontier
                }]));
        if iteration < 2 {
            state
                .dispose(typed.program(), &Holder::Return.place(), &mut Vec::new())
                .unwrap();
        }
    }
    let root = index.array(typed).unwrap();
    let mut steps = Vec::new();
    state.dispose(typed.program(), &root, &mut steps).unwrap();
    assert!(matches!(steps.as_slice(), [
        Step::DisposeArrayElements { range: nocter_model::ArrayRange::After(IndexBacking::Frontier(actual)), .. },
        Step::ReleaseContainer { .. }, Step::Dispose { source, .. }
    ] if *actual == frontier && source.holder() == endpoint));
    assert!(!state.contains(endpoint));
    assert!(state.contains(Holder::Return));
}

#[test]
fn prefix_transition_failures_publish_neither_partial_ownership_nor_steps() {
    let unit = unit();
    let query = query(&unit, "work");
    let typed = query.typed();
    let (index, frontier) = selection(query);
    for kind in [None, Some(BuiltinType::Bool), Some(BuiltinType::Usize)] {
        let mut state = State::entry(typed).unwrap();
        if let Some(kind) = kind {
            operand(&mut state, typed, index, kind);
        }
        if kind == Some(BuiltinType::Usize) {
            state
                .construct(
                    Holder::Return,
                    payload(
                        typed.program(),
                        typed.program().data_type(BuiltinType::Error).unwrap(),
                    )
                    .unwrap(),
                    &mut Vec::new(),
                )
                .unwrap();
        }
        let before = state.clone();
        let mut steps = Vec::new();
        assert!(
            state
                .advance_array(typed, index, frontier, Holder::Return, &mut steps)
                .is_err()
        );
        assert_eq!(state, before);
        assert!(steps.is_empty());
    }
}

#[test]
fn point_holes_and_foreign_frontiers_cannot_be_reinterpreted_as_this_prefix() {
    let unit = unit();
    let query = query(&unit, "work");
    let typed = query.typed();
    let (index, frontier) = selection(query);
    let (_, foreign) = selection(self::query(&unit, "other"));
    let mut state = State::entry(typed).unwrap();
    operand(&mut state, typed, index, BuiltinType::Usize);
    let before = state.clone();
    let mut steps = Vec::new();
    assert!(
        state
            .advance_array(typed, index, foreign, Holder::Return, &mut steps)
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());
    let root = index.array(typed).unwrap();
    state
        .move_to(
            typed.program(),
            &root.project_retained(index),
            &Holder::Return.place(),
            false,
            &mut steps,
        )
        .unwrap();
    state.end(index.selection()).unwrap();
    state
        .dispose(typed.program(), &Holder::Return.place(), &mut Vec::new())
        .unwrap();
    operand(&mut state, typed, index, BuiltinType::Usize);
    let before = state.clone();
    let mut steps = Vec::new();
    assert!(
        state
            .advance_array(typed, index, frontier, Holder::Return, &mut steps)
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());
}

#[test]
fn ordinary_value_operations_cannot_create_or_consume_a_live_frontier() {
    let unit = unit();
    let query = query(&unit, "work");
    let typed = query.typed();
    let (index, frontier) = selection(query);
    let endpoint = Holder::ArrayFrontier(frontier);
    let mut state = State::entry(typed).unwrap();
    advance(&mut state, typed, index, frontier);
    let before = state.clone();
    let mut steps = Vec::new();
    assert!(
        state
            .construct(
                endpoint,
                payload(
                    typed.program(),
                    typed.program().data_type(BuiltinType::Usize).unwrap()
                )
                .unwrap(),
                &mut steps
            )
            .is_err()
    );
    assert!(state.take(typed.program(), &endpoint.place()).is_err());
    assert!(
        state
            .dispose(typed.program(), &endpoint.place(), &mut steps)
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());
}
