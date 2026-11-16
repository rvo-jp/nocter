//! Actual private source lowering and integrity mutations; no substitute source interpreter.
use super::*;
use crate::{MirArithmeticCheck, MirBinaryOperation};
use nocter_checking::value_access::accepted::SourceUnit;
use nocter_model::{MirLocalId, ValueInputOperation};
use nocter_runtime_contract::RuntimeType;
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
fn tuple_repacking_retains_the_original_receipt_and_all_disjoint_siblings() {
    let program = lower(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-growth.nct"
    )));
    validate::program(&program).unwrap();
    let effect = program
        .bodies
        .values()
        .flat_map(|body| body.actions.values())
        .flat_map(|action| &action.alternatives)
        .flat_map(|alternative| alternative.before.iter().chain(&alternative.after))
        .find(|effect| matches!(effect, Effect::Repack { .. }))
        .unwrap();
    let Effect::Repack {
        before,
        destination,
        preserved,
        ..
    } = effect
    else {
        unreachable!()
    };
    assert_ne!(*before, destination.root);
    assert_eq!(preserved.len(), 1);
    assert_eq!(preserved[0].source.root, *before);
    assert_eq!(preserved[0].destination.root, destination.root);
    assert_eq!(
        preserved[0].source.projections.as_ref(),
        &[Projection::Field(nocter_model::ValueField::Tuple(1))]
    );
    for corruption in 0..4 {
        let mut changed = program.clone();
        let effect = changed
            .bodies
            .values_mut()
            .flat_map(|body| body.actions.values_mut())
            .flat_map(|action| &mut action.alternatives)
            .flat_map(|alternative| alternative.before.iter_mut().chain(&mut alternative.after))
            .find(|effect| matches!(effect, Effect::Repack { .. }))
            .unwrap();
        let Effect::Repack {
            source,
            destination,
            before,
            preserved,
            ..
        } = effect
        else {
            unreachable!()
        };
        match corruption {
            0 => *preserved = Box::new([]),
            1 => preserved[0].destination.root = *before,
            2 => source.slot = destination.slot,
            3 => *before = RuntimeDataId::Instance(u32::MAX),
            _ => unreachable!(),
        }
        assert!(
            validate::program(&changed).is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn indexed_repacking_uses_only_the_closed_singleton_coordinate() {
    let program = lower(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-growth.nct"
    )));
    let mut found = 0;
    for effect in program
        .bodies
        .values()
        .flat_map(|body| body.actions.values())
        .flat_map(|action| &action.alternatives)
        .flat_map(|alternative| alternative.before.iter().chain(&alternative.after))
    {
        if let Effect::Repack {
            destination,
            preserved,
            ..
        } = effect
        {
            assert_eq!(
                destination.projections.as_ref(),
                &[
                    Projection::Field(nocter_model::ValueField::Tuple(0)),
                    Projection::Field(nocter_model::ValueField::Array(0))
                ]
            );
            assert_eq!(preserved.len(), 1);
            assert_eq!(
                preserved[0].source.projections.as_ref(),
                &[Projection::Field(nocter_model::ValueField::Tuple(1))]
            );
            found += 1;
        }
    }
    assert!(
        found >= 2,
        "both growth and shrinkage need original receipts"
    );
}

#[test]
fn dynamic_hole_paths_require_an_original_saved_number_with_the_matching_array_bound() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-dynamic-hole-growth.nct"
    ));
    let source = source.replace("    var pair =", "    var other = [1, 2, 3]\n    let temporary = other[index]^\n    other[index] = temporary^\n    var pair =");
    let program = lower(&source);
    validate::program(&program).unwrap();
    let foreign = program
        .bodies
        .values()
        .find_map(|body| {
            body.slots.iter().find_map(|(id, slot)| {
                matches!(slot.key, SlotKey::IndexSnapshot(index) if index.bound() == 3)
                    .then_some(id)
            })
        })
        .unwrap();
    for invalid in 0..3 {
        let mut changed = program.clone();
        let effect = changed.bodies.values_mut().flat_map(|body| body.actions.values_mut())
            .flat_map(|action| &mut action.alternatives)
            .flat_map(|alternative| alternative.before.iter_mut().chain(&mut alternative.after))
            .find(|effect| matches!(effect, Effect::Repack { missing, .. } if missing.iter().any(|path| path.place().projections.iter().any(|part| matches!(part, Projection::Index(_)))))).unwrap();
        let Effect::Repack {
            missing,
            destination,
            ..
        } = effect
        else {
            unreachable!()
        };
        let index = missing
            .iter_mut()
            .filter_map(|hole| match hole {
                RemainderExclusion::Value(place) => Some(place.projections.iter_mut()),
                RemainderExclusion::ArrayPrefix { .. } => None,
            })
            .flatten()
            .find(|part| matches!(part, Projection::Index(_)))
            .unwrap();
        *index = match invalid {
            0 => Projection::Field(nocter_model::ValueField::Array(2)),
            1 => Projection::Index(destination.slot),
            2 => Projection::Index(foreign),
            _ => unreachable!(),
        };
        assert!(validate::program(&changed).is_err());
    }
}

