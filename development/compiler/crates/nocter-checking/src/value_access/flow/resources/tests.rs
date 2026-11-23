use super::*;
use crate::value_access::tests::queries;
mod enums;

#[test]
fn an_uninhabited_representation_cannot_become_an_owned_payload() {
    let queries = queries("func run(value: i32): i32 { value }").unwrap();
    let program = queries[0].program();
    let data = program.data_type(BuiltinType::I32).unwrap();
    let empty = program.instantiate_uninhabited_data(data).unwrap();
    assert!(
        payload(program, data)
            .unwrap()
            .instantiate(program, &empty)
            .is_err()
    );
}

#[test]
fn representation_reclassification_moves_one_owner_and_rejects_partial_publication() {
    let queries = queries("func work(take value: error?, take occupied: error?): void {}").unwrap();
    let query = &queries[0];
    let program = query.program();
    let inputs = query.body().inputs().explicit();
    let source = Holder::Local(inputs[0].local).place();
    let occupied = Holder::Local(inputs[1].local);
    let owned = payload(
        program,
        query.body().locals().get(inputs[0].local).unwrap().data,
    )
    .unwrap();
    let mut state = State::entry(query).unwrap();
    let before = state.clone();
    let mut steps = Vec::new();
    assert!(
        state
            .reclassify(
                program,
                &source,
                Holder::Return,
                owned,
                &mut steps,
                |_| Err(INVALID)
            )
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());
    let record = |from| {
        Ok(Step::Transport {
            source: source.clone(),
            destination: Holder::Return,
            from,
            payload: owned,
        })
    };
    assert!(
        state
            .reclassify(program, &source, occupied, owned, &mut steps, |from| Ok(
                Step::Transport {
                    source: source.clone(),
                    destination: occupied,
                    from,
                    payload: owned
                }
            ))
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());
    for invalid in [
        Step::Construct {
            destination: Holder::Return,
            payload: owned,
        },
        Step::Transport {
            source: occupied.place(),
            destination: Holder::Return,
            from: owned,
            payload: owned,
        },
    ] {
        assert!(
            state
                .reclassify(program, &source, Holder::Return, owned, &mut steps, |_| Ok(
                    invalid
                ))
                .is_err()
        );
        assert_eq!(state, before);
        assert!(steps.is_empty());
    }
    state
        .reclassify(program, &source, Holder::Return, owned, &mut steps, record)
        .unwrap();
    assert!(!state.contains(source.holder()));
    assert!(state.contains(occupied));
    assert!(state.contains(Holder::Return));
    assert!(
        matches!(steps.as_slice(), [Step::Transport { from, payload, .. }] if *from == owned && *payload == owned)
    );
    let after = state.clone();
    let prior = steps.clone();
    assert!(
        state
            .reclassify(program, &source, Holder::Return, owned, &mut steps, record)
            .is_err()
    );
    assert_eq!(state, after);
    assert_eq!(steps, prior);
}

#[test]
fn ownership_mutants_cannot_duplicate_destroy_twice_or_escape_without_a_return_holder() {
    let queries = queries("func run(take value: error): error { value^ }\n").unwrap();
    let query = &queries[0];
    let program = query.program();
    let local = query.body().locals().iter().next().unwrap().0;
    let source = Holder::Local(local).place();
    let mut state = State::entry(query).unwrap();
    let owned = payload(program, query.body().locals().get(local).unwrap().data).unwrap();
    assert!(!state.returned(program, owned.data()));
    assert!(
        state
            .construct(Holder::Local(local), owned, &mut Vec::new())
            .is_err()
    );
    let mut state = State::entry(query).unwrap();
    state.dispose(program, &source, &mut Vec::new()).unwrap();
    assert!(state.dispose(program, &source, &mut Vec::new()).is_err());
    assert!(!state.returned(program, owned.data()));
    let mut state = State::entry(query).unwrap();
    state
        .move_to(
            program,
            &source,
            &Holder::Return.place(),
            false,
            &mut Vec::new(),
        )
        .unwrap();
    assert!(state.returned(program, owned.data()));
    assert!(!state.returned(program, program.data_type(BuiltinType::I32).unwrap()));
    assert!(
        state
            .move_to(
                program,
                &source,
                &Holder::Return.place(),
                false,
                &mut Vec::new()
            )
            .is_err()
    );
}

mod arrays;
mod nominal;
mod progression;
mod sites;
mod snapshots;

#[test]
fn scoped_replacement_rejects_mismatched_data_without_consuming_its_source() {
    let queries = queries("func work(take value: i32, edit target: i32): void {}").unwrap();
    let query = &queries[0];
    let program = query.program();
    let input = query.body().inputs().explicit();
    let source = Holder::Local(input[0].local).place();
    let alias = input[1].local;
    let mut state = State::entry(query).unwrap();
    let before = state.clone();
    let mut steps = Vec::new();
    assert!(
        state
            .replace_referent(
                program,
                &source,
                &Holder::Local(alias).place(),
                program.data_type(BuiltinType::Bool).unwrap(),
                &mut steps
            )
            .is_err()
    );
    assert_eq!(state, before);
    assert!(steps.is_empty());
    state
        .replace_referent(
            program,
            &source,
            &Holder::Local(alias).place(),
            program.data_type(BuiltinType::I32).unwrap(),
            &mut steps,
        )
        .unwrap();
    assert!(!state.contains(source.holder()));
    assert!(!state.contains(Holder::Local(alias)));
    assert!(
        matches!(steps.as_slice(), [Step::ReplaceReferent { destination, .. }] if destination.holder() == Holder::Local(alias) && destination.fields().is_root())
    );
}

