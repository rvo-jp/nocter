//! Saved-index ownership, sparse cleanup, restoration and atomic rejection.
use super::*;
use crate::value_access::{
    ExpressionKind,
    accepted::{SourceUnit, Unit},
    tests::{bound, input},
};

fn unit() -> Unit {
    prepare(
        "func work(take values: [error; 2], take replacement: [error; 2], index: usize): error {\n\
         read first = values[index]\nread second = values[index]\nvalues[0]^\n}\n\
         func other(take values: [error; 2], take replacement: [error; 2], index: usize): error {\n\
         read first = values[index]\nvalues[0]^\n}",
    )
}
fn prepare(source: &str) -> Unit {
    let (sources, tree) = input(source);
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
                && holes == [Missing::Value(source(typed, index).fields().clone())])
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

#[test]
fn remainder_cleanup_uses_backing_until_both_spans_and_the_container_end() {
    let unit = unit();
    let typed = query(&unit, "work");
    let index = indices(typed)[0];
    let mut state = State::entry(typed).unwrap();
    operand(&mut state, typed, index, BuiltinType::Usize);
    extract(&mut state, typed, index);
    state.end(index.selection()).unwrap();
    let mut steps = Vec::new();
    let root = source(typed, index).holder().place();
    state.dispose(typed.program(), &root, &mut steps).unwrap();
    assert!(matches!(steps.as_slice(), [
        Step::DisposeArrayElements { source: high, range: nocter_model::ArrayRange::After(a), .. },
        Step::DisposeArrayElements { source: low, range: nocter_model::ArrayRange::Before(b), .. },
        Step::ReleaseContainer { source: container, .. },
        Step::Dispose { source: snapshot, .. },
    ] if high == &root && low == high && container == high && *a == IndexBacking::Selection(index) && *b == IndexBacking::Selection(index) && snapshot.holder() == Holder::IndexSnapshot(index)));
    assert!(!state.contains(root.holder()));
    assert!(!state.contains(Holder::IndexSnapshot(index)));
    assert!(state.contains(Holder::Return));
}

#[test]
fn whole_replacement_retires_the_old_snapshot_without_affecting_the_moved_element() {
    let unit = unit();
    let typed = query(&unit, "work");
    let index = indices(typed)[0];
    let mut state = State::entry(typed).unwrap();
    operand(&mut state, typed, index, BuiltinType::Usize);
    extract(&mut state, typed, index);
    state.end(index.selection()).unwrap();
    let root = source(typed, index).holder().place();
    let replacement = Holder::Local(typed.body().inputs().explicit()[1].local).place();
    let mut steps = Vec::new();
    state
        .replace(typed.program(), &replacement, &root, &mut steps)
        .unwrap();
    assert!(matches!(steps.as_slice(), [
        Step::DisposeArrayElements { range: nocter_model::ArrayRange::After(a), .. },
        Step::DisposeArrayElements { range: nocter_model::ArrayRange::Before(b), .. },
        Step::ReleaseContainer { .. },
        Step::Move { source, destination, .. },
        Step::Dispose { source: snapshot, .. },
    ] if *a == IndexBacking::Selection(index) && *b == IndexBacking::Selection(index) && source == &replacement && destination == &root && snapshot.holder() == Holder::IndexSnapshot(index)));
    assert!(state.initialized_root(root.holder()));
    assert!(!state.contains(replacement.holder()));
    assert!(!state.contains(Holder::IndexSnapshot(index)));
    assert!(state.contains(Holder::Return));
}

#[test]
fn possible_overlap_with_a_missing_ancestor_never_grants_concrete_child_cleanup() {
    let unit = prepare(
        "func work(take values: [[error; 2]; 2], index: usize): [error; 2] { read selected = values[index]\nvalues[0]^ }",
    );
    let typed = query(&unit, "work");
    let index = indices(typed)[0];
    let root = source(typed, index).holder().place();
    let concrete = root.project(nocter_model::ValueField::Array(0));
    let mut state = State::entry(typed).unwrap();
    operand(&mut state, typed, index, BuiltinType::Usize);
    extract(&mut state, typed, index);
    state.end(index.selection()).unwrap();
    let before = state.clone();
    let mut steps = Vec::new();
    assert!(
        state
            .dispose(typed.program(), &concrete, &mut steps)
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());

    // A partial inner-array hole must likewise not become a full cleanup interval merely
    // because its unknown outer selector is not a proven prefix of this concrete child.
    let data = typed
        .body()
        .locals()
        .get(typed.body().inputs().explicit()[0].local)
        .unwrap()
        .data;
    let mut value = OwnedValue::new(payload(typed.program(), data).unwrap());
    value.remove(
        &source(typed, index)
            .fields()
            .project(nocter_model::ValueField::Array(0)),
    );
    assert!(value.cleanup(typed.program(), &concrete).is_err());
}

