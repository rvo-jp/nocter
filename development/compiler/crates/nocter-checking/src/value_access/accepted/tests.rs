use super::*;
use crate::value_access::{
    flow::{
        Operation,
        resources::{Disposal, Holder, Step},
    },
    tests::{bound, field, input, string},
};
use nocter_declaration_lowering::access_declarations as declarations;
use nocter_json::Value;
use std::{fs, path::PathBuf};

fn accepted(source: &str) -> Unit {
    let (sources, tree) = input(source);
    let declarations = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    Unit::prepare(&declarations.bind(bound(&sources, &tree)).unwrap()).unwrap()
}
fn named<'a>(unit: &'a Unit, name: &str) -> &'a flow::Query {
    unit.bodies()
        .find(|(_, query)| {
            unit.program().symbols().spelling(
                unit.program()
                    .callables()
                    .get(query.typed().callable())
                    .unwrap()
                    .name(),
            ) == Some(name)
        })
        .unwrap()
        .1
}
fn error_disposals(steps: &[Step]) -> Vec<Holder> {
    steps
        .iter()
        .filter_map(|step| match step {
            Step::Dispose { source, payload } if payload.disposal() == Disposal::OwnedError => {
                Some(*source)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn whole_unit_accepts_nineteen_and_rejects_all_nineteen_check_failures() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v2-access");
    let manifest =
        nocter_json::parse(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let Value::Array(cases) = field(&manifest, "cases") else {
        panic!("cases");
    };
    let mut counts = [0; 2];
    for case in cases {
        let expectation = string(field(case, "expectation"));
        if matches!(expectation, "syntax-reject" | "declaration-reject") {
            continue;
        }
        let source = fs::read_to_string(root.join(string(field(case, "file")))).unwrap();
        let (sources, tree) = input(&source);
        let declarations = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let result = Unit::prepare(&declarations.bind(bound(&sources, &tree)).unwrap());
        assert_eq!(
            result.is_ok(),
            expectation == "accept",
            "{}: {result:?}",
            string(field(case, "id"))
        );
        assert!(
            !matches!(result, Err(Failure::Integrity(_))),
            "{}",
            string(field(case, "id"))
        );
        counts[usize::from(expectation == "accept")] += 1;
    }
    assert_eq!(counts, [19, 19]);
}

#[test]
fn prepared_owned_prefix_is_destroyed_in_reverse_order_only_on_cancellation() {
    let unit = accepted(
        "func forward(value: ^error): ^error { ^value }\nfunc take(first: ^error, second: ^error, number: i32): void {}\nfunc run(first: ^error, second: ^error, flag: bool): ^i32 {\n take(forward(first), forward(second), if flag { return 1 } else { 0 })\n return 0\n}\n",
    );
    let run = named(&unit, "run");
    let mut found = false;
    for (point, block) in run.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            let at = flow::Location { point, action };
            if let Operation::CancelCall { call, prepared: 2 } = event.operation {
                for transition in run.access().resources().at(at) {
                    assert_eq!(
                        error_disposals(transition.steps()),
                        [
                            Holder::Prepared { call, index: 1 },
                            Holder::Prepared { call, index: 0 }
                        ]
                    );
                    assert!(!transition.initialized_after(Holder::Prepared { call, index: 0 }));
                    found = true;
                }
            }
        }
    }
    assert!(found);
}

#[test]
fn merely_reserved_owner_is_destroyed_at_scope_exit_not_call_cancellation() {
    let unit = accepted(
        "func take(value: ^error, amount: i32): void {}\nfunc run(value: ^error, flag: bool): ^i32 {\n take(value, if flag { return 1 } else { 0 })\n return 0\n}\n",
    );
    let run = named(&unit, "run");
    let mut cancelled = false;
    let mut cleaned = false;
    for (point, block) in run.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            for transition in run
                .access()
                .resources()
                .at(flow::Location { point, action })
            {
                if matches!(event.operation, Operation::CancelCall { prepared: 1, .. }) {
                    assert!(error_disposals(transition.steps()).is_empty());
                    cancelled = true;
                }
                cleaned |= matches!(event.operation, Operation::ExitScope { .. })
                    && !error_disposals(transition.steps()).is_empty();
            }
        }
    }
    assert!(cancelled && cleaned);
}

