use super::*;
use nocter_declarations::value_access::{DataReference, FieldDefinition, Reservations};
use nocter_model::{ArenaBuilder, SymbolTable, ValueField};

struct Fixture {
    program: Program,
    data: TypeId,
    leaf: TypeId,
    locals: [Holder; 6],
    expressions: [AccessExpressionId; 6],
    head: ValueField,
    left: ValueField,
    right: ValueField,
    tail: ValueField,
    a: ValueField,
    b: ValueField,
}
impl Fixture {
    fn new() -> Self {
        let symbols = SymbolTable::from_spellings([
            "Leaf", "Outer", "a", "b", "head", "left", "right", "tail",
        ]);
        let mut reservations = Reservations::new(symbols.clone());
        let leaf = reservations.reserve_record(symbols.get("Leaf").unwrap(), 2);
        let outer = reservations.reserve_record(symbols.get("Outer").unwrap(), 4);
        let mut builder = reservations.finish().unwrap();
        let field = |name: &str, data| FieldDefinition {
            name: symbols.get(name).unwrap(),
            data,
        };
        builder
            .define_record(
                leaf,
                false,
                [
                    field("a", DataReference::Builtin(BuiltinType::Error)),
                    field("b", DataReference::Builtin(BuiltinType::Error)),
                ],
            )
            .unwrap();
        builder
            .define_record(
                outer,
                false,
                [
                    field("head", DataReference::Builtin(BuiltinType::Error)),
                    field("left", DataReference::Nominal(leaf)),
                    field("right", DataReference::Nominal(leaf)),
                    field("tail", DataReference::Builtin(BuiltinType::Error)),
                ],
            )
            .unwrap();
        let program = builder.finish().unwrap();
        let record = program.record(outer).unwrap();
        let data = record.data();
        let [head, left, right, tail] = ["head", "left", "right", "tail"].map(|s| {
            record
                .resolve_field(symbols.get(s).unwrap())
                .unwrap()
                .into()
        });
        let record = program.record(leaf).unwrap();
        let leaf = record.data();
        let [a, b] = ["a", "b"].map(|s| {
            record
                .resolve_field(symbols.get(s).unwrap())
                .unwrap()
                .into()
        });
        let mut locals = ArenaBuilder::<AccessLocalId, ()>::new();
        let locals = std::array::from_fn(|_| Holder::Local(locals.insert(())));
        let mut expressions = ArenaBuilder::<AccessExpressionId, ()>::new();
        let expressions = std::array::from_fn(|_| expressions.insert(()));
        Self {
            program,
            data,
            leaf,
            locals,
            expressions,
            head,
            left,
            right,
            tail,
            a,
            b,
        }
    }
    fn root(&self) -> Place {
        self.locals[0].place()
    }
    fn state(&self) -> State {
        let mut state = State::default();
        self.construct(&mut state, 0, self.data);
        state
    }
    fn construct(&self, state: &mut State, local: usize, data: TypeId) {
        state
            .construct(
                self.locals[local],
                payload(&self.program, data).unwrap(),
                &mut Vec::new(),
            )
            .unwrap();
    }
    fn move_to(&self, state: &mut State, source: &Place, destination: &Place) -> Vec<Step> {
        let mut steps = Vec::new();
        state
            .move_to(&self.program, source, destination, false, &mut steps)
            .unwrap();
        steps
    }
    fn initialized(&self, state: &State, place: &Place) -> bool {
        state.initialized(&self.program, place).unwrap()
    }
}

#[test]
fn assembly_transfers_components_once_and_invalid_input_preserves_the_entire_ledger() {
    let f = Fixture::new();
    let mut state = State::default();
    let error = f.program.data_type(BuiltinType::Error).unwrap();
    f.construct(&mut state, 1, error);
    f.construct(&mut state, 2, error);
    let record = payload(&f.program, f.leaf).unwrap();
    let destination = f.locals[3];
    let fields = [(f.b, f.locals[2].place()), (f.a, f.locals[1].place())];
    for invalid in [
        vec![fields[0].clone()],
        vec![fields[0].clone(), fields[0].clone()],
        vec![fields[0].clone(), (f.head, fields[1].1.clone())],
        vec![(f.b, fields[1].1.clone()), fields[1].clone()],
    ] {
        let before = state.clone();
        let mut steps = Vec::new();
        assert!(
            state
                .assemble(&f.program, destination, record, &invalid, &mut steps)
                .is_err()
        );
        assert_eq!(state, before);
        assert!(steps.is_empty());
    }
    let mut steps = Vec::new();
    state
        .assemble(&f.program, destination, record, &fields, &mut steps)
        .unwrap();
    assert!(!state.contains(f.locals[1]) && !state.contains(f.locals[2]));
    assert!(state.initialized(&f.program, &destination.place()).unwrap());
    assert!(
        matches!(steps.as_slice(), [Step::Assemble { fields: selected, .. }] if selected.as_ref() == fields)
    );
}