#[test]
fn partial_array_preservation_cannot_become_a_complete_load() {
    let program = lower(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-array-hole-growth.nct"
    )));
    validate::program(&program).unwrap();
    let mut changed = program.clone();
    let part = changed
        .bodies
        .values_mut()
        .flat_map(|body| body.actions.values_mut())
        .flat_map(|action| &mut action.alternatives)
        .flat_map(|alternative| alternative.before.iter_mut().chain(&mut alternative.after))
        .find_map(|effect| match effect {
            Effect::Repack { preserved, .. } => preserved
                .iter_mut()
                .find(|part| part.kind == Preservation::ArrayRemainder),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        part.source.projections.as_ref(),
        &[Projection::Field(nocter_model::ValueField::Tuple(1))]
    );
    part.kind = Preservation::Complete;
    assert!(validate::program(&changed).is_err());
}

#[test]
fn conditional_repacking_keeps_separate_original_guards_for_different_missing_subtrees() {
    let program = lower(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-conditional-hole-growth.nct"
    )));
    let mut found = 0;
    for action in program
        .bodies
        .values()
        .flat_map(|body| body.actions.values())
    {
        let receipts = action
            .alternatives
            .iter()
            .filter_map(|alternative| {
                alternative.after.iter().find_map(|effect| match effect {
                    Effect::Repack { missing, .. } => Some((alternative, missing.len())),
                    _ => None,
                })
            })
            .collect::<Vec<_>>();
        let sizes = receipts
            .iter()
            .map(|(_, size)| *size)
            .collect::<std::collections::BTreeSet<_>>();
        if sizes != std::collections::BTreeSet::from([0, 1]) {
            continue;
        }
        for (alternative, _) in receipts {
            assert!(!alternative.tests.is_empty());
        }
        found += 1;
    }
    assert!(found >= 2);
}

#[test]
fn singleton_hole_repacking_keeps_the_initialized_element_field_and_skips_its_parent() {
    let program = lower(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-singleton-hole-growth.nct"
    )));
    let field = |index| Projection::Field(nocter_model::ValueField::Tuple(index));
    let element = Projection::Field(nocter_model::ValueField::Array(0));
    let mut found = 0;
    for effect in program
        .bodies
        .values()
        .flat_map(|body| body.actions.values())
        .flat_map(|action| &action.alternatives)
        .flat_map(|alternative| alternative.before.iter().chain(&alternative.after))
    {
        if let Effect::Repack {
            missing, preserved, ..
        } = effect
        {
            if missing.is_empty() {
                continue;
            }
            assert_eq!(
                missing
                    .iter()
                    .map(|hole| hole.place().projections.as_ref())
                    .collect::<Vec<_>>(),
                vec![&[field(1), element, field(0)][..]]
            );
            let paths = preserved
                .iter()
                .map(|part| part.source.projections.clone())
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                paths,
                std::collections::BTreeSet::from([
                    Box::from([field(1), element, field(1)]),
                    Box::from([field(2)])
                ])
            );
            found += 1;
        }
    }
    assert!(found >= 2);
}

