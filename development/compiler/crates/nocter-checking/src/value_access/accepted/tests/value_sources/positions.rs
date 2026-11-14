//! Source-definition boundaries retain old values and exact preparation-time operand reads.
use super::*;
use crate::value_access::{InstantiationFailure, ValueSourceSelection};
use crate::value_access::{Referent, ValueSourceBoundary, ValueSourceRead, ValueSourceSlot, flow};

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
