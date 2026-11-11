//! Source-level owned environments share normal access, initialization and cleanup proofs.
use super::support::steps;
use super::*;
use nocter_model::ValueInvocation;

#[test]
fn a_captured_callback_cannot_hide_writes_to_an_explicit_prefix_counter() {
    for setup in [
        "let value = ()\nlet reset = (value; edit index: usize): void { index = 0 }",
        "let first = 0\nlet second = 0\nlet reset = (first, second; edit index: usize): void { index = 0 }",
        "let value = ()\nlet inner = (value; edit index: usize): void { index = 0 }\nlet reset = (inner; edit index: usize): void { inner(index) }",
    ] {
        let source = format!(
            "func work(): void {{ {setup}\nlet items = [1, 2]\nvar index: usize = 0\nwhile index < 2 {{ let moved = items[index]^\nindex += 1\nreset(index) }} }}"
        );
        let (sources, tree) = input(&source);
        assert!(
            matches!(
                SourceUnit::prepare(bound(&sources, &tree)),
                Err(SourceFailure::Checking(Failure::Flow { .. }))
            ),
            "{setup}"
        );
    }
}

#[test]
fn an_exact_named_callable_can_be_captured_as_an_ordinary_value() {
    let unit = accepted(
        "func increment(value: i32): i32 { value + 1 }\nfunc run(): i32 { let callback = increment\nlet outer = (callback;): i32 { callback(41) }\nouter() }",
    );
    assert_eq!(unit.closures().len(), 1);
}

#[test]
fn callable_value_captures_preserve_copy_transfer_and_selected_invocation_views() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/callable-value-captures.nct"
    ));
    let unit = accepted(source);
    for (name, expected) in [
        ("mutable_view", ValueInvocation::Edit),
        ("once_copy", ValueInvocation::Read),
        ("consuming", ValueInvocation::Take),
    ] {
        let root = named(&unit, name).typed().source_owner();
        let outer = unit
            .closures()
            .keys()
            .filter(|id| id.owner.source_owner() == root)
            .max_by_key(|id| id.slot)
            .unwrap();
        assert_eq!(
            unit.program()
                .closure(*outer)
                .unwrap()
                .contract()
                .invocation,
            expected,
            "{name}"
        );
    }
    let prefix = "func increment(value: i32): i32 { value + 1 }\n";
    let source = format!(
        "{prefix}func work(): i32 {{ let callback = increment\nlet outer = (callback^;): i32 {{ callback(41) }}\ncallback(41) }}"
    );
    let (sources, tree) = input(&source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn destroying_a_captured_callable_releases_its_schema_without_invocation_or_duplicate_owners() {
    let unit = accepted(
        "func work(take reason: error): void { let inner = (reason^;): void {}\nlet outer = (inner^;): void {}\ndrop outer }",
    );
    let root = steps(named(&unit, "work"));
    assert_eq!(
        root.iter()
            .filter(|step| matches!(step, Step::Capture { .. }))
            .count(),
        2
    );
    assert_eq!(root.iter().filter(|step| matches!(step, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedCapture)).count(), 1);
    assert!(!root.iter().any(|step| matches!(step, Step::Invoke { .. })));
    assert!(!root.iter().any(|step| matches!(step, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError)));
}

#[test]
fn named_callback_writes_map_explicit_inputs_after_the_frozen_environment() {
    let unit = accepted(
        "func update(edit value: i32): void { value += 1 }\nfunc work(): i32 { var value = 40\nlet callback = update\ncallback(value)\ncallback(value)\nvalue }",
    );
    assert!(named(&unit, "work").typed().results_closed());
}

#[test]
fn a_named_mut_callback_cannot_hide_reset_of_a_prefix_counter() {
    let source = "func reset(edit value: usize): void { value = 0 }\nfunc work(): void { var callback: mut(edit usize): void = reset\nlet items = [1, 2]\nvar index: usize = 0\nwhile index < 2 { let moved = items[index]^\nindex += 1\ncallback(index) } }";
    let (sources, tree) = input(source);
    let actual = SourceUnit::prepare(bound(&sources, &tree));
    assert!(
        matches!(&actual, Err(SourceFailure::Checking(Failure::Flow { .. }))),
        "accepted={}",
        actual.is_ok()
    );
}

#[test]
fn unknown_callable_environments_cannot_inherit_one_concrete_payloads_properties() {
    for source in [
        "func work(take callback: func(i32): i32): void { let outer = (callback^;): i32 { callback(41) } }",
        "func increment(value: i32): i32 { value + 1 }\nfunc make(): func(i32): i32 { increment }\nfunc work(): void { let callback = make()\nlet outer = (callback^;): i32 { callback(41) } }",
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == crate::value_access::Rule::TypeMismatch && diagnostic.source().message().contains("component payloads")),
            "{source}"
        );
    }
}

