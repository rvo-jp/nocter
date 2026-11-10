//! Actual private source lowering and integrity mutations; no substitute source interpreter.
use super::*;
use crate::{MirArithmeticCheck, MirBinaryOperation};
use nocter_checking::value_access::accepted::Unit;
use nocter_declaration_lowering::access_declarations;
use nocter_model::{MirLocalId, ValueInputOperation};
use nocter_target_program::value_access::InstanceProgram;
use nocter_test_support::ValueSourceFixture;
use std::sync::Arc;

fn lower(source: &str) -> Program {
    let fixture = ValueSourceFixture::new(source);
    let headers = access_declarations::Query::prepare(fixture.syntax()).unwrap();
    let unit = Arc::new(Unit::prepare(&headers.bind(fixture.syntax()).unwrap()).unwrap());
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