fn fallible_program(data: nocter_declarations::value_access::DataReference) -> (Program, TypeId) {
    use nocter_declarations::value_access::{DataReference, FieldDefinition, Reservations};
    let symbols = nocter_model::SymbolTable::from_spellings(["Holder", "value"]);
    let mut reservations = Reservations::new(symbols.clone());
    let holder = reservations.reserve_record(symbols.get("Holder").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            holder,
            false,
            [FieldDefinition {
                name: symbols.get("value").unwrap(),
                data: DataReference::Fallible(std::sync::Arc::new(data)),
            }],
        )
        .unwrap();
    let program = builder.finish().unwrap();
    let outcome = program
        .fields()
        .get(program.record(holder).unwrap().fields()[0])
        .unwrap()
        .data();
    (program, outcome)
}

#[test]
fn fallible_error_payloads_keep_the_selected_branch_and_partial_cleanup() {
    let (program, outcome) = fallible_program(BuiltinType::Error.into());
    let error = program.data_type(BuiltinType::Error).unwrap();
    let local = nocter_model::ArenaBuilder::<nocter_model::AccessLocalId, ()>::new().insert(());
    for primary in [true, false] {
        let mut state = State::default();
        state
            .construct(
                Holder::Local(local),
                payload(&program, error).unwrap(),
                &mut Vec::new(),
            )
            .unwrap();
        let before = state.clone();
        assert!(
            state
                .assemble(
                    &program,
                    Holder::Return,
                    payload(&program, outcome).unwrap(),
                    &[],
                    &mut Vec::new()
                )
                .is_err()
        );
        assert_eq!(state, before);
        let mut steps = Vec::new();
        state
            .assemble(
                &program,
                Holder::Return,
                payload(&program, outcome).unwrap(),
                &[(
                    nocter_model::ValueField::Variant(primary),
                    Holder::Local(local).place(),
                )],
                &mut steps,
            )
            .unwrap();
        assert!(state.returned(&program, outcome));
        assert!(!state.contains(Holder::Local(local)));
        assert!(matches!(steps.as_slice(), [Step::Assemble { fields, .. }]
            if fields[0].0 == nocter_model::ValueField::Variant(primary)));
        state
            .move_to(
                &program,
                &Holder::Return
                    .place()
                    .project(nocter_model::ValueField::Variant(primary)),
                &Holder::Local(local).place(),
                false,
                &mut Vec::new(),
            )
            .unwrap();
        let mut cleanup = Vec::new();
        state
            .dispose(&program, &Holder::Return.place(), &mut cleanup)
            .unwrap();
        assert!(matches!(
            cleanup.as_slice(),
            [Step::ReleaseContainer { .. }]
        ));
        assert!(state.contains(Holder::Local(local)));
    }
}

#[test]
fn a_partial_success_cleans_only_its_remaining_active_components() {
    use nocter_declarations::value_access::{DataReference, TupleData};
    use nocter_model::ValueField;
    let (program, outcome) = fallible_program(DataReference::Tuple(TupleData::new(
        BuiltinType::Error.into(),
        BuiltinType::Error.into(),
        [],
    )));
    let success = program
        .component(outcome, ValueField::Variant(true))
        .unwrap()
        .data;
    let mut locals = nocter_model::ArenaBuilder::<nocter_model::AccessLocalId, ()>::new();
    let source = Holder::Local(locals.insert(()));
    let moved = Holder::Local(locals.insert(()));
    let mut state = State::default();
    state
        .construct(source, payload(&program, success).unwrap(), &mut Vec::new())
        .unwrap();
    state
        .assemble(
            &program,
            Holder::Return,
            payload(&program, outcome).unwrap(),
            &[(ValueField::Variant(true), source.place())],
            &mut Vec::new(),
        )
        .unwrap();
    state
        .move_to(
            &program,
            &Holder::Return
                .place()
                .project(ValueField::Variant(true))
                .project(ValueField::Tuple(0)),
            &moved.place(),
            false,
            &mut Vec::new(),
        )
        .unwrap();
    let mut cleanup = Vec::new();
    state
        .dispose(&program, &Holder::Return.place(), &mut cleanup)
        .unwrap();
    assert!(matches!(cleanup.as_slice(), [Step::Dispose { source, .. },
        Step::ReleaseContainer { .. }, Step::ReleaseContainer { .. }]
        if source.fields().fields() == [nocter_model::ValueProjection::Variant(true), nocter_model::ValueProjection::Tuple(1)]));
    assert!(state.contains(moved));
}