#[test]
fn nested_owned_captures_have_distinct_lexical_code_and_one_canonical_program() {
    let unit = accepted(
        "func run(): i32 { let value = 40\nlet outer = (value;): i32 { let inner = (value;): i32 { value + 2 }\ninner() }\nouter() }",
    );
    assert_eq!(unit.closures().len(), 2);
    for query in unit.closures().values() {
        assert!(std::ptr::eq(query.typed().program(), unit.program()));
        assert!(query.typed().results_closed());
    }
}

#[test]
fn deep_and_sibling_capture_bodies_keep_schema_and_source_evidence() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/nested-closed-captures.nct"
    ));
    let (sources, tree) = input(source);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    assert_eq!(unit.closures().len(), 10);
    for (&id, query) in unit.closures() {
        assert_eq!(query.typed().code(), nocter_model::ValueCode::Closure(id));
        assert!(std::ptr::eq(query.typed().program(), unit.program()));
        assert!(query.typed().bind(&context).is_ok());
    }
    let (changed, changed_tree) =
        input(&source.replace("callback(value + 1)", "callback(value + 2)"));
    let current = headers.bind(bound(&changed, &changed_tree)).unwrap();
    assert!(
        unit.closures()
            .values()
            .all(|query| query.typed().source_owner()
                != named(&unit, "snapshots").typed().source_owner()
                || matches!(query.typed().bind(&current), Err(BodyFailure::ChangedBody)))
    );
}

#[test]
fn nested_capture_formation_and_body_consumption_keep_separate_once_demands() {
    let unit = accepted(
        "func work(take reason: error): error { let outer = (reason^;): error { let inner = (reason^;): error { reason^ }\ninner^() }\nouter^() }",
    );
    assert_eq!(unit.closures().len(), 2);
    for (&id, query) in unit.closures() {
        assert_eq!(
            unit.program().closure(id).unwrap().contract().invocation,
            ValueInvocation::Take
        );
        assert!(!steps(query).iter().any(|step| matches!(step, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError)));
    }
    let source = "struct Item { value: i32 }\nfunc work(): void { let value = Item { value: 42 }\nlet outer = (value^;): void { let inner = (value^;): void {}\ninner() }\nouter^()\nouter^() }";
    let (sources, tree) = input(source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn a_factory_call_inside_a_closure_closes_its_own_result_application() {
    let unit = accepted(
        "func increment(value: i32): i32 { value + 1 }\nfunc factory(): func(i32): i32 { increment }\nfunc run(): i32 { let value = 40\nlet action = (value;): i32 { let callback = factory()\ncallback(value + 1) }\naction() }",
    );
    assert!(
        unit.closures()
            .values()
            .all(|query| query.typed().results_closed())
    );
}

#[test]
fn closure_factory_applications_keep_separate_witnesses_and_reuse_dependencies() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/closure-result-applications.nct"
    ));
    let (sources, tree) = input(source);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    for query in unit.closures().values() {
        assert!(query.typed().results_closed());
        assert!(query.typed().bind(&context).is_ok());
        for point in query.graph().reachable().unwrap() {
            for action in &query.graph().points().get(point).unwrap().actions {
                if let Operation::CommitCall(call) = action.operation {
                    let expression = query.typed().body().expressions().get(call).unwrap();
                    assert!(!expression.ty.access().is_some_and(|access| {
                        crate::value_access::results::is_result(unit.program(), access.data())
                    }));
                }
            }
        }
    }
    let (changed, changed_tree) = input(&source.replace(
        "func forward(take action: func(i32): i32): func(i32): i32 { action^ }",
        "func forward(take action: func(i32): i32): func(i32): i32 { increment }",
    ));
    let current = headers.bind(bound(&changed, &changed_tree)).unwrap();
    assert!(
        unit.closures()
            .values()
            .any(|query| matches!(query.typed().bind(&current), Err(BodyFailure::ChangedBody)))
    );
}