#[test]
fn field_move_keeps_disjoint_siblings_and_replacement_restores_the_complete_value() {
    let f = Fixture::new();
    let original = f.state();
    let mut state = original.clone();
    let source = f.root().project(f.left).project(f.a);
    let destination = f.locals[1].place();
    let steps = f.move_to(&mut state, &source, &destination);
    assert!(state.contains(f.locals[0]));
    assert!(!f.initialized(&state, &source));
    assert!(!f.initialized(&state, &f.root()));
    assert!(!f.initialized(&state, &f.root().project(f.left)));
    assert!(f.initialized(&state, &f.root().project(f.left).project(f.b)));
    assert!(f.initialized(&state, &f.root().project(f.right)));
    assert!(
        matches!(&steps[..], [Step::Move { source: s, destination: d, payload }] if s == &source && d == &destination && payload.disposal() == Disposal::OwnedError)
    );
    assert!(state.take(&f.program, &f.root()).is_err());
    assert!(state.take(&f.program, &f.root().project(f.left)).is_err());
    f.move_to(&mut state, &destination, &source);
    assert_eq!(state, original);
}

#[test]
fn partial_cleanup_orders_surviving_regions_and_skips_transferred_fields() {
    let f = Fixture::new();
    let mut state = f.state();
    f.move_to(
        &mut state,
        &f.root().project(f.left).project(f.a),
        &f.locals[1].place(),
    );
    f.move_to(
        &mut state,
        &f.root().project(f.right).project(f.b),
        &f.locals[2].place(),
    );
    let mut steps = Vec::new();
    state.dispose(&f.program, &f.root(), &mut steps).unwrap();
    let expected = [
        f.root().project(f.tail),
        f.root().project(f.right).project(f.a),
        f.root().project(f.left).project(f.b),
        f.root().project(f.head),
    ];
    assert_eq!(steps.len(), expected.len() + 3); // two partial leaves and their parent end
    let disposed: Vec<_> = steps
        .iter()
        .filter(|step| matches!(step, Step::Dispose { .. }))
        .collect();
    for (step, expected) in disposed.iter().zip(expected) {
        assert!(
            matches!(step, Step::Dispose { source, payload } if source == &expected && payload.disposal() == Disposal::OwnedError)
        );
    }
    assert!(!state.contains(f.locals[0]));
    assert!(f.initialized(&state, &f.locals[1].place()));
    assert!(f.initialized(&state, &f.locals[2].place()));
    assert!(state.dispose(&f.program, &f.root(), &mut steps).is_err());
}

#[test]
fn replacing_a_partial_parent_cleans_only_its_remainder_and_keeps_other_holes() {
    let f = Fixture::new();
    let mut state = f.state();
    f.move_to(
        &mut state,
        &f.root().project(f.left).project(f.a),
        &f.locals[1].place(),
    );
    let missing = f.root().project(f.right).project(f.b);
    f.move_to(&mut state, &missing, &f.locals[2].place());
    f.construct(&mut state, 3, f.leaf);
    let mut steps = Vec::new();
    state
        .replace(
            &f.program,
            &f.locals[3].place(),
            &f.root().project(f.left),
            &mut steps,
        )
        .unwrap();
    assert!(
        matches!(&steps[..], [Step::Dispose { source, .. }, Step::ReleaseContainer { .. }, Step::Move { destination, payload, .. }]
        if source == &f.root().project(f.left).project(f.b) && destination == &f.root().project(f.left) && payload.data() == f.leaf)
    );
    assert!(f.initialized(&state, &f.root().project(f.left)));
    assert!(!f.initialized(&state, &missing));
    assert!(!f.initialized(&state, &f.root()));
    assert!(!state.contains(f.locals[3]));
}

#[test]
fn a_moved_parent_cannot_be_reconstructed_by_writing_a_deeper_field() {
    let f = Fixture::new();
    let mut state = f.state();
    let parent = f.root().project(f.left);
    f.move_to(&mut state, &parent, &f.locals[1].place());
    f.construct(
        &mut state,
        2,
        f.program.data_type(BuiltinType::Error).unwrap(),
    );
    let before = state.clone();
    let mut steps = Vec::new();
    assert!(
        state
            .replace(
                &f.program,
                &f.locals[2].place(),
                &parent.project(f.a),
                &mut steps
            )
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());
    f.move_to(&mut state, &f.locals[1].place(), &parent);
    assert!(f.initialized(&state, &f.root()));
}

