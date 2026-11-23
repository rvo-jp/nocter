//! Logical ledger evidence only; source array access and dynamic-index proofs remain gated.
use super::*;
use nocter_declarations::value_access::{DataReference, FieldDefinition, Reservations};
use nocter_model::{ArenaBuilder, SymbolTable, ValueField};
mod construction;

struct Fixture {
    program: Program,
    data: TypeId,
    locals: [Holder; 5],
    fields: [ValueField; 2],
}
impl Fixture {
    fn new(length: u64, compound: bool) -> Self {
        let symbols = SymbolTable::from_spellings(["Leaf", "Data", "first", "second", "items"]);
        let mut reservations = Reservations::new(symbols.clone());
        let leaf = reservations.reserve_record(symbols.get("Leaf").unwrap(), 2);
        let record = reservations.reserve_record(symbols.get("Data").unwrap(), 1);
        let mut builder = reservations.finish().unwrap();
        builder
            .define_record(
                leaf,
                false,
                ["first", "second"].map(|name| FieldDefinition {
                    name: symbols.get(name).unwrap(),
                    data: BuiltinType::Error.into(),
                }),
            )
            .unwrap();
        builder
            .define_record(
                record,
                false,
                [FieldDefinition {
                    name: symbols.get("items").unwrap(),
                    data: DataReference::Array {
                        element: std::sync::Arc::new(if compound {
                            DataReference::Nominal(leaf)
                        } else {
                            BuiltinType::Error.into()
                        }),
                        length,
                    },
                }],
            )
            .unwrap();
        let program = builder.finish().unwrap();
        let data = program
            .fields()
            .get(program.record(record).unwrap().fields()[0])
            .unwrap()
            .data();
        let fields =
            std::array::from_fn(|index| program.record(leaf).unwrap().fields()[index].into());
        let mut locals = ArenaBuilder::<AccessLocalId, ()>::new();
        let locals = std::array::from_fn(|_| Holder::Local(locals.insert(())));
        Self {
            program,
            data,
            locals,
            fields,
        }
    }
    fn root(&self) -> Place {
        self.locals[0].place()
    }
    fn element(&self, index: u64) -> Place {
        self.root().project(ValueField::Array(index))
    }
    fn state(&self) -> State {
        let mut state = State::default();
        state
            .construct(
                self.locals[0],
                payload(&self.program, self.data).unwrap(),
                &mut Vec::new(),
            )
            .unwrap();
        state
    }
}

#[test]
fn a_hole_in_a_maximum_length_array_leaves_two_reverse_cleanup_spans() {
    let fixture = Fixture::new(u64::MAX, false);
    let program = &fixture.program;
    let mut state = fixture.state();
    state
        .move_to(
            program,
            &fixture.element(1),
            &fixture.locals[1].place(),
            false,
            &mut Vec::new(),
        )
        .unwrap();
    assert!(!state.initialized_root(fixture.locals[0]));
    assert!(state.initialized(program, &fixture.element(0)).unwrap());
    assert!(!state.initialized(program, &fixture.element(1)).unwrap());
    assert!(
        state
            .initialized(program, &fixture.element(u64::MAX - 1))
            .unwrap()
    );
    let mut steps = Vec::new();
    state.dispose(program, &fixture.root(), &mut steps).unwrap();
    assert!(matches!(steps.as_slice(), [
        Step::DisposeArrayElements { source: high, payload: a, range: nocter_model::ArrayRange::Constant { start: 2, end: u64::MAX } },
        Step::DisposeArrayElements { source: low, payload: b, range: nocter_model::ArrayRange::Constant { start: 0, end: 1 } },
        Step::ReleaseContainer { source: root, .. },
    ] if high == &fixture.root() && low == high && root == high && a == b && a.data() == fixture.data));
    assert!(!state.contains(fixture.locals[0]));
    assert!(state.contains(fixture.locals[1]));
    let mut projections = 0;
    let projected = steps[0]
        .try_map_payloads::<(), _>(|value| {
            projections += 1;
            Ok(*value)
        })
        .unwrap();
    assert_eq!(projected, steps[0]);
    assert_eq!(projections, 1);
}

