//! Actual private source lowering and integrity mutations; no substitute source interpreter.
use super::*;
use crate::{MirArithmeticCheck, MirBinaryOperation};
use nocter_checking::value_access::accepted::SourceUnit;
use nocter_model::{MirLocalId, ValueInputOperation};
use nocter_target_program::value_access::InstanceProgram;
use nocter_test_support::ValueSourceFixture;
use std::sync::Arc;

fn lower(source: &str) -> Program {
    let fixture = ValueSourceFixture::new(source);
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    let root = unit
        .program()
        .resolve(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let instances = Arc::new(InstanceProgram::prepare(unit, root).unwrap());
    let lifecycle = lifecycle::Program::prepare(instances).unwrap();
    Program::lower(&lifecycle, RuntimeAbiIdentity::Arm64DarwinV1).unwrap()
}
fn key(body: &Body, slot: MirLocalId) -> SlotKey {
    body.slots.get(slot).unwrap().key
}
fn operations(body: &Body) -> impl Iterator<Item = &Operation> {
    body.actions.values().flat_map(|a| &a.operations)
}

#[test]
fn snapshots_precede_later_operand_calls_and_checked_arithmetic_uses_the_snapshots() {
    let program = lower(
        "func next(edit value: i32): i32 { value += 1\nvalue }\nfunc run(): i32 { var value = 0\nvalue + next(value) }",
    );
    let body = &program.bodies[&program.root];
    let operations: Vec<_> = operations(body).collect();
    let left = operations.iter().position(|op| matches!(op, Operation::Copy { destination, .. } if matches!(key(body, destination.slot), SlotKey::Operand { index: 0, .. }))).unwrap();
    let call = operations
        .iter()
        .position(|op| matches!(op, Operation::Call(_)))
        .unwrap();
    let right = operations.iter().position(|op| matches!(op, Operation::Copy { destination, .. } if matches!(key(body, destination.slot), SlotKey::Operand { index: 1, .. }))).unwrap();
    let add = operations
        .iter()
        .position(|op| {
            matches!(
                op,
                Operation::Binary {
                    operation: MirBinaryOperation::Add,
                    ..
                }
            )
        })
        .unwrap();
    assert!(left < call && call < right && right < add);
    let Operation::Binary {
        left: lhs,
        right: rhs,
        check,
        ..
    } = operations[add]
    else {
        panic!("add");
    };
    let Operation::Copy {
        destination: snapshot_left,
        ..
    } = operations[left]
    else {
        panic!("left snapshot");
    };
    let Operation::Copy {
        destination: snapshot_right,
        ..
    } = operations[right]
    else {
        panic!("right snapshot");
    };
    assert_eq!(lhs, snapshot_left);
    assert_eq!(rhs, snapshot_right);
    assert_eq!(*check, MirArithmeticCheck::Required);
}

#[test]
fn comparison_normalization_keeps_source_snapshot_order() {
    for (source, expected, swap, invert) in [
        ("==", MirBinaryOperation::Equal, false, false),
        ("!=", MirBinaryOperation::Equal, false, true),
        ("<", MirBinaryOperation::Less, false, false),
        (">", MirBinaryOperation::Less, true, false),
        ("<=", MirBinaryOperation::Less, true, true),
        (">=", MirBinaryOperation::Less, false, true),
    ] {
        let program = lower(&format!(
            "func run(left: i32, right: i32): bool {{ left {source} right }}"
        ));
        let body = &program.bodies[&program.root];
        let snapshots = operations(body)
            .filter_map(|op| match op {
                Operation::Copy { destination, .. }
                    if matches!(key(body, destination.slot), SlotKey::Operand { .. }) =>
                {
                    Some(destination.slot)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(snapshots.len(), 2);
        let (left, right) = if swap {
            (snapshots[1], snapshots[0])
        } else {
            (snapshots[0], snapshots[1])
        };
        assert!(operations(body).any(|op| matches!(op, Operation::Binary { operation, left: lhs, right: rhs, invert: actual, check: MirArithmeticCheck::NotRequired, .. } if *operation == expected && lhs.slot == left && rhs.slot == right && *actual == invert)));
    }
}

#[test]
fn short_circuit_calls_stay_in_the_long_branch_and_constant_short_paths_have_no_call() {
    let source = "func touch(edit value: i32): bool { value += 1\ntrue }\n";
    let program = lower(&format!(
        "{source}func run(flag: bool): bool {{ var value = 0\nflag && touch(value) }}"
    ));
    let body = &program.bodies[&program.root];
    let Terminator::Branch { yes, no, .. } = &body.terminators[&body.entry] else {
        panic!("branch");
    };
    let call = body
        .actions
        .iter()
        .find(|(_, action)| {
            action
                .operations
                .iter()
                .any(|op| matches!(op, Operation::Call(_)))
        })
        .unwrap()
        .0
        .point;
    assert_eq!(call, *yes);
    assert_ne!(call, *no);
    assert!(
        body.actions
            .iter()
            .filter(|(at, _)| at.point == *no)
            .flat_map(|(_, action)| &action.operations)
            .any(|op| matches!(
                op,
                Operation::Constant {
                    value: Constant::Boolean(false),
                    ..
                }
            ))
    );
    let constant = lower(&format!(
        "{source}func run(): bool {{ var value = 0\nfalse && touch(value) }}"
    ));
    assert_eq!(constant.bodies.len(), 1);
    assert!(
        !operations(&constant.bodies[&constant.root]).any(|op| matches!(op, Operation::Call(_)))
    );
}

#[test]
fn alias_bindings_and_compound_updates_keep_addresses_separate_from_data() {
    let program = lower(
        "func run(): i32 { var count = 1\nread source = count\nlet snapshot = source\nedit target = count\ntarget += 2\nsnapshot + target }",
    );
    let body = &program.bodies[&program.root];
    let aliases = operations(body)
        .filter_map(|op| match op {
            Operation::Storage(Effect::BindAddress {
                destination,
                source,
            }) if matches!(key(body, *destination), SlotKey::Local(_)) => {
                Some((*destination, source.slot))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(aliases.len(), 2);
    let origin = |mut slot| {
        let mut seen = std::collections::BTreeSet::new();
        while seen.insert(slot) {
            let parent = operations(body).find_map(|op| match op {
                Operation::Storage(Effect::BindAddress {
                    destination,
                    source,
                }) if *destination == slot => Some(source.slot),
                _ => None,
            });
            let Some(parent) = parent else {
                break;
            };
            slot = parent;
        }
        slot
    };
    let owner = origin(aliases[0].1);
    assert_eq!(owner, origin(aliases[1].1));
    assert_eq!(body.slots.get(owner).unwrap().storage, Storage::Payload);
    assert!(operations(body).any(|op| matches!(op, Operation::Binary { destination, left, check: MirArithmeticCheck::Required, .. } if destination.slot == aliases[1].0 && destination == left)));
    assert!(
        program
            .runtime
            .types()
            .iter()
            .all(|(_, kind)| !matches!(kind, nocter_runtime_contract::RuntimeType::Borrow { .. }))
    );
}

#[test]
fn consuming_adapter_call_projects_the_environment_and_protects_the_answer() {
    let program = lower(
        "func observe(input: error): i32 { 42 }\nfunc apply(take action: once(): i32): i32 { action^() }\nfunc run(take input: error): i32 { let action = (input^;): i32 { observe(input) }\napply(action^) }",
    );
    let mut found = false;
    for body in program.bodies.values() {
        for action in body.actions.values() {
            for operation in &action.operations {
                let Operation::Call(call) = operation else {
                    continue;
                };
                let callee = &program.bodies[&call.target];
                if callee.signature.environment().is_none() {
                    continue;
                }
                let environment = &call.inputs[0];
                assert_eq!(
                    environment.source.projections.as_ref(),
                    [Projection::Environment]
                );
                assert_eq!(environment.operation, ValueInputOperation::Read);
                assert_eq!(
                    body.slots.get(environment.source.slot).unwrap().storage,
                    Storage::Payload
                );
                assert_eq!(
                    key(body, call.answer.slot),
                    SlotKey::Answer(match key(body, environment.source.slot) {
                        SlotKey::Argument { call, .. } => call,
                        _ => panic!("carrier"),
                    })
                );
                for alternative in &action.alternatives {
                    assert!(
                        matches!(alternative.after.as_ref(), [Effect::Dispose { source, .. }, Effect::Transfer { source: answer, .. }] if source.slot == environment.source.slot && answer == &call.answer)
                    );
                }
                found = true;
            }
        }
    }
    assert!(found);
}

#[test]
fn malformed_computation_control_and_call_contracts_cannot_freeze() {
    let source =
        lower("func child(value: i32): i32 { value + 1 }\nfunc run(flag: bool): i32 { child(7) }");
    let mut missing_graph = source.clone();
    missing_graph
        .bodies
        .get_mut(&source.root)
        .unwrap()
        .terminators
        .clear();
    assert!(validate::program(&missing_graph).is_err());
    let mut wrong_input = source.clone();
    let call = wrong_input
        .bodies
        .get_mut(&source.root)
        .unwrap()
        .actions
        .values_mut()
        .flat_map(|a| &mut a.operations)
        .find_map(|op| match op {
            Operation::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.inputs[0].operation = ValueInputOperation::Take;
    assert!(validate::program(&wrong_input).is_err());
    let mut unchecked = source.clone();
    let check = unchecked
        .bodies
        .values_mut()
        .flat_map(|body| body.actions.values_mut())
        .flat_map(|a| &mut a.operations)
        .find_map(|op| match op {
            Operation::Binary { check, .. } => Some(check),
            _ => None,
        })
        .unwrap();
    *check = MirArithmeticCheck::NotRequired;
    assert!(validate::program(&unchecked).is_err());
    let mut bad_flag = source.clone();
    let alternative = &mut bad_flag
        .bodies
        .get_mut(&source.root)
        .unwrap()
        .actions
        .values_mut()
        .next()
        .unwrap()
        .alternatives[0];
    alternative.tests = Box::new([BitValue {
        word: 0,
        mask: 1,
        value: true,
    }]);
    assert!(validate::program(&bad_flag).is_err());
}

#[test]
fn frozen_record_literals_retain_schema_and_reject_mismatched_payloads() {
    let mut program = lower(
        "struct Pair { left: i32 }\nconst DATA: Pair = Pair { left: 42 }\nfunc run(): i32 { let data = DATA\ndata.left }",
    );
    assert_eq!(program.bodies.len(), 1);
    let body = program.bodies.get_mut(&program.root).unwrap();
    let value = body
        .actions
        .values_mut()
        .flat_map(|action| &mut action.operations)
        .find_map(|operation| {
            if let Operation::Frozen { value, .. } = operation {
                Some(value)
            } else {
                None
            }
        })
        .unwrap();
    assert!(matches!(
        value.node(),
        nocter_model::FrozenNode::Aggregate(_)
    ));
    *value = nocter_model::FrozenData::new(vec![nocter_model::FrozenNode::Unit]).unwrap();
    assert!(validate::program(&program).is_err());
}

#[test]
fn static_views_are_not_call_inputs_and_global_shapes_are_validated_once() {
    let mut program = lower("static DATA: i32 = 42\nfunc run(): i32 { DATA }");
    let body = &program.bodies[&program.root];
    assert!(body.inputs.is_empty());
    assert_eq!(body.statics.len(), 1);
    assert_eq!(program.statics.len(), 1);
    let id = body.statics[0].1;
    program.statics.get_mut(&id).unwrap().value =
        nocter_model::FrozenData::new(vec![nocter_model::FrozenNode::Unit]).unwrap();
    assert!(validate::program(&program).is_err());
    let mut program = lower("static DATA: i32 = 42\nfunc run(): i32 { DATA }");
    program.bodies.get_mut(&program.root).unwrap().statics = Box::new([]);
    assert!(validate::program(&program).is_err());
}

#[test]
fn evaluated_index_destination_role_is_validated_before_publication() {
    for (source, expected) in [
        (
            "func run(): i32 { var values = [40, 7]\nvalues[0] += 2\nvalues[0] }",
            false,
        ),
        ("func run(): i32 { [40, 42][1] }", true),
    ] {
        let mut program = lower(source);
        let body = program.bodies.get_mut(&program.root).unwrap();
        let operation = body
            .actions
            .values_mut()
            .flat_map(|a| a.operations.iter_mut())
            .find(|op| matches!(op, Operation::Index { .. }))
            .unwrap();
        let Operation::Index { copy, .. } = operation else {
            unreachable!()
        };
        assert_eq!(*copy, expected);
        *copy = !*copy;
        assert!(matches!(
            validate::program(&program),
            Err(Failure::InvalidBody(_))
        ));
    }
}

#[test]
fn frozen_array_schema_rejects_wrong_counts_and_element_types() {
    for value in [
        nocter_model::FrozenData::new(vec![
            nocter_model::FrozenNode::Scalar(nocter_model::ConstantValue::Integer(1)),
            nocter_model::FrozenNode::Repeated {
                element: 0,
                length: std::num::NonZeroU64::new(3).unwrap(),
                replacements: Box::new([]),
            },
        ])
        .unwrap(),
        nocter_model::FrozenData::new(vec![
            nocter_model::FrozenNode::Scalar(nocter_model::ConstantValue::Bool(true)),
            nocter_model::FrozenNode::Repeated {
                element: 0,
                length: std::num::NonZeroU64::new(2).unwrap(),
                replacements: Box::new([]),
            },
        ])
        .unwrap(),
    ] {
        let mut program = lower("const DATA: [i32; 2] = [1; 2]\nfunc run(): i32 { DATA[0] }");
        let operation = program
            .bodies
            .get_mut(&program.root)
            .unwrap()
            .actions
            .values_mut()
            .flat_map(|action| action.operations.iter_mut())
            .find(|op| matches!(op, Operation::Frozen { .. }))
            .unwrap();
        let Operation::Frozen {
            value: original, ..
        } = operation
        else {
            unreachable!()
        };
        *original = value;
        assert!(matches!(
            validate::program(&program),
            Err(Failure::InvalidBody(_))
        ));
    }
}

#[test]
fn repeated_storage_checks_the_seed_against_its_homogeneous_result() {
    let mut program = lower("func run(): i32 { let values = [42; 2]\nvalues[0] }");
    let operation = program
        .bodies
        .get_mut(&program.root)
        .unwrap()
        .actions
        .values_mut()
        .flat_map(|a| a.operations.iter_mut())
        .find(|op| matches!(op, Operation::Repeat { .. }))
        .unwrap();
    let Operation::Repeat {
        source,
        destination,
    } = operation
    else {
        unreachable!()
    };
    source.data = destination.data;
    assert!(matches!(
        validate::program(&program),
        Err(Failure::InvalidBody(_))
    ));
}

#[test]
fn array_index_dispositions_survive_freezing_and_result_producing_trap_flags_reject() {
    for (source, expected) in [
        (
            "func run(): i32 { [20, 42][1] }",
            nocter_model::IndexBoundsCheck::ProvenInBounds,
        ),
        (
            "func select(values: [i32; 2], index: usize): i32 { values[index] }\nfunc run(): i32 { select([20, 42], 1) }",
            nocter_model::IndexBoundsCheck::Required,
        ),
    ] {
        let mut program = lower(source);
        let checks: Vec<_> = program
            .bodies
            .values()
            .flat_map(operations)
            .filter_map(|operation| match operation {
                Operation::Index { bounds, .. } => Some(*bounds),
                _ => None,
            })
            .collect();
        assert_eq!(checks, [expected]);
        let operation = program
            .bodies
            .values_mut()
            .flat_map(|body| body.actions.values_mut())
            .flat_map(|action| action.operations.iter_mut())
            .find(|operation| matches!(operation, Operation::Index { .. }))
            .unwrap();
        let Operation::Index { bounds, .. } = operation else {
            unreachable!()
        };
        *bounds = nocter_model::IndexBoundsCheck::ProvenTrap;
        assert!(matches!(
            validate::program(&program),
            Err(Failure::InvalidBody(_))
        ));
    }
}

#[test]
fn proven_bounds_failure_is_a_terminal_action_without_a_selected_result() {
    let source = "struct Node { next: [Node; 0] }\nfunc run(): i32 { let values: [Node; 0] = []\nlet _ = values[0]\n42 }";
    let program = lower(source);
    let body = program.bodies.get(&program.root).unwrap();
    let (&at, action) = body
        .actions
        .iter()
        .find(|(_, action)| matches!(action.operations.as_ref(), [Operation::BoundsTrap]))
        .unwrap();
    assert!(matches!(
        body.terminators.get(&at.point),
        Some(Terminator::Unreachable)
    ));
    assert!(!operations(body).any(|operation| matches!(operation, Operation::Index { .. })));
    for mutation in 0..3 {
        let mut bad = program.clone();
        let body = bad.bodies.get_mut(&bad.root).unwrap();
        match mutation {
            0 => {
                body.terminators
                    .insert(at.point, Terminator::Jump(at.point));
            }
            1 => {
                body.actions.get_mut(&at).unwrap().operations =
                    vec![Operation::BoundsTrap, Operation::BoundsTrap].into();
            }
            _ => {
                body.actions.insert(
                    Location {
                        point: at.point,
                        action: at.action + 1,
                    },
                    action.clone(),
                );
            }
        }
        assert!(matches!(
            validate::program(&bad),
            Err(Failure::InvalidBody(_))
        ));
    }
}

#[test]
fn source_partial_array_cleanup_freezes_half_open_intervals_and_rejects_invalid_bounds() {
    let program = lower(
        "struct Item { value: i32 }\nfunc run(): i32 { let items = [Item { value: 40 }, Item { value: 2 }]\nlet first = items[0]^\nfirst.value + items[1].value }",
    );
    let body = program.bodies.get(&program.root).unwrap();
    assert!(
        body.actions
            .values()
            .flat_map(|a| &a.alternatives)
            .flat_map(|a| a.before.iter().chain(&a.after))
            .any(|effect| matches!(
                effect,
                Effect::DisposeArrayElements {
                    range: nocter_model::ArrayRange::Constant { start: 1, end: 2 },
                    ..
                }
            ))
    );
    for (start, end) in [(0, 0), (2, 1), (0, 3)] {
        let mut bad = program.clone();
        let effect = bad
            .bodies
            .get_mut(&bad.root)
            .unwrap()
            .actions
            .values_mut()
            .flat_map(|a| &mut a.alternatives)
            .flat_map(|a| a.before.iter_mut().chain(&mut a.after))
            .find(|effect| matches!(effect, Effect::DisposeArrayElements { .. }))
            .unwrap();
        let Effect::DisposeArrayElements {
            range: nocter_model::ArrayRange::Constant { start: a, end: b },
            ..
        } = effect
        else {
            unreachable!()
        };
        *a = start;
        *b = end;
        assert!(matches!(
            validate::program(&bad),
            Err(Failure::InvalidBody(_))
        ));
    }
}

#[test]
fn prefix_backing_can_drive_ranges_but_cannot_become_an_immutable_element_address() {
    let source = nocter_test_support::RETAINED_ARRAY_SOURCES
        .iter()
        .find(|(name, _)| *name == "dynamic-element-prefix-loop")
        .unwrap()
        .1;
    let program = lower(source);
    let body = &program.bodies[&program.root];
    let (endpoint, frontier) = body
        .slots
        .iter()
        .find_map(|(id, slot)| {
            if let SlotKey::ArrayFrontier(frontier) = slot.key {
                Some((id, frontier))
            } else {
                None
            }
        })
        .unwrap();
    assert!(
        !body
            .slots
            .iter()
            .any(|(_, slot)| matches!(slot.key, SlotKey::IndexSnapshot(_)))
    );
    let mut array = None;
    for effect in body
        .actions
        .values()
        .flat_map(|action| &action.alternatives)
        .flat_map(|alt| alt.before.iter().chain(&alt.after))
    {
        match effect {
            Effect::ConsolidateArrayPrefix {
                source,
                endpoint: actual,
            } => {
                assert_eq!(*actual, endpoint);
                array = Some(source.clone());
            }
            Effect::DisposeArrayElements { range, .. } => {
                assert_eq!(*range, nocter_model::ArrayRange::After(endpoint));
            }
            _ => {}
        }
    }
    let mut bad = program.clone();
    let body = bad.bodies.get_mut(&bad.root).unwrap();
    let Operation::Index { destination, .. } = operations(body)
        .find(|op| matches!(op, Operation::Index { .. }))
        .unwrap()
    else {
        unreachable!()
    };
    let mut selected = array.unwrap();
    selected.data = destination.data;
    selected.projections = vec![Projection::Index(endpoint)].into();
    let action = body
        .actions
        .values_mut()
        .find(|action| !action.operations.is_empty())
        .unwrap();
    action.operations = action
        .operations
        .iter()
        .cloned()
        .chain(std::iter::once(Operation::Storage(Effect::End(selected))))
        .collect();
    assert!(matches!(
        validate::program(&bad),
        Err(Failure::InvalidBody(_))
    ));
    // The same bound/representation does not make a point snapshot an advancing endpoint.
    let point_program = lower(
        "struct Item { value: i32 }\nfunc point(index: usize): i32 { let items = [Item { value: 40 }, Item { value: 2 }]\nlet first = items[index]^\nfirst.value }\nfunc run(): i32 { point(0) }",
    );
    let point = point_program
        .bodies
        .values()
        .flat_map(|body| body.slots.iter())
        .find_map(|(_, slot)| match slot.key {
            SlotKey::IndexSnapshot(index) => Some(index),
            _ => None,
        })
        .unwrap();
    assert_eq!(point.bound(), frontier.bound());
    let mut bad = program.clone();
    let body = bad.bodies.get_mut(&bad.root).unwrap();
    body.slots = body.slots.map(|id, slot| {
        let mut slot = slot.clone();
        if id == endpoint {
            slot.key = SlotKey::IndexSnapshot(point);
        }
        slot
    });
    assert!(matches!(
        validate::program(&bad),
        Err(Failure::InvalidBody(_))
    ));
}

#[test]
fn retained_cleanup_integrity_requires_a_matching_scalar_backing_slot() {
    use nocter_checking::value_access::flow::resources::RetainedIndex;
    use nocter_model::ArrayRange;
    let fixture = ValueSourceFixture::new(
        "struct Item { value: i32 }\nfunc run(): i32 { let items = [Item { value: 40 }, Item { value: 2 }]\nlet index: usize = 0\nread selected = items[index]\nlet first = items[0]^\nfirst.value + items[1].value }",
    );
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    let root = unit
        .program()
        .resolve(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let instances = Arc::new(InstanceProgram::prepare(unit, root).unwrap());
    let typed = instances.body(instances.root()).unwrap().typed();
    let index = typed
        .body()
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            if matches!(
                expression.kind,
                nocter_checking::value_access::ExpressionKind::Index {
                    bounds: nocter_model::IndexBoundsCheck::Required,
                    ..
                }
            ) {
                RetainedIndex::for_selection(typed, id)
            } else {
                None
            }
        })
        .unwrap();
    let lifecycle = lifecycle::Program::prepare(instances).unwrap();
    let mut program = Program::lower(&lifecycle, RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    let size = program
        .runtime
        .types()
        .primitive(nocter_runtime_contract::RuntimePrimitive::Usize)
        .unwrap();
    let body = program.bodies.get_mut(&program.root).unwrap();
    let mut slots = nocter_model::ArenaBuilder::new();
    for (id, slot) in body.slots.iter() {
        assert_eq!(slots.insert(slot.clone()), id);
    }
    let snapshot = slots.insert(Slot {
        key: SlotKey::IndexSnapshot(index),
        storage: Storage::Payload,
        representations: Box::new([size]),
    });
    body.slots = slots.finish();
    let effect = body
        .actions
        .values_mut()
        .flat_map(|action| &mut action.alternatives)
        .flat_map(|alt| alt.before.iter_mut().chain(&mut alt.after))
        .find(|effect| matches!(effect, Effect::DisposeArrayElements { .. }))
        .unwrap();
    let Effect::DisposeArrayElements { range, .. } = effect else {
        unreachable!()
    };
    *range = ArrayRange::Through(snapshot);
    // A representation-only mutation verifies the consumer contract. It does not qualify
    // source dynamic transfer, snapshot initialization, dominance or executed native cleanup.
    validate::program(&program).unwrap();
    for wrong in [Storage::Address, Storage::Payload] {
        let mut bad = program.clone();
        let body = bad.bodies.get_mut(&bad.root).unwrap();
        body.slots = body.slots.map(|id, slot| {
            let mut slot = slot.clone();
            if id == snapshot {
                slot.storage = wrong;
                if wrong == Storage::Payload {
                    slot.key = SlotKey::Expression(index.operand());
                }
            }
            slot
        });
        assert!(matches!(
            validate::program(&bad),
            Err(Failure::InvalidBody(_))
        ));
    }
    let mut bad = program.clone();
    let signed = bad
        .runtime
        .types()
        .primitive(nocter_runtime_contract::RuntimePrimitive::Signed(32))
        .unwrap();
    let body = bad.bodies.get_mut(&bad.root).unwrap();
    body.slots = body.slots.map(|id, slot| {
        let mut slot = slot.clone();
        if id == snapshot {
            slot.representations = Box::new([signed]);
        }
        slot
    });
    assert!(matches!(
        validate::program(&bad),
        Err(Failure::InvalidBody(_))
    ));
}

#[test]
fn sparse_remainder_exclusions_must_be_unique_descendants_of_the_same_owner() {
    let source = nocter_test_support::RETAINED_ARRAY_SOURCES
        .iter()
        .find(|(name, _)| *name == "multiple-dynamic-element-fields")
        .unwrap()
        .1;
    let program = lower(source);
    for duplicate in [false, true] {
        let mut bad = program.clone();
        let effect = bad
            .bodies
            .values_mut()
            .flat_map(|body| body.actions.values_mut())
            .flat_map(|action| &mut action.alternatives)
            .flat_map(|alternative| alternative.before.iter_mut().chain(&mut alternative.after))
            .find(|effect| matches!(effect, Effect::DisposeArrayRemainder { .. }))
            .unwrap();
        let Effect::DisposeArrayRemainder { source, missing } = effect else {
            unreachable!()
        };
        missing[1] = if duplicate {
            missing[0].clone()
        } else {
            RemainderExclusion::Value(source.clone())
        };
        assert!(matches!(
            validate::program(&bad),
            Err(Failure::InvalidBody(_))
        ));
    }
}

#[test]
fn mixed_remainder_prefixes_require_their_own_frontier_role_and_bound() {
    let source = nocter_test_support::RETAINED_ARRAY_SOURCES
        .iter()
        .find(|(name, _)| *name == "mixed-prefix-element-fields")
        .unwrap()
        .1;
    let mut program = lower(source);
    let mut found = false;
    for body in program.bodies.values_mut() {
        let point = body
            .slots
            .iter()
            .find_map(|(id, slot)| matches!(slot.key, SlotKey::IndexSnapshot(_)).then_some(id));
        for effect in body
            .actions
            .values_mut()
            .flat_map(|action| &mut action.alternatives)
            .flat_map(|alternative| alternative.before.iter_mut().chain(&mut alternative.after))
        {
            let Effect::DisposeArrayRemainder { missing, .. } = effect else {
                continue;
            };
            for exclusion in missing {
                if let RemainderExclusion::ArrayPrefix { endpoint, .. } = exclusion {
                    assert!(matches!(body.slots.get(*endpoint).unwrap().key,
                        SlotKey::ArrayFrontier(frontier) if frontier.bound() == 3));
                    let point = point.unwrap();
                    assert!(matches!(body.slots.get(point).unwrap().key,
                        SlotKey::IndexSnapshot(index) if index.bound() != 3));
                    *endpoint = point;
                    found = true;
                }
            }
        }
        if found {
            break;
        }
    }
    assert!(found);
    assert!(matches!(
        validate::program(&program),
        Err(Failure::InvalidBody(_))
    ));
}