#[test]
fn moving_all_children_keeps_their_parent_container_available_for_restoration() {
    let f = Fixture::new();
    let mut state = f.state();
    let parent = f.root().project(f.left);
    f.move_to(&mut state, &parent.project(f.a), &f.locals[1].place());
    f.move_to(&mut state, &parent.project(f.b), &f.locals[2].place());
    assert_eq!(state.owned.get(&f.locals[0]).unwrap().missing.len(), 2);
    f.move_to(&mut state, &f.locals[1].place(), &parent.project(f.a));
    assert!(f.initialized(&state, &parent.project(f.a)));
    assert!(!f.initialized(&state, &parent));
    f.move_to(&mut state, &f.locals[2].place(), &parent.project(f.b));
    assert_eq!(state, f.state());
}

#[test]
fn field_ownership_and_logical_overlap_never_depend_on_names_or_physical_offsets() {
    let f = Fixture::new();
    let left = f.root().project(f.left);
    let right = f.root().project(f.right);
    assert!(f.root().overlaps(&left));
    assert!(left.overlaps(&left.project(f.a)));
    assert!(!left.overlaps(&right));
    assert!(!left.project(f.a).overlaps(&right.project(f.a)));
    assert!(!left.project(f.a).overlaps(&left.project(f.b)));
    assert!(!left.overlaps(&f.locals[1].place().project(f.left)));
    assert_eq!(left.fields().data(&f.program, f.data), Some(f.leaf));
    assert_eq!(f.program.project_data(f.data, &FieldPath::new([f.a])), None);
    assert_eq!(
        f.program
            .project_data(f.data, &FieldPath::new([f.left, f.right])),
        None
    );
    assert_eq!(
        f.program
            .project_data(f.data, &FieldPath::new([f.head, f.a])),
        None
    );
    let mut state = f.state();
    let before = state.clone();
    assert!(state.take(&f.program, &f.root().project(f.a)).is_err());
    assert_eq!(state, before);
}

#[test]
fn failed_transfers_replacements_and_frame_operations_leave_no_partial_effects() {
    let f = Fixture::new();
    let mut state = f.state();
    f.construct(
        &mut state,
        1,
        f.program.data_type(BuiltinType::I32).unwrap(),
    );
    f.construct(
        &mut state,
        2,
        f.program.data_type(BuiltinType::Error).unwrap(),
    );
    let before = state.clone();
    let mut steps = Vec::new();
    let destination = f.root().project(f.head);
    assert!(
        state
            .replace(&f.program, &f.locals[1].place(), &destination, &mut steps)
            .is_err()
    );
    assert_eq!(state, before);
    assert!(
        state
            .move_to(
                &f.program,
                &f.locals[2].place(),
                &destination,
                false,
                &mut steps
            )
            .is_err()
    );
    assert_eq!(state, before);
    assert!(
        state
            .move_to(
                &f.program,
                &f.locals[2].place(),
                &Holder::Expression(f.expressions[0]).place(),
                false,
                &mut steps
            )
            .is_err()
    );
    assert_eq!(state, before);
    assert!(
        state
            .replace(&f.program, &f.locals[1].place(), &f.root(), &mut steps)
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());
    assert!(
        state
            .construct(
                Holder::Expression(f.expressions[0]),
                payload(&f.program, f.leaf).unwrap(),
                &mut steps
            )
            .is_err()
    );
    assert_eq!(state, before);
    state.begin(f.expressions[0]).unwrap();
    state
        .construct(
            Holder::Expression(f.expressions[1]),
            payload(&f.program, f.leaf).unwrap(),
            &mut steps,
        )
        .unwrap();
    let before = state.clone();
    assert!(state.end(f.expressions[0]).is_err());
    assert_eq!(state, before);
    assert!(state.begin(f.expressions[0]).is_err());
    assert_eq!(state, before);
}

