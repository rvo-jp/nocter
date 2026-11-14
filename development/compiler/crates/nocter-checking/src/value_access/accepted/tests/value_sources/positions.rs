//! Source-definition boundaries retain old values and exact preparation-time operand reads.
use super::*;
use crate::value_access::{InstantiationFailure, ValueSourceSelection};
use crate::value_access::{Referent, ValueSourceBoundary, ValueSourceRead, ValueSourceSlot, flow};

#[test]
fn referent_roots_preserve_an_alias_prefix_and_follow_the_current_owner_after_a_write() {
    let unit = accepted(
        "struct Pair { first: i32\nsecond: i32 }\nfunc make(value: i32): Pair { Pair { first: value, second: 0 } }\nfunc run(): i32 { var bundle = (make(41), 0)\nedit alias = bundle.0\nalias.first = 42\nalias.first }",
    );
    let owner = code(&unit, "run");
    let graph = unit.value_sources(owner).unwrap();
    let flow = graph.flow().unwrap();
    let body = unit.code_body(owner).unwrap().typed().body();
    let frame = unit.program().bind_code(owner, []).unwrap();
    let mut snapshots = BTreeSet::new();
    for ((at, referent), root) in flow.roots() {
        let Referent::Place(place) = referent else {
            continue;
        };
        let place = body.places().get(place).unwrap();
        let local = body.locals().get(place.local).unwrap();
        if local.kind != nocter_model::ValueBindingKind::EditAlias
            || place.fields.fields().is_empty()
        {
            continue;
        }
        assert_eq!(flow.root(at, referent), Some(root));
        let [carrier] = flow.root_carriers(at, referent).unwrap() else {
            panic!("one lexical enclosing slot");
        };
        let ValueSourceSlot::Local(container) = carrier.slot else {
            panic!("the original tuple local");
        };
        assert_ne!(body.locals().get(container).unwrap().data, local.data);
        let writes = matches!(unit.code_body(owner).unwrap().graph().points().get(at.point).unwrap().actions[at.action].operation,
            flow::Operation::Assign { target, .. } | flow::Operation::CompoundAssign { target, .. } if target == referent);
        assert_eq!(flow.root_after(at, referent).is_some(), writes);
        assert_eq!(flow.root_carriers_after(at, referent).is_some(), writes);
        if let Some(updated) = flow.root_after(at, referent) {
            assert_ne!(updated, root);
            let ValueSource::Select { source, path } = &graph.node(updated).unwrap().value else {
                panic!("updated alias root");
            };
            assert_eq!(path.fields(), &[ValueProjection::Tuple(0)]);
            let [after] = flow.root_carriers_after(at, referent).unwrap() else {
                panic!("one updated enclosing slot");
            };
            assert_eq!(after.slot, carrier.slot);
            assert_eq!(&after.path, path);
            assert_eq!(after.source, *source);
            assert_ne!(after.source, carrier.source);
            assert!(
                matches!(
                    graph.node(*source).unwrap().value,
                    ValueSource::Replace { weak: false, .. }
                ),
                "the updated root must follow the original write"
            );
        }
        let ValueSource::Select { source, path } = &graph.node(root).unwrap().value else {
            panic!("the referent root must select the aliased Pair from its tuple owner");
        };
        assert_eq!(path.fields(), &[ValueProjection::Tuple(0)]);
        assert_eq!(&carrier.path, path);
        assert_eq!(carrier.source, *source);
        let mut work = nocter_constant_evaluation::state::WorkBudget::new(
            std::num::NonZeroU64::new(10_000).unwrap(),
        );
        assert_closed_root_contracts(&unit, &frame, at, referent, local.data, &mut work);
        assert_eq!(
            unit.instantiate_value_data(&frame, root, local.data, &mut work)
                .unwrap(),
            unit.program().instantiate_closed_data(local.data).unwrap()
        );
        // This reference's root is a Pair, not the enclosing physical tuple or selected i32.
        assert_eq!(
            unit.instantiate_value_data(&frame, root, place.data, &mut work),
            Err(InstantiationFailure::Unresolved)
        );
        let mut pending = vec![root];
        let mut seen = BTreeSet::new();
        let mut updated = false;
        while let Some(source) = pending.pop() {
            if !seen.insert(source) {
                continue;
            }
            let value = &graph.node(source).unwrap().value;
            updated |= matches!(value, ValueSource::Replace { .. });
            pending.extend(edges(value));
        }
        snapshots.insert(updated);
    }
    assert_eq!(
        snapshots,
        BTreeSet::from([false, true]),
        "a lexical alias snapshot must not hide the later referent write"
    );
}