#[test]
fn restoring_a_hole_and_replacing_a_live_element_have_distinct_cleanup() {
    let fixture = Fixture::new(4, false);
    let program = &fixture.program;
    let mut state = fixture.state();
    let saved = fixture.locals[1].place();
    let replacement = fixture.locals[2].place();
    let error = payload(program, program.data_type(BuiltinType::Error).unwrap()).unwrap();
    state
        .move_to(program, &fixture.element(1), &saved, false, &mut Vec::new())
        .unwrap();
    state
        .construct(fixture.locals[2], error, &mut Vec::new())
        .unwrap();
    let mut steps = Vec::new();
    state
        .replace(program, &replacement, &fixture.element(1), &mut steps)
        .unwrap();
    assert!(matches!(steps.as_slice(), [Step::Move { .. }]));
    assert!(state.initialized_root(fixture.locals[0]));
    steps.clear();
    state
        .replace(program, &saved, &fixture.element(1), &mut steps)
        .unwrap();
    assert!(
        matches!(steps.as_slice(), [Step::Dispose { source, .. }, Step::Move { .. }] if source == &fixture.element(1))
    );
    steps.clear();
    state.dispose(program, &fixture.root(), &mut steps).unwrap();
    assert!(
        matches!(steps.as_slice(), [Step::Dispose { source, payload }] if source == &fixture.root() && payload.data() == fixture.data)
    );
}

#[test]
fn partially_moved_element_fields_are_cleaned_between_surviving_array_spans() {
    let fixture = Fixture::new(4, true);
    let program = &fixture.program;
    let mut state = fixture.state();
    state
        .move_to(
            program,
            &fixture.element(0),
            &fixture.locals[1].place(),
            false,
            &mut Vec::new(),
        )
        .unwrap();
    state
        .move_to(
            program,
            &fixture.element(2).project(fixture.fields[0]),
            &fixture.locals[2].place(),
            false,
            &mut Vec::new(),
        )
        .unwrap();
    let mut steps = Vec::new();
    state.dispose(program, &fixture.root(), &mut steps).unwrap();
    assert!(matches!(steps.as_slice(), [
        Step::DisposeArrayElements { range: nocter_model::ArrayRange::Constant { start: 3, end: 4 }, .. },
        Step::Dispose { source: remaining, payload },
        Step::ReleaseContainer { source: element, .. },
        Step::DisposeArrayElements { range: nocter_model::ArrayRange::Constant { start: 1, end: 2 }, .. },
        Step::ReleaseContainer { source: array, .. },
    ] if remaining == &fixture.element(2).project(fixture.fields[1]) && element == &fixture.element(2) && array == &fixture.root() && payload.disposal() == Disposal::OwnedError));
}

#[test]
fn absent_array_containers_cannot_be_restored_by_writing_an_element() {
    let fixture = Fixture::new(2, false);
    let program = &fixture.program;
    let mut state = fixture.state();
    for index in 0..2 {
        state
            .move_to(
                program,
                &fixture.element(index),
                &fixture.locals[usize::try_from(index).unwrap() + 1].place(),
                false,
                &mut Vec::new(),
            )
            .unwrap();
    }
    let mut steps = Vec::new();
    state.dispose(program, &fixture.root(), &mut steps).unwrap();
    assert!(matches!(steps.as_slice(), [Step::ReleaseContainer { .. }]));
    let before = state.clone();
    steps.clear();
    assert!(
        state
            .replace(
                program,
                &fixture.locals[1].place(),
                &fixture.element(0),
                &mut steps
            )
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());
}

#[test]
fn empty_and_out_of_bounds_positions_never_supply_elements() {
    for length in [0, 1, u64::MAX] {
        let fixture = Fixture::new(length, false);
        let mut state = fixture.state();
        let before = state.clone();
        assert!(
            state
                .take(&fixture.program, &fixture.element(length))
                .is_err()
        );
        assert_eq!(state, before);
        let mut steps = Vec::new();
        state
            .dispose(&fixture.program, &fixture.root(), &mut steps)
            .unwrap();
        assert!(matches!(steps.as_slice(), [Step::Dispose { .. }]));
    }
}