#[test]
fn intact_records_stay_as_one_schema_cleanup_and_empty_partial_containers_drop_no_field_twice() {
    let f = Fixture::new();
    let mut state = f.state();
    let mut steps = Vec::new();
    state.dispose(&f.program, &f.root(), &mut steps).unwrap();
    assert!(
        matches!(&steps[..], [Step::Dispose { source, payload }] if source == &f.root() && payload.disposal() == Disposal::Aggregate && payload.data() == f.data)
    );
    let mut state = f.state();
    for (index, field) in [f.head, f.left, f.right, f.tail].into_iter().enumerate() {
        f.move_to(
            &mut state,
            &f.root().project(field),
            &f.locals[index + 1].place(),
        );
    }
    assert!(state.contains(f.locals[0]));
    assert!(!state.initialized_root(f.locals[0]));
    let mut steps = Vec::new();
    state.dispose(&f.program, &f.root(), &mut steps).unwrap();
    assert!(matches!(&steps[..], [Step::ReleaseContainer { source, .. }] if source == &f.root()));
    assert!(!state.contains(f.locals[0]));
    for &holder in &f.locals[1..5] {
        assert!(state.initialized_root(holder));
    }
}

#[test]
fn disjoint_move_order_converges_to_identical_sparse_state_for_every_field_subset() {
    let f = Fixture::new();
    let fields = [f.head, f.left, f.right, f.tail];
    for mask in 0..16 {
        let mut forward = f.state();
        let mut reverse = f.state();
        for (index, &field) in fields.iter().enumerate() {
            if mask & (1 << index) != 0 {
                f.move_to(
                    &mut forward,
                    &f.root().project(field),
                    &f.locals[index + 1].place(),
                );
            }
        }
        for (index, &field) in fields.iter().enumerate().rev() {
            if mask & (1 << index) != 0 {
                f.move_to(
                    &mut reverse,
                    &f.root().project(field),
                    &f.locals[index + 1].place(),
                );
            }
        }
        assert_eq!(forward, reverse);
        let mut steps = Vec::new();
        forward.dispose(&f.program, &f.root(), &mut steps).unwrap();
        let expected: Vec<_> = if mask == 0 {
            vec![f.root()]
        } else {
            (0..4)
                .rev()
                .filter(|&index| mask & (1 << index) == 0)
                .map(|index| f.root().project(fields[index]))
                .collect()
        };
        let actual: Vec<_> = steps
            .iter()
            .filter_map(|step| match step {
                Step::Dispose { source, .. } => Some(source.clone()),
                Step::ReleaseContainer { .. } => None,
                _ => panic!("cleanup emitted an unrelated operation"),
            })
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn partial_temporaries_keep_their_evaluation_frame_and_reverse_cleanup_order() {
    let f = Fixture::new();
    let [outer, value, inner, moved, ..] = f.expressions;
    let mut state = State::default();
    state.begin(outer).unwrap();
    state
        .construct(
            Holder::Expression(value),
            payload(&f.program, f.data).unwrap(),
            &mut Vec::new(),
        )
        .unwrap();
    state.begin(inner).unwrap();
    state
        .move_to(
            &f.program,
            &Holder::Expression(value)
                .place()
                .project(f.left)
                .project(f.a),
            &Holder::Expression(moved).place(),
            false,
            &mut Vec::new(),
        )
        .unwrap();
    assert_eq!(state.ending(inner).unwrap(), [Holder::Expression(moved)]);
    state
        .dispose(
            &f.program,
            &Holder::Expression(moved).place(),
            &mut Vec::new(),
        )
        .unwrap();
    state.end(inner).unwrap();
    assert_eq!(state.ending(outer).unwrap(), [Holder::Expression(value)]);
    let mut steps = Vec::new();
    state
        .dispose(&f.program, &Holder::Expression(value).place(), &mut steps)
        .unwrap();
    assert_eq!(steps.len(), 6); // tail, right, left.b, end left, head, end root
    assert!(
        matches!(&steps[1], Step::Dispose { payload, .. } if payload.disposal() == Disposal::Aggregate)
    );
    state.end(outer).unwrap();
    assert!(state.owned.is_empty());
}

#[test]
fn selected_invocation_cannot_transfer_an_incomplete_parent() {
    let f = Fixture::new();
    let mut state = f.state();
    let mut steps = Vec::new();
    let first = f.root().project(f.left).project(f.a);
    state
        .invoke(&f.program, &first, f.expressions[0], 0, &mut steps)
        .unwrap();
    let before = state.clone();
    assert!(
        state
            .invoke(&f.program, &f.root(), f.expressions[0], 1, &mut steps)
            .is_err()
    );
    assert_eq!(state, before);
    assert!(matches!(&steps[..], [Step::Invoke { source, .. }] if source == &first));
    assert!(!state.returned(&f.program, f.data));
    assert!(
        state
            .move_to(
                &f.program,
                &f.root(),
                &Holder::Return.place(),
                false,
                &mut steps
            )
            .is_err()
    );
}

#[test]
fn deeply_shared_payload_cleanup_visits_only_the_selected_frontier_without_recursion() {
    const COUNT: usize = 128;
    let names: Vec<_> = (0..COUNT).map(|i| format!("R{i:03}")).collect();
    let symbols =
        SymbolTable::from_spellings(names.iter().map(String::as_str).chain(["left", "right"]));
    let mut reservations = Reservations::new(symbols.clone());
    let ids: Vec<_> = names
        .iter()
        .map(|s| {
            reservations.reserve_record(symbols.get(s).unwrap(), if s == &names[0] { 0 } else { 2 })
        })
        .collect();
    let mut builder = reservations.finish().unwrap();
    builder.define_record(ids[0], false, []).unwrap();
    for pair in ids.windows(2) {
        builder
            .define_record(
                pair[1],
                false,
                [
                    FieldDefinition {
                        name: symbols.get("left").unwrap(),
                        data: DataReference::Nominal(pair[0]),
                    },
                    FieldDefinition {
                        name: symbols.get("right").unwrap(),
                        data: DataReference::Nominal(pair[0]),
                    },
                ],
            )
            .unwrap();
    }
    let program = builder.finish().unwrap();
    let mut locals = ArenaBuilder::<AccessLocalId, ()>::new();
    let root = Holder::Local(locals.insert(()));
    let moved = Holder::Local(locals.insert(()));
    let mut source = root.place();
    for &id in ids[1..].iter().rev() {
        source = source.project(
            program
                .record(id)
                .unwrap()
                .resolve_field(symbols.get("left").unwrap())
                .unwrap()
                .into(),
        );
    }
    let mut state = State::default();
    state
        .construct(
            root,
            payload(
                &program,
                program.record(*ids.last().unwrap()).unwrap().data(),
            )
            .unwrap(),
            &mut Vec::new(),
        )
        .unwrap();
    state
        .move_to(&program, &source, &moved.place(), false, &mut Vec::new())
        .unwrap();
    assert_eq!(state.owned.get(&root).unwrap().missing.len(), 1);
    let mut steps = Vec::new();
    state.dispose(&program, &root.place(), &mut steps).unwrap();
    assert_eq!(steps.len(), 2 * (COUNT - 1));
    assert_eq!(
        steps
            .iter()
            .filter(|step| matches!(step, Step::Dispose { .. }))
            .count(),
        COUNT - 1
    );
    assert!(steps.iter().all(|step| matches!(step, Step::Dispose { payload, .. } | Step::ReleaseContainer { payload, .. } if payload.disposal() == Disposal::Aggregate)));
    assert!(state.initialized_root(moved));
}

#[test]
fn transition_initialization_distinguishes_a_partial_owner_from_a_complete_value() {
    let f = Fixture::new();
    let before = f.state();
    let mut after = before.clone();
    let steps = f.move_to(
        &mut after,
        &f.root().project(f.left).project(f.a),
        &f.locals[1].place(),
    );
    let mut points = ArenaBuilder::<nocter_model::AccessFlowPointId, ()>::new();
    let at = Location {
        point: points.insert(()),
        action: 0,
    };
    let mut report = Report::default();
    report.record(
        at,
        &before,
        &super::super::StepPlan::new(steps, 0, before.condition()),
        &after,
    );
    let transition = report.at(at).next().unwrap();
    assert!(transition.initialized_before(f.locals[0]));
    assert!(!transition.initialized_after(f.locals[0]));
    assert!(after.contains(f.locals[0]));
    assert!(!transition.initialized_before(f.locals[1]));
    assert!(transition.initialized_after(f.locals[1]));
}

#[test]
fn whole_record_replacement_cleans_the_partial_remainder_before_installing_the_new_value() {
    let f = Fixture::new();
    let mut state = f.state();
    f.move_to(
        &mut state,
        &f.root().project(f.left).project(f.a),
        &f.locals[1].place(),
    );
    f.construct(&mut state, 2, f.data);
    let mut steps = Vec::new();
    state
        .replace(&f.program, &f.locals[2].place(), &f.root(), &mut steps)
        .unwrap();
    assert_eq!(steps.len(), 7);
    assert!(
        steps[..6]
            .iter()
            .all(|step| matches!(step, Step::Dispose { .. } | Step::ReleaseContainer { .. }))
    );
    assert!(
        matches!(&steps[6], Step::Move { source, destination, payload } if source == &f.locals[2].place() && destination == &f.root() && payload.data() == f.data)
    );
    assert!(f.initialized(&state, &f.root()));
    assert!(state.owned.get(&f.locals[0]).unwrap().missing.is_empty());
    assert!(!state.contains(f.locals[2]));
    assert!(f.initialized(&state, &f.locals[1].place()));
}