#[test]
fn evaluated_roots_keep_the_containing_array_without_claiming_a_numeric_index() {
    let unit = accepted(
        "func run(): i32 { var items = [(0, 1), (0, 1)]\nvar index: usize = 1\nitems[index].0 = 42\nitems[index].0 }",
    );
    let owner = code(&unit, "run");
    let graph = unit.value_sources(owner).unwrap();
    let flow = graph.flow().unwrap();
    let body = unit.code_body(owner).unwrap().typed().body();
    let mut selected = 0;
    for ((at, referent), _) in flow.roots() {
        let Referent::Evaluated(_) = referent else {
            continue;
        };
        let [carrier] = flow.root_carriers(at, referent).unwrap() else {
            panic!("one enclosing array");
        };
        if !carrier.path.fields().contains(&ValueProjection::Array(())) {
            continue;
        }
        let ValueSourceSlot::Local(local) = carrier.slot else {
            panic!("the original array local");
        };
        assert_ne!(
            Some(body.locals().get(local).unwrap().data),
            body.referent_data(referent)
        );
        assert!(graph.node(carrier.source).is_some());
        selected += 1;
    }
    assert!(
        selected > 0,
        "evaluated field roots retain their larger containing source"
    );
}

fn assert_closed_root_contracts(
    unit: &Unit,
    frame: &nocter_declarations::value_access::CodeInstanceKey,
    at: flow::Location,
    referent: Referent,
    data: nocter_model::TypeId,
    work: &mut nocter_constant_evaluation::state::WorkBudget,
) {
    let contract = unit
        .instantiate_referent_root_data(frame, at, referent, work)
        .unwrap();
    assert_eq!(
        contract,
        unit.program().instantiate_closed_data(data).unwrap()
    );
    assert_eq!(
        unit.instantiate_referent_root_data(
            frame,
            flow::Location {
                action: usize::MAX,
                ..at
            },
            referent,
            work
        ),
        Err(InstantiationFailure::Unresolved),
        "closed metadata still requires an original root occurrence"
    );
    let after = unit
        .value_sources(frame.code())
        .unwrap()
        .flow()
        .unwrap()
        .root_after(at, referent)
        .map(|_| {
            unit.instantiate_referent_root_data_after(frame, at, referent, work)
                .unwrap()
        });
    if let Some(after) = &after {
        assert_eq!(*after, contract);
    }
    let frozen = unit.instantiate_value_data_map(frame, work).unwrap();
    assert_eq!(frozen.root(at, referent), Some(&contract));
    assert_eq!(frozen.root_after(at, referent), after.as_ref());
}