#[test]
fn moving_the_last_possible_element_never_overflows_or_emits_an_empty_span() {
    let fixture = Fixture::new(u64::MAX, false);
    let mut state = fixture.state();
    state
        .move_to(
            &fixture.program,
            &fixture.element(u64::MAX - 1),
            &fixture.locals[1].place(),
            false,
            &mut Vec::new(),
        )
        .unwrap();
    let mut steps = Vec::new();
    state
        .dispose(&fixture.program, &fixture.root(), &mut steps)
        .unwrap();
    assert!(
        matches!(steps.as_slice(), [Step::DisposeArrayElements { range: nocter_model::ArrayRange::Constant { start: 0, end }, .. }, Step::ReleaseContainer { .. }] if *end == u64::MAX - 1)
    );
}

#[test]
fn zero_sized_elements_keep_separate_logical_availability() {
    let symbols = SymbolTable::from_spellings(["Data", "items"]);
    let mut reservations = Reservations::new(symbols.clone());
    let owner = reservations.reserve_record(symbols.get("Data").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            owner,
            false,
            [FieldDefinition {
                name: symbols.get("items").unwrap(),
                data: DataReference::Array {
                    element: std::sync::Arc::new(BuiltinType::Void.into()),
                    length: 2,
                },
            }],
        )
        .unwrap();
    let program = builder.finish().unwrap();
    let data = program
        .fields()
        .get(program.record(owner).unwrap().fields()[0])
        .unwrap()
        .data();
    let mut locals = ArenaBuilder::<AccessLocalId, ()>::new();
    let root = Holder::Local(locals.insert(()));
    let saved = Holder::Local(locals.insert(()));
    let mut state = State::default();
    state
        .construct(root, payload(&program, data).unwrap(), &mut Vec::new())
        .unwrap();
    state
        .move_to(
            &program,
            &root.place().project(ValueField::Array(0)),
            &saved.place(),
            false,
            &mut Vec::new(),
        )
        .unwrap();
    assert!(
        !state
            .initialized(&program, &root.place().project(ValueField::Array(0)))
            .unwrap()
    );
    assert!(
        state
            .initialized(&program, &root.place().project(ValueField::Array(1)))
            .unwrap()
    );
    let mut steps = Vec::new();
    state.dispose(&program, &root.place(), &mut steps).unwrap();
    assert!(matches!(
        steps.as_slice(),
        [
            Step::DisposeArrayElements {
                range: nocter_model::ArrayRange::Constant { start: 1, end: 2 },
                ..
            },
            Step::ReleaseContainer { .. }
        ]
    ));
}

#[test]
fn array_cleanup_below_a_record_field_retains_each_container_boundary() {
    let fixture = Fixture::new(3, false);
    let program = &fixture.program;
    let record = program
        .record(
            program
                .resolve_nominal(program.symbols().get("Data").unwrap())
                .unwrap(),
        )
        .unwrap();
    let mut state = State::default();
    state
        .construct(
            fixture.locals[0],
            payload(program, record.data()).unwrap(),
            &mut Vec::new(),
        )
        .unwrap();
    let array = fixture.root().project(record.fields()[0].into());
    state
        .move_to(
            program,
            &array.project(ValueField::Array(1)),
            &fixture.locals[1].place(),
            false,
            &mut Vec::new(),
        )
        .unwrap();
    let mut steps = Vec::new();
    state.dispose(program, &fixture.root(), &mut steps).unwrap();
    assert!(matches!(steps.as_slice(), [
        Step::DisposeArrayElements { source: high, range: nocter_model::ArrayRange::Constant { start: 2, end: 3 }, .. },
        Step::DisposeArrayElements { source: low, range: nocter_model::ArrayRange::Constant { start: 0, end: 1 }, .. },
        Step::ReleaseContainer { source: inner, .. },
        Step::ReleaseContainer { source: outer, .. },
    ] if high == &array && low == &array && inner == &array && outer == &fixture.root()));
}