#[test]
fn sparse_tuple_repacking_never_snapshots_a_missing_subtree_or_its_parent() {
    let program = lower(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-growth-holes.nct"
    )));
    let effect = program
        .bodies
        .values()
        .flat_map(|body| body.actions.values())
        .flat_map(|action| &action.alternatives)
        .flat_map(|alternative| alternative.before.iter().chain(&alternative.after))
        .find(|effect| matches!(effect, Effect::Repack { missing, .. } if !missing.is_empty()))
        .unwrap();
    let Effect::Repack {
        missing, preserved, ..
    } = effect
    else {
        unreachable!()
    };
    let field = |index| Projection::Field(nocter_model::ValueField::Tuple(index));
    assert_eq!(
        missing
            .iter()
            .map(|hole| hole.place().projections.as_ref())
            .collect::<Vec<_>>(),
        vec![&[field(1), field(0)][..]]
    );
    let paths = preserved
        .iter()
        .map(|part| part.source.projections.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        paths,
        std::collections::BTreeSet::from([Box::from([field(1), field(1)]), Box::from([field(2)])])
    );
    for corruption in 0..6 {
        let mut changed = program.clone();
        let effect = changed
            .bodies
            .values_mut()
            .flat_map(|body| body.actions.values_mut())
            .flat_map(|action| &mut action.alternatives)
            .flat_map(|alternative| alternative.before.iter_mut().chain(&mut alternative.after))
            .find(|effect| matches!(effect, Effect::Repack { missing, .. } if !missing.is_empty()))
            .unwrap();
        let Effect::Repack { missing, .. } = effect else {
            unreachable!()
        };
        let corrupt = |path: Box<[Projection]>| {
            let mut place = missing[0].place().clone();
            place.projections = path;
            RemainderExclusion::Value(place)
        };
        match corruption {
            0 => *missing = Box::new([]),
            1 => *missing = vec![missing[0].clone(), missing[0].clone()].into(),
            2 => *missing = vec![corrupt(Box::from([field(1)])), missing[0].clone()].into(),
            3 => *missing = vec![corrupt(Box::from([field(0)]))].into(),
            4 => *missing = vec![corrupt(Box::from([field(99)]))].into(),
            5 => {
                *missing = vec![corrupt(Box::from([Projection::Field(
                    nocter_model::ValueField::Array(0),
                )]))]
                .into();
            }
            _ => unreachable!(),
        }
        assert!(
            validate::program(&changed).is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn guarded_transport_is_neither_exact_copy_narrowing_nor_overlapping_reinterpretation() {
    let program = lower(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/widened-outcome-results.nct"
    )));
    validate::program(&program).unwrap();
    let mut copied = program.clone();
    let effect = transport_effect(transport_body(&mut copied));
    let Effect::Transport {
        source,
        destination,
    } = effect
    else {
        unreachable!()
    };
    *effect = Effect::Transfer {
        source: source.clone(),
        destination: destination.clone(),
    };
    assert!(validate::program(&copied).is_err());
    let mut narrowed = program.clone();
    let Effect::Transport {
        source,
        destination,
    } = transport_effect(transport_body(&mut narrowed))
    else {
        unreachable!()
    };
    std::mem::swap(source, destination);
    assert!(validate::program(&narrowed).is_err());
    let mut overlapping = program;
    let body = transport_body(&mut overlapping);
    let Effect::Transport {
        source,
        destination,
    } = transport_effect(body)
    else {
        unreachable!()
    };
    let source = source.clone();
    let target = destination.root;
    body.slots = body.slots.map(|id, slot| {
        let mut slot = slot.clone();
        if id == source.slot {
            slot.representations = slot
                .representations
                .iter()
                .copied()
                .chain([target])
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
        }
        slot
    });
    let Effect::Transport { destination, .. } = transport_effect(body) else {
        unreachable!()
    };
    destination.slot = source.slot;
    assert!(validate::program(&overlapping).is_err());
}

fn transport_body(program: &mut Program) -> &mut Body {
    program
        .bodies
        .values_mut()
        .find(|body| {
            body.actions
                .values()
                .flat_map(|action| &action.alternatives)
                .flat_map(|alternative| alternative.before.iter().chain(&alternative.after))
                .any(|effect| matches!(effect, Effect::Transport { .. }))
        })
        .unwrap()
}
fn transport_effect(body: &mut Body) -> &mut Effect {
    body.actions
        .values_mut()
        .flat_map(|action| &mut action.alternatives)
        .flat_map(|alternative| alternative.before.iter_mut().chain(&mut alternative.after))
        .find(|effect| matches!(effect, Effect::Transport { .. }))
        .unwrap()
}

