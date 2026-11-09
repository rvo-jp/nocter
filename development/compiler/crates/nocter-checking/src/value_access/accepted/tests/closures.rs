//! Source-level owned environments share normal access, initialization and cleanup proofs.
use super::support::steps;
use super::*;
use nocter_model::ValueInvocation;

#[test]
fn owning_a_capture_does_not_make_an_observing_closure_once() {
    let unit = accepted(
        "func observe(input: error): void {}\nfunc run(take input: error): void {\nlet reader = (input;): void { observe(input) }\nreader()\nreader()\n}",
    );
    let (&id, body) = unit.closures().iter().next().unwrap();
    let schema = unit.program().closure(id).unwrap();
    assert_eq!(schema.contract().invocation, ValueInvocation::Read);
    let data = unit
        .program()
        .function_data(nocter_declarations::value_access::FunctionWitness {
            origin: nocter_model::ValueFunctionOrigin::Closure(id),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    assert_eq!(unit.program().data_copyable(data), Some(false));
    assert!(!steps(body).iter().any(
        |s| matches!(s, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError)
    ));
    let trace = steps(named(&unit, "run"));
    assert_eq!(
        trace
            .iter()
            .filter(|s| matches!(s, Step::Capture { .. }))
            .count(),
        1
    );
    assert_eq!(trace.iter().filter(|s| matches!(s, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedCapture)).count(), 1);
    assert!(
        unit.closures()
            .values()
            .all(|b| std::ptr::eq(b.typed().program(), unit.program()))
    );
}

#[test]
fn update_and_extraction_select_mut_and_once_without_call_site_markers() {
    let update = accepted(
        "func run(): i32 {\nlet count = 1\nvar next = (count;): i32 { count = count + 1\ncount }\nnext() + next()\n}",
    );
    let (&id, body) = update.closures().iter().next().unwrap();
    assert_eq!(
        update.program().closure(id).unwrap().contract().invocation,
        ValueInvocation::Edit
    );
    assert!(
        steps(body)
            .iter()
            .any(|s| matches!(s, Step::ReplaceInput { .. }))
    );
    let consume = accepted(
        "func run(take input: error): error {\nlet finish = (input;): error { input }\nfinish()\n}",
    );
    let (&id, body) = consume.closures().iter().next().unwrap();
    assert_eq!(
        consume.program().closure(id).unwrap().contract().invocation,
        ValueInvocation::Take
    );
    assert!(steps(body).iter().any(
        |s| matches!(s, Step::Move { destination, .. } if destination.holder() == Holder::Return)
    ));
    assert!(!steps(body).iter().any(
        |s| matches!(s, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError)
    ));
}

#[test]
fn copied_environments_snapshot_and_distinct_bodies_keep_distinct_code() {
    let unit = accepted(
        "func first(): i32 {\nvar value = 1\nlet action = (value; n: i32): i32 { value + n }\nvalue = 20\nlet copy = action\naction(copy(2))\n}\nfunc second(): i32 {\nlet value = 5\nlet action = (value;): i32 { value }\naction()\n}",
    );
    assert_eq!(unit.closures().len(), 2);
    let ids = unit.closures().keys().copied().collect::<Vec<_>>();
    assert_ne!(ids[0], ids[1]);
    for id in ids {
        let data = unit
            .program()
            .function_data(nocter_declarations::value_access::FunctionWitness {
                origin: nocter_model::ValueFunctionOrigin::Closure(id),
                invocation: ValueInvocation::Read,
            })
            .unwrap();
        assert_eq!(unit.program().data_copyable(data), Some(true));
    }
}

#[test]
fn captured_record_partial_return_stages_the_field_and_cleans_only_the_remainder() {
    let unit = accepted(
        "struct Pair {\nfirst: error\nsecond: error\n}\nfunc run(take pair: Pair): error {\nlet finish = (pair;): error { pair.first }\nfinish()\n}",
    );
    let body = unit.closures().values().next().unwrap();
    let trace = steps(body);
    assert_eq!(trace.iter().filter(|s| matches!(s, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError)).count(), 1);
    assert_eq!(
        trace
            .iter()
            .filter(|s| matches!(s, Step::ReleaseContainer { .. }))
            .count(),
        1
    );
}

#[test]
fn closure_body_and_capture_changes_invalidate_the_existing_query() {
    let original =
        "func run(): i32 {\nlet value = 1\nlet action = (value;): i32 { value }\naction()\n}";
    let (sources, tree) = input(original);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    assert!(unit.bind(&context).is_ok());
    let (changed, new_tree) = input(&original.replace("{ value }", "{ value + 1 }"));
    let current = headers.bind(bound(&changed, &new_tree)).unwrap();
    assert!(matches!(unit.bind(&current), Err(BodyFailure::ChangedBody)));
}

#[test]
fn returned_environments_keep_their_code_schema_and_input_supply_stays_static() {
    let unit = accepted(
        "func observe(input: error): void {}\nfunc make(take input: error): func(): void { (input;): void { observe(input) } }\nfunc apply(action: func(): void): void { action() }\nfunc run(take input: error): void {\nlet action = make(input)\napply(action)\naction()\n}",
    );
    let make = named(&unit, "make").typed().callable();
    assert!(matches!(
        unit.result_witness(make).unwrap().origin,
        nocter_model::ValueFunctionOrigin::Closure(_)
    ));
    assert!(
        steps(named(&unit, "make"))
            .iter()
            .any(|s| matches!(s, Step::Move { destination, payload, .. }
        if destination.holder() == Holder::Return && payload.disposal() == Disposal::OwnedCapture))
    );
}

#[test]
fn unreachable_environment_moves_do_not_force_consuming_invocation() {
    let unit = accepted(
        "func observe(input: error): void {}\nfunc run(take input: error): void {\nlet action = (input;): void {\nif false { let moved = input }\nobserve(input)\n}\naction()\naction()\n}",
    );
    let (&id, _) = unit.closures().iter().next().unwrap();
    assert_eq!(
        unit.program().closure(id).unwrap().contract().invocation,
        ValueInvocation::Read
    );
}

#[test]
fn interrupted_callback_input_cleans_the_prepared_environment_once() {
    let unit = accepted(
        "func apply(action: func(): void, number: i32): void {}\nfunc run(take input: error): void {\napply((input;): void {}, if true { return } else { return })\n}",
    );
    let trace = steps(named(&unit, "run"));
    assert_eq!(
        trace
            .iter()
            .filter(|s| matches!(s, Step::Capture { .. }))
            .count(),
        1
    );
    assert_eq!(trace.iter().filter(|s| matches!(s, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedCapture)).count(), 1);
    assert!(
        !trace
            .iter()
            .any(|s| matches!(s, Step::Invoke { .. } | Step::Receive { .. }))
    );
}

#[test]
fn invalid_capture_supply_uses_and_unused_bodies_reject_the_whole_unit() {
    for source in [
        "func run(input: error): void { let action = (input;): void {} }",
        "func run(take input: error): error {\nlet action = (input;): error { input }\nlet first = action()\naction()\n}",
        "func run(): i32 {\nlet value = 1\nlet action = (value;): i32 { value += 1\nvalue }\naction()\n}",
        "func run(): void {\nlet value = 1\nlet action = (value; x: error): error { x }\n}",
        "func run(): i32 {\nlet value = 1\nlet action = (value;): i32 { break }\naction()\n}",
    ] {
        let (sources, tree) = input(source);
        let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        assert!(
            matches!(
                Unit::prepare(&headers.bind(bound(&sources, &tree)).unwrap()),
                Err(Failure::Body { .. } | Failure::Flow { .. })
            ),
            "{source}"
        );
    }
}