#[test]
fn argument_preparation_commit_and_tag_observation_have_original_root_receipts() {
    let unit = accepted(
        "func combine(value: i32, edit target: i32): i32 { target = value\ntarget }\nfunc run(): i32 { var value: i32? = 41\nread selected = value otherwise { return 0 }\nvar next = 0\ncombine(selected, next) }",
    );
    let owner = code(&unit, "run");
    let graph = unit.value_sources(owner).unwrap();
    let flow = graph.flow().unwrap();
    let checked = unit.code_body(owner).unwrap();
    let mut counts = [0; 3];
    for (point, block) in checked.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            let at = flow::Location { point, action };
            let (arguments, category) = match event.operation {
                flow::Operation::PrepareArgument { call, argument } => {
                    let ExpressionKind::Call { arguments, .. } =
                        &checked.typed().body().expressions().get(call).unwrap().kind
                    else {
                        panic!("call")
                    };
                    (&arguments[argument..=argument], 0)
                }
                flow::Operation::CommitCall(call) => {
                    let ExpressionKind::Call { arguments, .. } =
                        &checked.typed().body().expressions().get(call).unwrap().kind
                    else {
                        panic!("call")
                    };
                    (arguments.as_ref(), 1)
                }
                flow::Operation::Evaluate(id) => {
                    if let ExpressionKind::PrimaryCondition { source } =
                        checked.typed().body().expressions().get(id).unwrap().kind
                    {
                        assert!(flow.root(at, source).is_some());
                        counts[2] += 1;
                    }
                    continue;
                }
                _ => continue,
            };
            for argument in arguments {
                let (crate::value_access::ArgumentAction::ReserveRead(target)
                | crate::value_access::ArgumentAction::ReserveUpdate(target)) = argument.action
                else {
                    continue;
                };
                assert!(flow.root(at, target).is_some());
                if category == 1 {
                    let updated = flow.root_after(at, target);
                    if argument.operation == nocter_model::ValueInputOperation::Edit {
                        let ValueSource::Select { source, .. } =
                            graph.node(updated.unwrap()).unwrap().value
                        else {
                            panic!("updated call root");
                        };
                        assert!(matches!(
                            graph.node(source).unwrap().value,
                            ValueSource::CallWrite { .. }
                        ));
                    } else {
                        let ValueSource::Select { source: before, .. } =
                            graph.node(flow.root(at, target).unwrap()).unwrap().value
                        else {
                            panic!("read root")
                        };
                        let ValueSource::Select { source: after, .. } =
                            graph.node(updated.unwrap()).unwrap().value
                        else {
                            panic!("post-call read root")
                        };
                        assert_eq!(
                            before, after,
                            "the fixture's separate read root is unchanged"
                        );
                    }
                }
                counts[category] += 1;
            }
        }
    }
    assert_eq!(counts, [2, 2, 1]);
}

#[test]
fn a_read_projection_keeps_the_post_call_parent_changed_through_a_separate_edit_projection() {
    let unit = accepted(
        "func touch(value: i32, edit target: i32): i32 { target = 42\nvalue }\nfunc run(): i32 { var pair = (1, 2)\ntouch(pair.0, pair.1) }",
    );
    let owner = code(&unit, "run");
    let graph = unit.value_sources(owner).unwrap();
    let checked = unit.code_body(owner).unwrap();
    let roots = graph.flow().unwrap();
    let mut calls = 0;
    for (point, block) in checked.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            let flow::Operation::CommitCall(call) = event.operation else {
                continue;
            };
            let ExpressionKind::Call { arguments, .. } =
                &checked.typed().body().expressions().get(call).unwrap().kind
            else {
                panic!("call")
            };
            let [read, edit] = arguments.as_ref() else {
                panic!("two projections")
            };
            let crate::value_access::ArgumentAction::ReserveRead(read) = read.action else {
                panic!("read")
            };
            let crate::value_access::ArgumentAction::ReserveUpdate(edit) = edit.action else {
                panic!("edit")
            };
            let at = flow::Location { point, action };
            let parent = |root| {
                let ValueSource::Select { source, ref path } = graph.node(root).unwrap().value
                else {
                    panic!("tuple root")
                };
                assert!(path.is_root());
                source
            };
            let before = parent(roots.root(at, read).unwrap());
            let after = parent(roots.root_after(at, read).unwrap());
            assert_ne!(before, after);
            assert_eq!(after, parent(roots.root_after(at, edit).unwrap()));
            let [old_carrier] = roots.root_carriers(at, read).unwrap() else {
                panic!("one old parent");
            };
            let [new_carrier] = roots.root_carriers_after(at, read).unwrap() else {
                panic!("one updated parent");
            };
            assert_eq!(old_carrier.slot, new_carrier.slot);
            assert!(old_carrier.path.is_root());
            assert!(new_carrier.path.is_root());
            assert_eq!(old_carrier.source, before);
            assert_eq!(new_carrier.source, after);
            assert_eq!(
                roots.root_carriers_after(at, read),
                roots.root_carriers_after(at, edit)
            );
            let ValueSource::CallWrite {
                call: changed,
                ref path,
                ..
            } = graph.node(after).unwrap().value
            else {
                panic!("post-call parent")
            };
            assert_eq!(changed, call);
            assert_eq!(path.fields(), &[ValueProjection::Tuple(1)]);
            calls += 1;
        }
    }
    assert_eq!(calls, 1);
}