#[test]
fn excluded_outcome_alternatives_cannot_be_constructed_or_registered_as_stored_values() {
    let program = lower(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/producerless-outcome-results.nct"
    )));
    let empty = program
        .runtime
        .types()
        .iter()
        .find_map(|(id, kind)| matches!(kind, RuntimeType::Uninhabited).then_some(id))
        .unwrap();
    let optional = program
        .runtime
        .types()
        .iter()
        .find_map(|(id, kind)| {
            matches!(kind, RuntimeType::Optional(payload) if *payload == empty).then_some(id)
        })
        .unwrap();
    validate::program(&program).unwrap();
    let mut bad_tag = program.clone();
    let primary = bad_tag
        .bodies
        .values_mut()
        .flat_map(|body| body.actions.values_mut())
        .flat_map(|action| &mut action.alternatives)
        .flat_map(|alternative| alternative.before.iter_mut().chain(&mut alternative.after))
        .find_map(|effect| match effect {
            Effect::VariantTag {
                destination,
                primary,
            } if destination.data == optional => Some(primary),
            _ => None,
        })
        .unwrap();
    assert!(!*primary);
    *primary = true;
    assert!(validate::program(&bad_tag).is_err());
    let mut bad_slot = program;
    let (body, id) = bad_slot
        .bodies
        .iter()
        .find_map(|(&body, value)| {
            value.slots.iter().find_map(|(id, slot)| {
                matches!(slot.key, SlotKey::Expression(_)).then_some((body, id))
            })
        })
        .unwrap();
    let body = bad_slot.bodies.get_mut(&body).unwrap();
    body.slots = body.slots.map(|selected, slot| {
        let mut slot = slot.clone();
        if selected == id {
            slot.representations = slot
                .representations
                .iter()
                .copied()
                .chain([empty])
                .collect();
        }
        slot
    });
    assert!(validate::program(&bad_slot).is_err());
}

#[test]
fn compound_input_transport_cannot_substitute_another_callable_context() {
    let source = lower(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-compound-inputs.nct"
    )));
    let candidates =
        source
            .bodies
            .values()
            .flat_map(operations)
            .filter_map(|operation| {
                let Operation::Call(call) = operation else {
                    return None;
                };
                let [input] = call.inputs.as_ref() else {
                    return None;
                };
                let Some(nocter_runtime_contract::RuntimeTypeRepresentation::ValueAggregate {
                    fields,
                }) = source.runtime.type_representations().get(input.source.data)
                else {
                    return None;
                };
                (input.operation == ValueInputOperation::Take
                    && fields.len() == 2
                    && matches!(
                        source.runtime.types().get(fields[0].ty()),
                        Some(RuntimeType::StaticCallable { .. })
                    ))
                .then_some((call.target, input.source.data))
            })
            .collect::<Vec<_>>();
    let &(first, data) = candidates
        .iter()
        .find(|(target, _)| {
            candidates
                .iter()
                .filter(|(other, _)| target == other)
                .count()
                >= 2
        })
        .unwrap();
    let &(other, _) = candidates
        .iter()
        .find(|(target, other)| *target != first && *other != data)
        .unwrap();
    let mut changed = source.clone();
    let call = changed
        .bodies
        .values_mut()
        .flat_map(|body| body.actions.values_mut())
        .flat_map(|action| &mut action.operations)
        .find_map(|operation| match operation {
            Operation::Call(call) if call.target == first => Some(call),
            _ => None,
        })
        .unwrap();
    call.target = other;
    // Another unchanged edge still reaches the original instance: rejection must not depend
    // on turning that body unreachable when the one call is corrupted.
    assert!(
        changed
            .bodies
            .values()
            .flat_map(operations)
            .any(|operation| matches!(operation, Operation::Call(call) if call.target == first))
    );
    assert!(matches!(
        validate::program(&changed),
        Err(Failure::InvalidBody(_))
    ));
}

