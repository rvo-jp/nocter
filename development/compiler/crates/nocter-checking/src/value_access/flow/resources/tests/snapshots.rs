//! Ledger qualification; source dynamic transfer and executable cleanup remain gated.
use super::*;
use crate::value_access::{
    ExpressionKind,
    accepted::{SourceUnit, Unit},
    tests::{bound, input},
};

fn unit() -> Unit {
    let (sources, tree) = input(
        "func work(take values: [error; 2], index: usize): error {\n\
         read first = values[index]\nread second = values[index]\nvalues[0]^\n}\n\
         func other(take values: [error; 2], index: usize): error {\n\
         read first = values[index]\nvalues[0]^\n}",
    );
    SourceUnit::prepare(bound(&sources, &tree))
        .unwrap()
        .into_unit()
}
fn query<'a>(unit: &'a Unit, name: &str) -> &'a TypedQuery {
    let code = unit
        .program()
        .resolve(unit.program().symbols().get(name).unwrap())
        .unwrap();
    unit.bodies().find(|(id, _)| *id == code).unwrap().1.typed()
}
fn indices(typed: &TypedQuery) -> Vec<RetainedIndex> {
    typed
        .body()
        .expressions()
        .iter()
        .filter(|(_, expression)| {
            matches!(
                expression.kind,
                ExpressionKind::Index {
                    bounds: nocter_model::IndexBoundsCheck::Required,
                    ..
                }
            )
        })
        .map(|(id, _)| RetainedIndex::for_selection(typed, id).unwrap())
        .collect()
}
fn source(typed: &TypedQuery, index: RetainedIndex) -> Place {
    Holder::Local(typed.body().inputs().explicit()[0].local)
        .place()
        .project_retained(index)
}
fn operand(state: &mut State, typed: &TypedQuery, index: RetainedIndex, kind: BuiltinType) {
    state.begin(index.selection()).unwrap();
    let data = typed.program().data_type(kind).unwrap();
    state
        .construct(
            Holder::Expression(index.operand()),
            payload(typed.program(), data).unwrap(),
            &mut Vec::new(),
        )
        .unwrap();
}
fn extract(state: &mut State, typed: &TypedQuery, index: RetainedIndex) -> Vec<Step> {
    let mut steps = Vec::new();
    state
        .move_to(
            typed.program(),
            &source(typed, index),
            &Holder::Return.place(),
            false,
            &mut steps,
        )
        .unwrap();
    steps
}
#[test]
fn snapshot_and_hole_commit_together_and_outlive_the_operand_frame() {
    let unit = unit();
    let typed = query(&unit, "work");
    let index = indices(typed)[0];
    let mut state = State::entry(typed).unwrap();
    operand(&mut state, typed, index, BuiltinType::Usize);
    let steps = extract(&mut state, typed, index);
    assert!(matches!(steps.as_slice(), [
        Step::Move { source: scalar, destination: snapshot, payload },
        Step::Move { source: element, destination: result, .. },
    ] if scalar.holder() == Holder::Expression(index.operand())
        && snapshot.holder() == Holder::IndexSnapshot(index)
        && payload.disposal() == Disposal::Trivial
        && element == &source(typed, index)
        && result.holder() == Holder::Return));
    assert!(!state.contains(Holder::Expression(index.operand())));
    assert!(state.ending(index.selection()).unwrap().is_empty());
    state.end(index.selection()).unwrap();
    assert!(state.contains(Holder::IndexSnapshot(index)));
    assert!(
        state
            .condition()
            .owned()
            .any(|(holder, holes)| holder == Holder::IndexSnapshot(index) && holes.is_empty())
    );
    assert!(
        state
            .condition()
            .owned()
            .any(|(holder, holes)| holder == source(typed, index).holder()
                && holes == [source(typed, index).fields().clone()])
    );
    assert!(
        !state
            .initialized(typed.program(), &source(typed, index))
            .unwrap()
    );
    let before = state.clone();
    assert!(
        state
            .dispose(
                typed.program(),
                &Holder::IndexSnapshot(index).place(),
                &mut Vec::new()
            )
            .is_err()
    );
    assert_eq!(state, before);
}
#[test]
fn restoration_retires_backing_after_its_last_use_and_permits_later_recipe_reuse() {
    let unit = unit();
    let typed = query(&unit, "work");
    let index = indices(typed)[0];
    let mut state = State::entry(typed).unwrap();
    let original = state.clone();
    for _ in 0..2 {
        operand(&mut state, typed, index, BuiltinType::Usize);
        extract(&mut state, typed, index);
        state.end(index.selection()).unwrap();
        let mut steps = Vec::new();
        state
            .move_to(
                typed.program(),
                &Holder::Return.place(),
                &source(typed, index),
                false,
                &mut steps,
            )
            .unwrap();
        assert!(matches!(steps.as_slice(), [
            Step::Move { destination, .. }, Step::Dispose { source: snapshot, .. }
        ] if destination == &source(typed, index) && snapshot.holder() == Holder::IndexSnapshot(index)));
        assert_eq!(state, original);
    }
}
#[test]
fn failed_capture_or_destination_install_keeps_the_entire_ledger_and_step_list() {
    let unit = unit();
    let typed = query(&unit, "work");
    let index = indices(typed)[0];
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
                .move_to(
                    typed.program(),
                    &source(typed, index),
                    &Holder::Return.place(),
                    false,
                    &mut steps
                )
                .is_err()
        );
        assert_eq!(state, before);
        assert!(steps.is_empty());
    }
}
#[test]
fn live_backing_and_other_body_descriptors_cannot_be_recaptured() {
    let unit = unit();
    let typed = query(&unit, "work");
    let index = indices(typed)[0];
    let mut state = State::entry(typed).unwrap();
    operand(&mut state, typed, index, BuiltinType::Usize);
    extract(&mut state, typed, index);
    state.end(index.selection()).unwrap();
    operand(&mut state, typed, index, BuiltinType::Usize);
    let before = state.clone();
    let mut steps = Vec::new();
    assert!(
        state
            .invoke(
                typed.program(),
                &source(typed, index),
                index.selection(),
                0,
                &mut steps
            )
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());
    let mut state = State::entry(typed).unwrap();
    let foreign = indices(query(&unit, "other"))[0];
    operand(&mut state, typed, foreign, BuiltinType::Usize);
    let before = state.clone();
    assert!(
        state
            .move_to(
                typed.program(),
                &source(typed, foreign),
                &Holder::Return.place(),
                false,
                &mut steps
            )
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());
}