#[test]
fn fresh_move_only_input_is_read_without_copy_and_its_temporary_is_released() {
    let unit = accepted(
        "func forward(value: ^error): ^error { ^value }\nfunc read(value: error): error { value }\nfunc run(value: ^error): void {\n let _ = read(forward(value))\n}\n",
    );
    let run = named(&unit, "run");
    let mut found = false;
    for (point, block) in run.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            if matches!(event.operation, Operation::EndEvaluation(_)) {
                for transition in run
                    .access()
                    .resources()
                    .at(flow::Location { point, action })
                {
                    if let [Holder::Temporary { .. }] =
                        error_disposals(transition.steps()).as_slice()
                    {
                        found = true;
                    }
                }
            }
        }
    }
    assert!(found);
}

#[test]
fn access_cannot_keep_a_fresh_owned_argument_alive_past_its_evaluation() {
    for tail in [
        "let saved = read(forward(value))\n let _ = saved",
        "return read(forward(value))",
    ] {
        let result_type = if tail.starts_with("return") {
            "error"
        } else {
            "void"
        };
        let source = format!(
            "func forward(value: ^error): ^error {{ ^value }}\nfunc read(value: error): error {{ value }}\nfunc run(value: ^error): {result_type} {{\n {tail}\n}}\n"
        );
        let (sources, tree) = input(&source);
        let declarations = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let result = Unit::prepare(&declarations.bind(bound(&sources, &tree)).unwrap());
        assert!(
            matches!(result, Err(Failure::Flow { .. })),
            "{source}: {result:?}"
        );
    }
}