#[test]
fn guarded_carriers_retain_the_actual_source_on_each_side_of_a_replacement() {
    for carrier in ["i32?", "i32!"] {
        let unit = accepted(&format!(
            "func make(value: i32): {carrier} {{ value }}
                func inspect(value: i32): i32 {{ value }}\nfunc run(): i32 {{
                var current = make(41)\nlet before = inspect(current!)\ncurrent = make(42)
                let after = inspect(current!)\nbefore + after\n}}"
        ));
        let owner = code(&unit, "run");
        let graph = unit.value_sources(owner).unwrap();
        let body = unit.code_body(owner).unwrap().typed().body();
        let calls = factories(&unit, owner);
        assert_eq!(calls.len(), 2);
        let mut snapshots = Vec::new();
        for (_, node) in graph.nodes() {
            let Some(expression) = node.expression else {
                continue;
            };
            let ExpressionKind::GuardView {
                source: Referent::Place(place),
                ..
            } = body.expressions().get(expression).unwrap().kind
            else {
                continue;
            };
            let ValueSource::Adapt { source } = node.value else {
                panic!("a tag assertion must retain its carrier source");
            };
            let ValueSource::Select { source: root, path } = &graph.node(source).unwrap().value
            else {
                panic!("the fixture guards an existing whole local");
            };
            assert!(path.is_root());
            let slot = ValueSourceSlot::Local(body.places().get(place).unwrap().local);
            let at = node.origin.unwrap();
            match graph
                .flow()
                .unwrap()
                .read(
                    ValueSourceBoundary {
                        point: at.point,
                        action: at.action,
                    },
                    slot,
                )
                .unwrap()
            {
                ValueSourceRead::Value(selected) => assert_eq!(*root, selected),
                ValueSourceRead::Incoming(_) => {
                    let ValueSource::Merge {
                        point,
                        slot: selected,
                        ..
                    } = graph.node(*root).unwrap().value
                    else {
                        panic!("the guard must retain the original boundary's merge");
                    };
                    assert_eq!(point, at.point);
                    assert_eq!(selected, slot);
                }
                ValueSourceRead::Empty => panic!("the fixture guards an initialized local"),
            }
            snapshots.push(reachable_calls(graph, source));
            assert_eq!(
                node.data,
                body.expressions()
                    .get(expression)
                    .unwrap()
                    .ty
                    .access()
                    .map(nocter_model::ValueAccess::data)
            );
        }
        assert_eq!(
            snapshots,
            // Replacement retains the old root as metadata; it is not a second current value.
            vec![
                BTreeSet::from([calls[0]]),
                BTreeSet::from([calls[0], calls[1]])
            ]
        );
    }
}