#[test]
fn invocation_keeps_snapshot_ownership_and_distinct_alternatives_keep_distinct_backing() {
    let unit = unit();
    let typed = query(&unit, "work");
    let indices = indices(typed);
    assert_eq!(indices.len(), 2);
    let mut conditions = Vec::new();
    for &index in &indices {
        let mut state = State::entry(typed).unwrap();
        operand(&mut state, typed, index, BuiltinType::Usize);
        let mut steps = Vec::new();
        state
            .invoke(
                typed.program(),
                &source(typed, index),
                index.selection(),
                0,
                &mut steps,
            )
            .unwrap();
        assert!(matches!(steps.as_slice(), [
            Step::Move { destination, .. }, Step::Invoke { source: element, .. },
        ] if destination.holder() == Holder::IndexSnapshot(index) && element == &source(typed, index)));
        state.end(index.selection()).unwrap();
        assert!(state.contains(Holder::IndexSnapshot(index)));
        conditions.push(state.condition());
    }
    // The ledger intentionally knows no numbers. Even if both evaluate to the same number,
    // their snapshots' owning presence distinguishes the operation's runtime alternatives.
    assert_ne!(conditions[0], conditions[1]);
    for (condition, &index) in conditions.iter().zip(&indices) {
        assert!(
            condition
                .owned()
                .any(|(holder, _)| holder == Holder::IndexSnapshot(index))
        );
        let other = indices.iter().find(|&&other| other != index).unwrap();
        assert!(
            !condition
                .owned()
                .any(|(holder, _)| holder == Holder::IndexSnapshot(*other))
        );
    }
}