#[test]
fn closure_application_closure_still_rejects_incompatible_witnesses() {
    let source = "func increment(value: i32): i32 { value + 1 }\nfunc decrement(value: i32): i32 { value - 1 }\nfunc forward(take action: func(i32): i32): func(i32): i32 { action^ }\nfunc work(flag: bool): i32 { let action = (flag;): i32 { let next = if flag { forward(increment) } else { forward(decrement) }\nnext(42) }\naction() }";
    let (sources, tree) = input(source);
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == crate::value_access::Rule::TypeMismatch && diagnostic.source().message().contains("different callable witnesses"))
    );
}

#[test]
fn owning_a_capture_does_not_make_an_observing_closure_once() {
    let unit = accepted(
        "func observe(input: error): void {}\nfunc run(take input: error): void {\nlet reader = (input^;): void { observe(input) }\nreader()\nreader()\n}",
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
            .any(|s| matches!(s, Step::ReplaceReferent { .. }))
    );
    let consume = accepted(
        "func run(take input: error): error {\nlet finish = (input^;): error { input^ }\nfinish^()\n}",
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
        "struct Pair {\nfirst: error\nsecond: error\n}\nfunc run(take pair: Pair): error {\nlet finish = (pair^;): error { pair.first^ }\nfinish^()\n}",
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
        "func observe(input: error): void {}\nfunc make(take input: error): func(): void { (input^;): void { observe(input) } }\nfunc apply(action: func(): void): void { action() }\nfunc run(take input: error): void {\nlet action = make(input^)\napply(action)\naction()\n}",
    );
    let make = named(&unit, "make").typed().source_owner();
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
        "func observe(input: error): void {}\nfunc run(take input: error): void {\nlet action = (input^;): void {\nif false { let moved = input^ }\nobserve(input)\n}\naction()\naction()\n}",
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
        "func apply(action: func(): void, number: i32): void {}\nfunc run(take input: error): void {\napply((input^;): void {}, if true { return } else { return })\n}",
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
        "func run(take input: error): error {\nlet action = (input^;): error { input^ }\nlet first = action^()\naction^()\n}",
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

#[test]
fn empty_and_multiple_owned_captures_share_one_environment_input() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/owned-environments.nct"
    ));
    let unit = accepted(source);
    for query in unit.closures().values() {
        let inputs = query.typed().body().inputs();
        assert!(inputs.environment.is_some());
        assert!(query.typed().results_closed());
        assert!(std::ptr::eq(query.typed().program(), unit.program()));
    }
}

#[test]
fn multiple_capture_names_preserve_source_and_component_retirement() {
    for source in [
        "func work(): i32 { let left = 20\nlet right = 22\nlet action = (left, right^;): i32 { left + right }\nright }",
        "struct Item { value: i32 }\nfunc work(): i32 { let first = Item { value: 20 }\nlet second = Item { value: 22 }\nlet action = (first^, second^;): i32 { drop first\nfirst.value + second.value }\naction^() }",
        "struct Item { value: i32 }\nfunc work(): void { let first = Item { value: 20 }\nlet second = 22\nlet action = (second, first;): void {} }",
    ] {
        let (sources, tree) = input(source);
        assert!(
            matches!(
                SourceUnit::prepare(bound(&sources, &tree)),
                Err(SourceFailure::Checking(
                    Failure::Flow { .. } | Failure::Body { .. }
                ))
            ),
            "{source}"
        );
    }
}

#[test]
fn consuming_one_capture_cleans_only_the_other_live_components() {
    let unit = accepted(
        "func work(take first: error, take second: error, take third: error): error { let action = (first^, second^, third^;): error { drop second\nfirst^ }\naction^() }",
    );
    let body = unit.closures().values().next().unwrap();
    let (&id, _) = unit.closures().iter().next().unwrap();
    assert_eq!(
        unit.program().closure(id).unwrap().contract().invocation,
        ValueInvocation::Take
    );
    let trace = steps(body);
    assert_eq!(trace.iter().filter(|s| matches!(s, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError)).count(), 2);
}

#[test]
fn repeated_capture_names_and_parameter_collisions_reject_locally() {
    for head in [
        "(left, left;)",
        "(left, left^;)",
        "(left, right; left: i32)",
    ] {
        let source = format!(
            "func work(): void {{ let left = 20\nlet right = 22\nlet action = {head}: void {{}} }}"
        );
        let (sources, tree) = input(&source);
        assert!(
            matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Body { failure: BodyFailure::Rejected(diagnostic), .. })) if diagnostic.rule() == crate::value_access::Rule::ShadowedName),
            "{source}"
        );
    }
}
