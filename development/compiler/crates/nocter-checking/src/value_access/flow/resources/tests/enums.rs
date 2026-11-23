//! Whole-case receipts preserve ownership even when validation fails partway through assembly.
use super::*;
use crate::value_access::accepted::ModuleSourceUnit;
use nocter_compile_input::ModuleSourceKind::Root;
use nocter_model::ArenaBuilder;

#[test]
fn enum_assembly_validates_the_ordered_case_before_retiring_its_fields() {
    let fixture = nocter_test_support::ValueModuleFixture::new(
        &[(
            "/app/index.nct",
            Root,
            "enum State { empty\npair(a: error, b: error)\nfailed(a: error) }",
        )],
        &[],
    );
    let input = fixture.input();
    let source = ModuleSourceUnit::prepare(&input).unwrap();
    let program = source.unit().program();
    let data = program.type_named("State").unwrap();
    let schema = program
        .enumeration(program.nominal_identity(data).unwrap())
        .unwrap();
    let [a, b] = program
        .variant_components(data, schema.variants()[1])
        .unwrap()
        .map(|component| component.field)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let other = program
        .variant_components(data, schema.variants()[2])
        .unwrap()
        .next()
        .unwrap()
        .field;
    let mut ids = ArenaBuilder::<AccessLocalId, ()>::new();
    let [destination, left, right] = std::array::from_fn(|_| Holder::Local(ids.insert(())));
    let mut state = State::default();
    let error = payload(program, program.data_type(BuiltinType::Error).unwrap()).unwrap();
    for local in [left, right] {
        state.construct(local, error, &mut Vec::new()).unwrap();
    }
    let fields = [(a, left.place()), (b, right.place())];
    let owner = payload(program, data).unwrap();
    assert_eq!(owner.disposal(), Disposal::Aggregate);
    let assemble = |state: &mut State,
                    tag,
                    fields: &[(nocter_model::ValueField, Place)],
                    steps: &mut Vec<Step>| {
        state.assemble_with(
            program,
            destination,
            owner,
            Assembly {
                tag: Some(tag),
                fields,
            },
            steps,
            |actual, expected| actual == expected,
        )
    };
    let initial = state.clone();
    let mut steps = Vec::new();
    for (tag, invalid) in [
        (0, fields.to_vec()),
        (1, vec![(other, left.place()), (b, right.place())]),
        (1, vec![fields[1].clone(), fields[0].clone()]),
        (1, vec![(a, left.place()), (b, left.place())]),
        (1, vec![fields[0].clone()]),
        (3, vec![]),
    ] {
        assert!(assemble(&mut state, tag, &invalid, &mut steps).is_err());
        assert_eq!(state, initial);
        assert!(steps.is_empty());
    }
    assemble(&mut state, 1, &fields, &mut steps).unwrap();
    assert!(!state.contains(left) && !state.contains(right));
    assert!(state.initialized(program, &destination.place()).unwrap());
    assert!(
        state
            .take(program, &destination.place().project(a))
            .is_err()
    );
    assert!(
        matches!(&steps[..], [Step::Assemble { enum_case: Some(1), fields: selected, .. }] if selected.as_ref() == fields)
    );
    state
        .dispose(program, &destination.place(), &mut steps)
        .unwrap();
    assert!(!state.contains(destination));
    assert!(
        matches!(&steps[1..], [Step::Dispose { source, payload: disposed }] if *source == destination.place() && *disposed == owner)
    );

    assemble(&mut state, 0, &[], &mut steps).unwrap();
    assert!(
        matches!(steps.last(), Some(Step::Assemble { enum_case: Some(0), fields, .. }) if fields.is_empty())
    );
}