#[test]
fn rhs_self_transfer_and_maybe_initialized_replacement_destroy_exactly_present_owners() {
    let unit = accepted(
        "func run(value: ^error, next: ^error, flag: bool): ^error {\n if flag { drop value }\n value = ^next\n value = ^value\n return ^value\n}\n",
    );
    let run = named(&unit, "run");
    let assignments = run
        .graph()
        .points()
        .iter()
        .flat_map(|(point, block)| {
            block
                .actions
                .iter()
                .enumerate()
                .filter_map(move |(action, event)| {
                    matches!(event.operation, Operation::Assign { .. })
                        .then_some(flow::Location { point, action })
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(assignments.len(), 2);
    let mut replacement_disposals = run
        .access()
        .resources()
        .at(assignments[0])
        .map(|transition| error_disposals(transition.steps()).len())
        .collect::<Vec<_>>();
    replacement_disposals.sort_unstable();
    replacement_disposals.dedup();
    assert_eq!(replacement_disposals, [0, 1]);
    assert!(
        run.access()
            .resources()
            .at(assignments[1])
            .all(|transition| error_disposals(transition.steps()).is_empty())
    );
}

#[test]
fn all_callees_are_validated_even_when_unreachable_or_recursive() {
    for source in [
        "func entry(): void {}\nfunc bad(): &i32 { let local = 0\n &local }\n",
        "func a(value: ^error): ^error { b(value) }\nfunc b(value: ^error): ^error { a(value) }\n",
        "func entry(): void {}\nfunc bad(value: ^error): ^error { drop value\n ^value }\n",
    ] {
        let (sources, tree) = input(source);
        let declarations = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let result = Unit::prepare(&declarations.bind(bound(&sources, &tree)).unwrap());
        if source.contains("func a(") {
            assert!(result.is_ok(), "{result:?}");
        } else {
            assert!(matches!(result, Err(Failure::Flow { .. })), "{result:?}");
        }
    }
}

#[test]
fn accepted_source_binding_requires_all_original_bodies_and_the_same_authority() {
    let source = "func forward(value: ^error): ^error { ^value }\nfunc entry(): void {}\n";
    let (sources, tree) = input(source);
    let declarations = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let current = declarations.bind(bound(&sources, &tree)).unwrap();
    let unit = Unit::prepare(&current).unwrap();
    assert_eq!(unit.bodies().len(), current.program().callables().len());
    let (later_sources, later_tree) = input(&format!("\n\n{source}"));
    let later = declarations
        .bind(bound(&later_sources, &later_tree))
        .unwrap();
    let rebound = unit.bind(&later).unwrap();
    assert!(std::ptr::eq(rebound.unit(), &raw const unit));
    for (id, query) in unit.bodies() {
        assert!(std::ptr::eq(rebound.body(id).unwrap().query(), query));
    }
    let (changed_sources, changed_tree) = input(&source.replace("{ ^value }", "{ return ^value }"));
    let changed = declarations
        .bind(bound(&changed_sources, &changed_tree))
        .unwrap();
    assert!(matches!(unit.bind(&changed), Err(BodyFailure::ChangedBody)));
    let independent = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        unit.bind(&independent.bind(bound(&sources, &tree)).unwrap()),
        Err(BodyFailure::ChangedAuthority)
    ));
}

#[test]
fn update_replacement_disposes_the_original_payload_and_keeps_the_owner_initialized() {
    let unit = accepted(
        "func replace(destination: &error, next: ^error): void { destination = ^next }\nfunc run(value: ^error, next: ^error): ^error {\n let edit = &value\n edit = ^next\n return ^value\n}\n",
    );
    for name in ["replace", "run"] {
        let query = named(&unit, name);
        let mut found = false;
        for (point, block) in query.graph().points().iter() {
            for (action, event) in block.actions.iter().enumerate() {
                if !matches!(event.operation, Operation::Assign { .. }) {
                    continue;
                }
                for transition in query
                    .access()
                    .resources()
                    .at(flow::Location { point, action })
                {
                    if name == "replace" {
                        assert!(
                            matches!(transition.steps(), [Step::ReplaceInput { payload, .. }] if payload.disposal() == Disposal::OwnedError && !payload.has_external_dependencies())
                        );
                    } else {
                        assert_eq!(error_disposals(transition.steps()).len(), 1);
                        assert!(matches!(
                            transition.steps().last(),
                            Some(Step::Move {
                                destination: Holder::Local(_),
                                ..
                            })
                        ));
                    }
                    found = true;
                }
            }
        }
        assert!(found);
    }
}

#[test]
fn loop_paths_release_body_owners_and_can_return_ownership_without_accumulation() {
    accepted(
        "func run(value: ^error, flag: bool): ^error {\n while flag {\n let moved = ^value\n value = ^moved\n continue\n }\n return ^value\n}\n",
    );
    accepted(
        "func read(value: error): error { value }\nfunc forward(value: ^error): ^error { ^value }\nfunc run(value: ^error, flag: bool): void {\n loop {\n let _ = read(forward(value))\n if flag { break }\n return\n }\n}\n",
    );
    accepted(
        "func run(flag: bool): void { loop { while (if flag { break } else { false }) {} } }\n",
    );
}

#[test]
fn successful_invocation_transfers_prepared_ownership_without_destroying_it_in_the_caller() {
    let unit = accepted(
        "func forward(value: ^error): ^error { ^value }\nfunc take(value: ^error): void {}\nfunc run(value: ^error): void { take(forward(value)) }\n",
    );
    let run = named(&unit, "run");
    let mut transferred = false;
    let mut received = false;
    for (point, block) in run.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            for transition in run
                .access()
                .resources()
                .at(flow::Location { point, action })
            {
                assert!(error_disposals(transition.steps()).is_empty());
                assert!(
                    !transition.steps().iter().any(|step| matches!(step,
                    Step::Construct { payload, .. } if payload.disposal() == Disposal::OwnedError))
                );
                if matches!(event.operation, Operation::CommitCall(_)) {
                    received |= transition.steps().iter().any(|step| {
                        matches!(step,
                        Step::Receive { payload, .. } if payload.disposal() == Disposal::OwnedError)
                    });
                    transferred |= transition.steps().iter().any(|step| {
                        matches!(
                            step,
                            Step::Invoke {
                                source: Holder::Prepared { .. },
                                ..
                            }
                        )
                    });
                }
            }
        }
    }
    assert!(transferred && received);
}
