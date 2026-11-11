use super::*;
use crate::value_access::tests::queries;

#[test]
fn ownership_mutants_cannot_duplicate_destroy_twice_or_escape_without_a_return_holder() {
    let queries = queries("func run(take value: error): error { value^ }\n").unwrap();
    let query = &queries[0];
    let program = query.program();
    let local = query.body().locals().iter().next().unwrap().0;
    let source = Holder::Local(local).place();
    let mut state = State::entry(query).unwrap();
    let owned = payload(program, query.body().locals().get(local).unwrap().data).unwrap();
    assert!(!state.returned(owned.data()));
    assert!(
        state
            .construct(Holder::Local(local), owned, &mut Vec::new())
            .is_err()
    );
    let mut state = State::entry(query).unwrap();
    state.dispose(program, &source, &mut Vec::new()).unwrap();
    assert!(state.dispose(program, &source, &mut Vec::new()).is_err());
    assert!(!state.returned(owned.data()));
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
    assert!(state.returned(owned.data()));
    assert!(!state.returned(program.data_type(BuiltinType::I32).unwrap()));
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