#[test]
fn prepared_data_exhaustion_retains_the_argument_site_and_allows_retry() {
    let unit =
        accepted("func identity(value: i32): i32 { value }\nfunc run(): i32 { identity(42) }");
    let owner = code(&unit, "run");
    let frame = unit.program().bind_code(owner, []).unwrap();
    let mut exhausted =
        nocter_constant_evaluation::state::WorkBudget::new(std::num::NonZeroU64::new(1).unwrap());
    let failure = unit
        .instantiate_prepared_data(&frame, &mut exhausted)
        .unwrap_err();
    let crate::value_access::PreparedDataFailure::Argument {
        site,
        index,
        failure,
    } = failure
    else {
        panic!("the original preparation")
    };
    assert_eq!(site.owner, owner);
    assert_eq!(index, 0);
    assert_eq!(failure, InstantiationFailure::WorkLimit);
    let mut work = nocter_constant_evaluation::state::WorkBudget::new(
        std::num::NonZeroU64::new(1_000).unwrap(),
    );
    let prepared = unit.instantiate_prepared_data(&frame, &mut work).unwrap();
    let data = unit
        .program()
        .data_type(nocter_model::BuiltinType::I32)
        .unwrap();
    assert_eq!(
        prepared.get(site.call, index),
        Some(&unit.program().instantiate_closed_data(data).unwrap())
    );
    assert_eq!(prepared.iter().count(), 1);
    assert!(prepared.get(site.call, 1).is_none());
}

#[test]
fn body_input_metadata_rejects_an_ordinary_local_before_using_its_closed_schema() {
    let unit = accepted("func run(value: i32): i32 { let copied = value\ncopied }");
    let owner = code(&unit, "run");
    let frame = unit.program().bind_code(owner, []).unwrap();
    let body = unit.code_body(owner).unwrap().typed().body();
    let input = body.inputs().explicit()[0].local;
    let local = body
        .locals()
        .iter()
        .find(|(local, _)| *local != input)
        .unwrap()
        .0;
    let data = body.locals().get(input).unwrap().data;
    assert_eq!(body.locals().get(local).unwrap().data, data);
    let mut work =
        nocter_constant_evaluation::state::WorkBudget::new(std::num::NonZeroU64::new(100).unwrap());
    assert_eq!(
        unit.instantiate_input_data(&frame, input, &mut work)
            .unwrap(),
        unit.program().instantiate_closed_data(data).unwrap()
    );
    assert_eq!(
        unit.instantiate_input_data(&frame, local, &mut work),
        Err(InstantiationFailure::Unresolved)
    );
}

#[test]
fn contextual_body_input_metadata_uses_one_budget_and_allows_retry() {
    let unit = accepted(
        "func first(): i32 { 42 }\nfunc forward(take action: func(): i32): func(): i32 { action^ }",
    );
    let owner = code(&unit, "forward");
    let ValueCode::Declaration(declaration) = owner else {
        panic!("named function")
    };
    let parameter = unit
        .program()
        .codes()
        .get(declaration)
        .unwrap()
        .parameters()[0];
    let selected = nocter_declarations::value_access::FunctionWitness {
        origin: code(&unit, "first").origin().unwrap(),
        invocation: nocter_model::ValueInvocation::Read,
    };
    let frame = unit
        .program()
        .bind_code(owner, [(parameter.into(), selected)])
        .unwrap();
    let body = unit.code_body(owner).unwrap().typed().body();
    let input = body.inputs().explicit()[0].local;
    let data = body.locals().get(input).unwrap().data;
    let mut exhausted =
        nocter_constant_evaluation::state::WorkBudget::new(std::num::NonZeroU64::new(1).unwrap());
    assert_eq!(
        unit.instantiate_input_data(&frame, input, &mut exhausted),
        Err(InstantiationFailure::WorkLimit)
    );
    let mut work = nocter_constant_evaluation::state::WorkBudget::new(
        std::num::NonZeroU64::new(1_000).unwrap(),
    );
    let expected = unit.program().instantiate_data(data, &frame).unwrap();
    assert_eq!(
        unit.instantiate_input_data(&frame, input, &mut work)
            .unwrap(),
        expected
    );
    assert_eq!(
        unit.instantiate_result_contract(&frame, &mut work).unwrap(),
        expected
    );
}

