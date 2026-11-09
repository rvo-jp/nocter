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

mod nominal;
