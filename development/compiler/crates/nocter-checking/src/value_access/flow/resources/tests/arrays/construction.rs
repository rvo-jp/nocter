use super::*;

#[test]
fn listed_arrays_transfer_distinct_component_owners_without_requiring_copy() {
    for length in 0..=2 {
        let fixture = Fixture::new(length, false);
        let program = &fixture.program;
        let mut state = State::default();
        let error = payload(program, program.data_type(BuiltinType::Error).unwrap()).unwrap();
        let fields = (0..length)
            .map(|index| {
                let source = fixture.locals[usize::try_from(index).unwrap() + 1];
                state.construct(source, error, &mut Vec::new()).unwrap();
                (ValueField::Array(index), source.place())
            })
            .collect::<Vec<_>>();
        let mut steps = Vec::new();
        state
            .assemble(
                program,
                fixture.locals[0],
                payload(program, fixture.data).unwrap(),
                &fields,
                &mut steps,
            )
            .unwrap();
        assert!(state.initialized_root(fixture.locals[0]));
        assert!(
            fields
                .iter()
                .all(|(_, source)| !state.contains(source.holder()))
        );
        assert!(
            matches!(steps.as_slice(), [Step::Assemble { fields: chosen, payload, .. }] if chosen.as_ref() == fields && payload.data() == fixture.data)
        );
        assert_eq!(program.data_copyable(fixture.data), Some(length == 0));
    }
}

#[test]
fn malformed_array_assembly_cannot_publish_partial_transfers_or_overwrite_a_seed() {
    let fixture = Fixture::new(2, false);
    let program = &fixture.program;
    let mut state = State::default();
    let error = payload(program, program.data_type(BuiltinType::Error).unwrap()).unwrap();
    for &holder in &fixture.locals[1..3] {
        state.construct(holder, error, &mut Vec::new()).unwrap();
    }
    state
        .construct(
            fixture.locals[3],
            payload(program, program.data_type(BuiltinType::I32).unwrap()).unwrap(),
            &mut Vec::new(),
        )
        .unwrap();
    let first = fixture.locals[1].place();
    let second = fixture.locals[2].place();
    let fields = [
        (ValueField::Array(0), first.clone()),
        (ValueField::Array(1), second.clone()),
    ];
    for (destination, invalid) in [
        (fixture.locals[0], vec![fields[0].clone()]),
        (
            fixture.locals[0],
            vec![fields[0].clone(), fields[0].clone()],
        ),
        (
            fixture.locals[0],
            vec![(ValueField::Tuple(0), first.clone()), fields[1].clone()],
        ),
        (
            fixture.locals[0],
            vec![fields[0].clone(), (ValueField::Array(2), second)],
        ),
        (
            fixture.locals[0],
            vec![fields[0].clone(), (ValueField::Array(1), first)],
        ),
        (
            fixture.locals[0],
            vec![
                fields[0].clone(),
                (ValueField::Array(1), fixture.locals[3].place()),
            ],
        ),
        (fixture.locals[1], fields.to_vec()),
    ] {
        let before = state.clone();
        let mut steps = Vec::new();
        assert!(
            state
                .assemble(
                    program,
                    destination,
                    payload(program, fixture.data).unwrap(),
                    &invalid,
                    &mut steps
                )
                .is_err()
        );
        assert_eq!(state, before);
        assert!(steps.is_empty());
    }
    let fixture = Fixture::new(u64::MAX, false);
    assert!(
        State::default()
            .assemble(
                &fixture.program,
                fixture.locals[0],
                payload(&fixture.program, fixture.data).unwrap(),
                &[],
                &mut Vec::new()
            )
            .is_err()
    );
}