#[test]
fn repeated_alias_writes_select_the_current_referent_instead_of_the_original_binding() {
    let unit = accepted("func make(value: i32): i32 { value }
func run(): i32 { var value = make(40)\nedit alias = value\nalias = make(41)\nalias = make(42)\nvalue }");
    let owner = code(&unit, "run");
    let checked = unit.code_body(owner).unwrap();
    let graph = unit.value_sources(owner).unwrap();
    let positions = graph.flow().unwrap();
    let calls = factories(&unit, owner);
    assert_eq!(calls.len(), 3);
    let mut writes = 0;
    for (point, block) in checked.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            let flow::Operation::Assign { target, .. } = event.operation else {
                continue;
            };
            let previous = positions
                .referent(flow::Location { point, action }, target)
                .unwrap();
            let ValueSource::Select { source, path } = &graph.node(previous).unwrap().value else {
                panic!("actual selected root")
            };
            assert!(path.is_root());
            if writes == 0 {
                assert_eq!(reachable_calls(graph, *source), BTreeSet::from([calls[0]]));
            } else {
                let ValueSource::Replace {
                    value, path, weak, ..
                } = &graph.node(*source).unwrap().value
                else {
                    panic!("the previous replacement, not the alias's original binding")
                };
                assert!(!weak && path.is_root());
                assert_eq!(reachable_calls(graph, *value), BTreeSet::from([calls[1]]));
            }
            writes += 1;
        }
    }
    assert_eq!(writes, 2);
}

#[test]
fn sparse_definition_iteration_exposes_only_the_final_write_of_an_original_action() {
    let unit = accepted(
        "func touch(edit left: i32, edit right: i32): void { left += 1\nright += 1 }
func run(): i32 { var pair = (20, 20)\ntouch(pair.0, pair.1)\npair.0 + pair.1 }",
    );
    let graph = unit.value_sources(code(&unit, "run")).unwrap();
    let positions = graph.flow().unwrap();
    let mut sites = BTreeSet::new();
    let mut definitions = 0;
    for ((at, slot), source) in positions.definitions() {
        assert!(sites.insert((at, slot)));
        assert_eq!(
            positions.read(
                ValueSourceBoundary {
                    point: at.point,
                    action: at.action + 1
                },
                slot
            ),
            Some(ValueSourceRead::Value(source))
        );
        definitions += 1;
    }
    assert!(
        definitions < positions.write_count(),
        "the two nonoverlapping edit arguments write one root in one commit"
    );
    for ((at, expression), source) in positions.operands() {
        assert_eq!(positions.operand(at, expression), Some(source));
    }
}

#[test]
fn ordinary_operands_retain_their_actual_source_before_later_operand_mutation() {
    let unit = accepted(
        "func make(value: i32): i32 { value }
func run(): i32 {
    var current = make(41)
    let _ = current + if true { current = make(42)\n1 } else { 1 }
    current += 0
    if current == 42 { current } else { 0 }
}",
    );
    let owner = code(&unit, "run");
    let graph = unit.value_sources(owner).unwrap();
    let positions = graph.flow().unwrap();
    let checked = unit.code_body(owner).unwrap();
    let calls = factories(&unit, owner);
    assert_eq!(calls.len(), 2);
    let mut counts = [0; 4];
    let mut snapshots = 0;
    for (point, block) in checked.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            let (value, category) = match event.operation {
                flow::Operation::PrepareOperand { value, .. } => (value, 0),
                flow::Operation::Condition(value) => (value, 1),
                flow::Operation::Discard(value) => (value, 2),
                flow::Operation::CompoundAssign { value, .. } => (value, 3),
                _ => continue,
            };
            let source = positions
                .operand(flow::Location { point, action }, value)
                .expect("the original operand has an occurrence source");
            assert_ne!(graph.node(source).unwrap().value, ValueSource::Empty);
            counts[category] += 1;
            if category == 0 && reachable_calls(graph, source) == BTreeSet::from([calls[0]]) {
                snapshots += 1;
            }
        }
    }
    assert!(counts.into_iter().all(|count| count > 0));
    assert_eq!(
        snapshots, 1,
        "the first arithmetic operand precedes replacement"
    );
}