#[test]
fn scoped_geometry_keeps_the_reference_and_its_larger_container_at_the_original_action() {
    let program = lower(
        "struct Pair { first: i32\nsecond: i32 }\nfunc run(): i32 { var bundle = (Pair { first: 41, second: 1 }, 0)\nedit pair = bundle.0\npair.first = 42\nbundle.0.first }",
    );
    let body = &program.bodies[&program.root];
    assert_eq!(body.geometry().len(), 2);
    assert_eq!(body.geometry()[0].at, body.geometry()[1].at);
    assert_ne!(body.geometry()[0].before, body.geometry()[1].before);
    for pair in body.geometry() {
        assert_eq!(pair.before, pair.after);
        let action = &body.actions()[&pair.at];
        assert!(action.alternatives.iter().any(|alternative| {
            alternative
                .after
                .iter()
                .chain(&alternative.before)
                .any(|effect| matches!(effect, Effect::Transfer { .. }))
        }));
    }
    for corrupt_site in [false, true] {
        let mut changed = program.clone();
        let pair = &mut changed.bodies.get_mut(&changed.root).unwrap().geometry[0];
        if corrupt_site {
            pair.at.action = usize::MAX;
        } else {
            pair.before = RuntimeDataId::Instance(u32::MAX);
        }
        assert!(matches!(
            validate::program(&changed),
            Err(Failure::InvalidBody(_))
        ));
    }
}

#[test]
fn adapter_cleanup_pairs_its_completed_environment_address_with_the_full_carrier_geometry() {
    let program = lower(
        "struct Item { value: i32 }\nfunc consume(take action: once(): i32): i32 { action^() }\nfunc run(): i32 { let item = Item { value: 42 }\nlet action = (item^;): i32 { item.value }\nconsume(action^) }",
    );
    let mut cleanups = 0;
    for body in program.bodies.values() {
        for (&at, action) in body.actions() {
            for alternative in &action.alternatives {
                for effect in &alternative.after {
                    let Effect::Dispose { source, .. } = effect else {
                        continue;
                    };
                    if source.projections.as_ref() != [Projection::Environment] {
                        continue;
                    }
                    let RuntimeType::StaticCallable {
                        environment: Some(environment),
                    } = program.runtime.types().get(source.root).unwrap()
                    else {
                        panic!("captured carrier")
                    };
                    assert_eq!(source.data, *environment);
                    assert!(
                        body.geometry()
                            .iter()
                            .any(|pair| pair.at == at && pair.after == source.root)
                    );
                    cleanups += 1;
                }
            }
        }
    }
    assert!(cleanups > 0);
}