#[test]
fn partial_selected_element_cleans_its_remaining_field_between_intact_spans() {
    let unit = prepare(
        "struct Pair { first: error\nsecond: error }\nfunc work(take values: [Pair; 2], index: usize): void { read selected = values[index] }",
    );
    let typed = query(&unit, "work");
    let index = indices(typed)[0];
    let element = source(typed, index);
    let root = element.holder().place();
    let root_data = typed
        .body()
        .locals()
        .get(typed.body().inputs().explicit()[0].local)
        .unwrap()
        .data;
    let PayloadSchema::Array(array) = typed.program().data_schema(root_data).unwrap() else {
        panic!()
    };
    let PayloadSchema::Aggregate(pair) = typed.program().data_schema(array.element()).unwrap()
    else {
        panic!()
    };
    let fields: Vec<_> = pair.components().map(|field| field.field).collect();
    let first = element.project(fields[0]);
    let second = element.project(fields[1]);
    let mut state = State::entry(typed).unwrap();
    let original = state.clone();
    // A restored partial field ends the snapshot, allowing this same recipe to capture again.
    for restore in [true, false] {
        operand(&mut state, typed, index, BuiltinType::Usize);
        let mut steps = Vec::new();
        state
            .move_to(
                typed.program(),
                &first,
                &Holder::Return.place(),
                false,
                &mut steps,
            )
            .unwrap();
        assert!(
            matches!(steps.as_slice(), [Step::Move { destination, .. }, Step::Move { source, .. }]
            if destination.holder() == Holder::IndexSnapshot(index) && source == &first)
        );
        state.end(index.selection()).unwrap();
        if restore {
            state
                .move_to(
                    typed.program(),
                    &Holder::Return.place(),
                    &first,
                    false,
                    &mut steps,
                )
                .unwrap();
            assert_eq!(state, original);
        }
    }
    let mut steps = Vec::new();
    state.dispose(typed.program(), &root, &mut steps).unwrap();
    assert!(matches!(steps.as_slice(), [
        Step::DisposeArrayElements { range: nocter_model::ArrayRange::After(a), .. },
        Step::Dispose { source: remaining, payload },
        Step::ReleaseContainer { source: selected, .. },
        Step::DisposeArrayElements { range: nocter_model::ArrayRange::Before(b), .. },
        Step::ReleaseContainer { source: container, .. },
        Step::Dispose { source: snapshot, .. },
    ] if *a == IndexBacking::Selection(index) && *b == *a
        && remaining == &second && payload.disposal() == Disposal::OwnedError
        && selected == &element && container == &root
        && snapshot.holder() == Holder::IndexSnapshot(index)));
    assert!(state.contains(Holder::Return));
    assert!(!state.contains(root.holder()));
    assert!(!state.contains(Holder::IndexSnapshot(index)));
}

fn nested_unit() -> Unit {
    prepare(
        "func work(take values: [[error; 3]; 2], row: usize, column: usize): void { read selected = values[row][column] }",
    )
}

#[test]
fn nested_capture_rolls_back_outer_snapshot_when_the_inner_operand_is_invalid() {
    let unit = nested_unit();
    let typed = query(&unit, "work");
    let indices = indices(typed);
    assert_eq!(indices.len(), 2);
    let mut state = State::entry(typed).unwrap();
    operand(&mut state, typed, indices[0], BuiltinType::Usize);
    operand(&mut state, typed, indices[1], BuiltinType::Bool);
    let nested = source(typed, indices[0]).project_retained(indices[1]);
    let before = state.clone();
    let mut steps = Vec::new();
    assert!(
        state
            .move_to(
                typed.program(),
                &nested,
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
fn nested_snapshots_follow_one_hole_through_restoration_and_reverse_cleanup() {
    let unit = nested_unit();
    let typed = query(&unit, "work");
    let indices = indices(typed);
    let outer = indices[0];
    let inner = indices[1];
    assert_eq!((outer.bound(), inner.bound()), (2, 3));
    let row = source(typed, outer);
    let selected = row.project_retained(inner);
    let root = row.holder().place();
    let mut state = State::entry(typed).unwrap();
    let original = state.clone();
    for restore in [true, false] {
        for &index in &indices {
            operand(&mut state, typed, index, BuiltinType::Usize);
        }
        let mut steps = Vec::new();
        state
            .move_to(
                typed.program(),
                &selected,
                &Holder::Return.place(),
                false,
                &mut steps,
            )
            .unwrap();
        for &index in indices.iter().rev() {
            state.end(index.selection()).unwrap();
            assert!(state.contains(Holder::IndexSnapshot(index)));
        }
        if restore {
            state
                .move_to(
                    typed.program(),
                    &Holder::Return.place(),
                    &selected,
                    false,
                    &mut steps,
                )
                .unwrap();
            assert_eq!(state, original);
        }
    }
    let mut steps = Vec::new();
    state.dispose(typed.program(), &root, &mut steps).unwrap();
    assert!(matches!(steps.as_slice(), [
        Step::DisposeArrayElements { source: high, range: nocter_model::ArrayRange::After(a), .. },
        Step::DisposeArrayElements { source: inner_high, range: nocter_model::ArrayRange::After(b), .. },
        Step::DisposeArrayElements { source: inner_low, range: nocter_model::ArrayRange::Before(c), .. },
        Step::ReleaseContainer { source: inner_container, .. },
        Step::DisposeArrayElements { source: low, range: nocter_model::ArrayRange::Before(d), .. },
        Step::ReleaseContainer { source: container, .. },
        Step::Dispose { source: first_snapshot, .. }, Step::Dispose { source: second_snapshot, .. },
    ] if high == &root && low == high && container == high
        && inner_high == &row && inner_low == inner_high && inner_container == inner_high
        && *a == IndexBacking::Selection(outer) && *d == *a
        && *b == IndexBacking::Selection(inner) && *c == *b
        && first_snapshot.holder() == Holder::IndexSnapshot(outer)
        && second_snapshot.holder() == Holder::IndexSnapshot(inner)));
    assert!(state.contains(Holder::Return));
    assert!(!state.contains(root.holder()));
    for index in indices {
        assert!(!state.contains(Holder::IndexSnapshot(index)));
    }
}