#[test]
fn staged_returns_publish_their_original_source_only_after_the_staging_action() {
    let unit = accepted(SOURCE);
    let owner = code(&unit, "captured_components");
    let frame = unit.program().bind_code(owner, []).unwrap();
    let checked = unit.code_body(owner).unwrap();
    let data = checked.typed().result_data();
    for (point, block) in checked.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            if !matches!(event.operation, flow::Operation::StageReturn(_)) {
                continue;
            }
            let before = ValueSourceSelection {
                boundary: ValueSourceBoundary { point, action },
                slot: ValueSourceSlot::Return,
                path: ValuePath::default(),
            };
            let mut work = nocter_constant_evaluation::state::WorkBudget::new(
                std::num::NonZeroU64::new(10_000).unwrap(),
            );
            assert_eq!(
                unit.instantiate_stored_data(&frame, &before, data, &mut work),
                Err(InstantiationFailure::Unresolved)
            );
            let after = ValueSourceSelection {
                boundary: ValueSourceBoundary {
                    point,
                    action: action + 1,
                },
                ..before
            };
            assert_eq!(
                unit.instantiate_stored_data(&frame, &after, data, &mut work)
                    .unwrap(),
                unit.program().instantiate_closed_data(data).unwrap()
            );
            return;
        }
    }
    panic!("original staged return");
}

#[test]
fn assignment_boundaries_retain_the_previous_and_new_original_value_definitions() {
    let unit = accepted(SOURCE);
    let owner = code(&unit, "captured_components");
    let graph = unit.value_sources(owner).unwrap();
    let positions = graph.flow().unwrap();
    let checked = unit.code_body(owner).unwrap();
    let typed = checked.typed().body();
    let calls = factories(&unit, owner);
    for (point, block) in checked.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            let flow::Operation::Assign {
                target: Referent::Place(place),
                ..
            } = event.operation
            else {
                continue;
            };
            let place = typed.places().get(place).unwrap();
            if !place.fields.is_root() {
                continue;
            }
            let slot = ValueSourceSlot::Local(place.local);
            let ValueSourceRead::Value(before) = positions
                .read(ValueSourceBoundary { point, action }, slot)
                .unwrap()
            else {
                panic!("prior definition")
            };
            let ValueSourceRead::Value(after) = positions
                .read(
                    ValueSourceBoundary {
                        point,
                        action: action + 1,
                    },
                    slot,
                )
                .unwrap()
            else {
                panic!("new definition")
            };
            assert_ne!(before, after);
            assert_eq!(reachable_calls(graph, before), BTreeSet::from([calls[0]]));
            let ValueSource::Replace { source, value, .. } = graph.node(after).unwrap().value
            else {
                panic!("replacement definition")
            };
            assert_eq!(source, before);
            assert_eq!(reachable_calls(graph, value), BTreeSet::from([calls[1]]));
            assert!(
                positions
                    .read(
                        ValueSourceBoundary {
                            point,
                            action: block.actions.len() + 1
                        },
                        slot
                    )
                    .is_none()
            );
            return;
        }
    }
    panic!("original root assignment");
}

#[test]
fn prepared_slots_keep_the_actual_operand_source_at_each_original_preparation_action() {
    let unit = accepted(SOURCE);
    let owner = code(&unit, "prepared_snapshot");
    let graph = unit.value_sources(owner).unwrap();
    let positions = graph.flow().unwrap();
    let checked = unit.code_body(owner).unwrap();
    let mut count = 0;
    for (point, block) in checked.graph().points().iter() {
        for (action, event) in block.actions.iter().enumerate() {
            let flow::Operation::PrepareArgument { call, argument } = event.operation else {
                continue;
            };
            let ExpressionKind::Call { arguments, .. } =
                &checked.typed().body().expressions().get(call).unwrap().kind
            else {
                panic!("call")
            };
            let original = arguments[argument].value;
            let source = positions
                .operand(flow::Location { point, action }, original)
                .unwrap();
            assert_eq!(
                positions.read(
                    ValueSourceBoundary {
                        point,
                        action: action + 1
                    },
                    ValueSourceSlot::Argument {
                        call,
                        index: argument
                    }
                ),
                Some(ValueSourceRead::Value(source))
            );
            count += 1;
        }
    }
    assert!(count >= 4);
}