#[test]
fn static_primitive_calls_keep_closed_roles_without_fake_mir_bodies() {
    use nocter_compile_input::{ModuleIdentity, PrimitiveRoleLocator, ToolchainInput};
    use nocter_declaration_lowering::access_declarations as declarations;
    use nocter_model::PackageIdentity;
    use nocter_runtime_contract::PrimitiveRole;
    let package = PackageIdentity::new("test:standard");
    let module = ModuleIdentity::new(package.clone(), ["error"]);
    let toolchain = ToolchainInput::new(
        package.clone(),
        ModuleIdentity::new(package, ["prelude"]),
        Vec::new(),
        Vec::new(),
    )
    .with_primitive_roles(vec![PrimitiveRoleLocator::new(
        PrimitiveRole::NewError,
        module.clone(),
        "create_failure",
    )]);
    let authority = declarations::PrimitiveAuthority::for_module(&toolchain, &module).unwrap();
    let fixture = ValueSourceFixture::new(
        "primitive func create_failure(code: str, message: str): error\nfunc run(): void { let failure = create_failure(\"test\", \"message\")\ndrop failure }",
    );
    let headers =
        declarations::Query::prepare_with_primitive_authority(fixture.syntax(), &authority)
            .unwrap();
    let unit = Arc::new(
        nocter_checking::value_access::accepted::Unit::prepare(
            &headers.bind(fixture.syntax()).unwrap(),
        )
        .unwrap(),
    );
    let root = unit
        .program()
        .resolve_code(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let instances = Arc::new(InstanceProgram::prepare(unit, root).unwrap());
    assert_eq!(instances.instances().len(), 1);
    let lifecycle = lifecycle::Program::prepare(instances).unwrap();
    let program = Program::lower(&lifecycle, RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    assert_eq!(program.bodies.len(), 1);
    assert!(operations(&program.bodies[&program.root]).any(|operation| matches!(operation, Operation::Call(call) if call.target == CallTarget::Primitive(PrimitiveRole::NewError))));
    for role in [PrimitiveRole::ErrorContext, PrimitiveRole::ErrorCode] {
        let mut invalid = program.clone();
        for operation in invalid
            .bodies
            .get_mut(&invalid.root)
            .unwrap()
            .actions
            .values_mut()
            .flat_map(|action| action.operations.iter_mut())
        {
            if let Operation::Call(call) = operation {
                call.target = CallTarget::Primitive(role);
            }
        }
        assert!(matches!(
            validate::program(&invalid),
            Err(Failure::InvalidBody(_))
        ));
    }
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
                let callee = &program.bodies[&call.target.instance().unwrap()];
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
fn conditional_constant_point_restoration_keeps_old_error_cleanup_guarded_without_snapshots() {
    let program = lower(
        "func run(take first: error, take second: error, take next: error, flag: bool): void { var items = [first^, second^]\nvar index: usize = 1\nif flag { let old = items[index]^ }\nitems[index] = next^ }",
    );
    let body = &program.bodies[&program.root];
    assert!(
        !body
            .slots
            .iter()
            .any(|(_, slot)| matches!(slot.key, SlotKey::IndexSnapshot(_)))
    );
    let selected = |place: &Place| {
        place.projections.as_ref() == [Projection::Field(nocter_model::ValueField::Array(1))]
    };
    let action = body
        .actions
        .values()
        .find(|action| {
            action.alternatives.len() > 1
                && action.alternatives.iter().all(|alternative| {
                    alternative
                        .before
                        .iter()
                        .chain(&alternative.after)
                        .any(|effect| {
                            matches!(effect,
                Effect::Transfer { destination, .. } if selected(destination))
                        })
                })
        })
        .unwrap();
    let mut counts = std::collections::BTreeSet::new();
    for alternative in &action.alternatives {
        assert!(!alternative.tests.is_empty());
        counts.insert(
            alternative
                .before
                .iter()
                .chain(&alternative.after)
                .filter(|effect| {
                    matches!(effect,
            Effect::Dispose { source, disposal: Disposal::Error } if selected(source))
                })
                .count(),
        );
    }
    assert_eq!(counts, std::collections::BTreeSet::from([0, 1]));
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

#[test]
fn prefix_repacking_retains_the_original_frontier_and_rejects_invalid_exclusions() {
    let program = lower(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-prefix-hole-growth.nct"
    )));
    validate::program(&program).unwrap();
    for corruption in 0..5 {
        let mut changed = program.clone();
        let effect = changed
            .bodies
            .values_mut()
            .flat_map(|body| body.actions.values_mut())
            .flat_map(|action| &mut action.alternatives)
            .flat_map(|alternative| alternative.before.iter_mut().chain(&mut alternative.after))
            .find(|effect| {
                matches!(effect, Effect::Repack { missing, .. }
                if missing.iter().any(|hole| hole.endpoint().is_some()))
            })
            .unwrap();
        let Effect::Repack {
            missing,
            destination,
            preserved,
            ..
        } = effect
        else {
            unreachable!()
        };
        assert!(
            preserved
                .iter()
                .any(|part| part.kind == Preservation::ArrayRemainder)
        );
        match corruption {
            0 => *missing = Box::new([]),
            1 => *missing = vec![missing[0].clone(), missing[0].clone()].into(),
            2 => {
                let RemainderExclusion::ArrayPrefix { endpoint, .. } = &mut missing[0] else {
                    unreachable!()
                };
                *endpoint = destination.slot;
            }
            3 => {
                let RemainderExclusion::ArrayPrefix { array, .. } = &mut missing[0] else {
                    unreachable!()
                };
                array.root = destination.root;
            }
            4 => {
                preserved
                    .iter_mut()
                    .find(|part| part.kind == Preservation::ArrayRemainder)
                    .unwrap()
                    .kind = Preservation::Complete;
            }
            _ => unreachable!(),
        }
        assert!(
            validate::program(&changed).is_err(),
            "corruption {corruption}"
        );
    }
}
