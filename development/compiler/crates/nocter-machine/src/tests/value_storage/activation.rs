//! Physical storage effects consume actual accepted source; no source-value interpreter is used.
use super::*;
use crate::value_access::{self as machine, AddressBase, Effect};
use nocter_mir::value_access as mir;
use nocter_target_program::value_access::lifecycle;

fn lower(
    fixture: &ValueSourceFixture,
) -> (lifecycle::Program, mir::Program, machine::StorageProgram) {
    let lifecycle = lifecycle::Program::prepare(Arc::new(close(fixture))).unwrap();
    let mir = mir::Program::lower(&lifecycle, RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    let machine = machine::StorageProgram::lower(&mir).unwrap();
    (lifecycle, mir, machine)
}
fn effects(body: &machine::Body) -> impl Iterator<Item = &Effect> {
    body.actions()
        .values()
        .flat_map(|action| action.alternatives.iter())
        .flat_map(|alternative| alternative.before.iter().chain(alternative.after.iter()))
}
fn key(body: &mir::Body, slot: nocter_model::MirLocalId) -> mir::SlotKey {
    body.slots().get(slot).unwrap().key
}

#[test]
fn produced_and_transferred_access_inputs_bind_the_committed_temporary_backing() {
    for source in [
        "func touch(edit input: i32): void { input += 1 }\nfunc run(): void { touch(1 + 2) }",
        "func observe(input: error): void {}\nfunc run(take input: error): void { observe(input^) }",
    ] {
        let (lifecycle, mir, machine) = lower(&ValueSourceFixture::new(source));
        let root = lifecycle.instance_program().root();
        let source = &mir.bodies()[&root];
        let body = &machine.bodies()[&root];
        let action = body
            .actions()
            .values()
            .find(|a| !a.call_bindings.is_empty())
            .unwrap();
        let [
            Effect::BindAddress {
                destination,
                source: address,
            },
        ] = &*action.call_bindings
        else {
            panic!("one address channel");
        };
        assert!(matches!(
            key(source, address.slot),
            mir::SlotKey::Temporary { .. }
        ));
        assert_eq!(address.base, AddressBase::Frame);
        assert_ne!(*destination, address.offset);
        for alternative in &action.alternatives {
            assert!(alternative.before.iter().any(|effect| matches!(effect,
                Effect::CopyBytes { destination, bytes, .. } if destination.slot == address.slot && *bytes == address.size)));
        }
        assert!(effects(body).any(
            |effect| matches!(effect, Effect::Dispose { source, .. } if source.slot == address.slot)
        ));
    }
}

#[test]
fn partial_record_transfers_use_field_offsets_and_cleanup_surviving_bytes() {
    let (lifecycle, mir, machine) = lower(&ValueSourceFixture::new(
        "struct Pair { first: error\nsecond: error }\nfunc run(take pair: Pair, flag: bool): void {\nif flag { let saved = pair.first^ }\n}",
    ));
    let root = lifecycle.instance_program().root();
    let source = &mir.bodies()[&root];
    let body = &machine.bodies()[&root];
    let pair_local = source.signature().explicit()[0].local;
    let (pair, _) = source
        .slots()
        .iter()
        .find(|(_, slot)| slot.key == mir::SlotKey::Local(pair_local))
        .unwrap();
    let home = body.frame().slots().get(pair).unwrap();
    let transfer = effects(body)
        .find_map(|effect| match effect {
            Effect::CopyBytes {
                source,
                destination,
                bytes,
            } if source.slot == pair && source.size == 8 => Some((*source, *destination, *bytes)),
            _ => None,
        })
        .unwrap();
    assert_eq!(transfer.0.offset, home.offset);
    assert_eq!(transfer.2, 8);
    assert_ne!(transfer.0.slot, transfer.1.slot);
    assert!(transfer.1.offset >= home.offset + home.size);
    assert!(effects(body).any(|effect| matches!(effect, Effect::Dispose { source, disposal: mir::Disposal::Error } if source.slot == pair && source.offset == home.offset + 8)));
    assert!(
        effects(body).any(|effect| matches!(effect, Effect::End(source) if source.slot == pair))
    );
    let bank = body.frame().flags_offset().unwrap();
    assert!(bank >= home.offset + home.size);
}

#[test]
fn updated_adapter_carrier_is_disposed_before_receiving_the_protected_answer() {
    let (lifecycle, mir, machine) = lower(&ValueSourceFixture::new(
        "func reset(edit target: error, take replacement: error): i32 { target = replacement^\n42 }\nfunc apply(take action: once(take error): i32, take replacement: error): i32 { action^(replacement^) }\nfunc run(take input: error, take replacement: error): i32 {\nlet action = (input^; take next: error): i32 { reset(input, next^) }\napply(action^, replacement^)\n}",
    ));
    let mut found = false;
    for (&id, body) in machine.bodies() {
        let source = &mir.bodies()[&id];
        for action in body.actions().values() {
            for alternative in &action.alternatives {
                if let [
                    Effect::Dispose {
                        source: carrier,
                        disposal: mir::Disposal::Error,
                    },
                    Effect::CopyBytes {
                        source: answer,
                        destination,
                        ..
                    },
                ] = &*alternative.after
                {
                    let mir::SlotKey::Argument { call, index: 0 } = key(source, carrier.slot)
                    else {
                        continue;
                    };
                    assert_eq!(key(source, answer.slot), mir::SlotKey::Answer(call));
                    assert_eq!(
                        key(source, destination.slot),
                        mir::SlotKey::Expression(call)
                    );
                    assert_ne!(carrier.slot, answer.slot);
                    assert_eq!(answer.size, 4);
                    assert!(
                        answer.offset + answer.size <= carrier.offset
                            || carrier.offset + carrier.size <= answer.offset
                    );
                    assert_eq!(carrier.base, AddressBase::Frame);
                    assert_eq!(
                        carrier.offset,
                        body.frame().slots().get(carrier.slot).unwrap().offset
                    );
                    assert!(alternative.before.iter().any(|effect| matches!(effect, Effect::CopyBytes { destination, .. } if destination.slot == carrier.slot)));
                    found = true;
                }
            }
        }
    }
    assert!(found);
    assert!(lifecycle.instance_program().instances().len() > 1);
}

#[test]
fn replacement_through_an_edit_input_addresses_the_referent_and_disposes_before_copying() {
    let (lifecycle, _, machine) = lower(&ValueSourceFixture::new(
        "func replace(edit target: error, take replacement: error): void { target = replacement^ }\nfunc run(take input: error, take replacement: error): void { replace(input, replacement^) }",
    ));
    let mut found = false;
    for body in machine.bodies().values() {
        for action in body.actions().values() {
            for alternative in &action.alternatives {
                for effects in [&*alternative.before, &*alternative.after] {
                    for pair in effects.windows(2) {
                        if let [
                            Effect::Dispose {
                                source,
                                disposal: mir::Disposal::Error,
                            },
                            Effect::CopyBytes { destination, .. },
                        ] = pair
                            && matches!(source.base, AddressBase::Referent { .. })
                        {
                            assert_eq!(source, destination);
                            assert_eq!(source.offset, 0);
                            found = true;
                        }
                    }
                }
            }
        }
    }
    assert!(found);
    assert_eq!(machine.bodies().len(), lifecycle.bodies().len());
}

#[test]
fn zero_byte_payloads_share_an_anchor_without_merging_slots() {
    let (lifecycle, mir, machine) = lower(&ValueSourceFixture::new(
        "struct Empty {}\nfunc inspect(input: Empty): void {}\nfunc run(take first: Empty, take second: Empty): void { inspect(first)\ninspect(second) }",
    ));
    let root = lifecycle.instance_program().root();
    let source = &mir.bodies()[&root];
    let frame = machine.bodies()[&root].frame();
    let inputs: Vec<_> = source
        .signature()
        .explicit()
        .iter()
        .map(|input| {
            source
                .slots()
                .iter()
                .find(|(_, slot)| slot.key == mir::SlotKey::Local(input.local))
                .unwrap()
                .0
        })
        .collect();
    assert_ne!(inputs[0], inputs[1]);
    for &input in &inputs {
        let slot = frame.slots().get(input).unwrap();
        assert_eq!((slot.offset, slot.size), (0, 0));
    }
    assert!(frame.size() > 0);
    assert_eq!(frame.size() % frame.alignment(), 0);
}

#[test]
fn equal_zero_byte_field_offsets_keep_distinct_projection_identities() {
    let (lifecycle, mir, machine) = lower(&ValueSourceFixture::new(
        "struct Pair { first: void\nsecond: void }\nfunc run(take pair: Pair): void { let first = pair.first^\nlet second = pair.second^ }",
    ));
    let root = lifecycle.instance_program().root();
    let source = &mir.bodies()[&root];
    let pair = source.signature().explicit()[0].local;
    let mut fields = effects(&machine.bodies()[&root])
        .filter_map(|effect| match effect {
            Effect::CopyBytes {
                source: address,
                bytes: 0,
                ..
            } if key(source, address.slot) == mir::SlotKey::Local(pair) => Some(*address),
            _ => None,
        })
        .collect::<Vec<_>>();
    fields.dedup();
    assert_eq!(fields.len(), 2);
    assert_eq!(
        (fields[0].slot, fields[0].offset, fields[0].data),
        (fields[1].slot, fields[1].offset, fields[1].data)
    );
    assert_ne!(fields[0].identity, fields[1].identity);
}

#[test]
fn forwarding_a_scoped_input_keeps_the_referent_address_instead_of_the_pointer_slot() {
    let (_, _, machine) = lower(&ValueSourceFixture::new(
        "func observe(input: error): void {}\nfunc forward(input: error): void { observe(input) }\nfunc run(take input: error): void { forward(input) }",
    ));
    assert!(
        machine
            .bodies()
            .values()
            .any(|body| body.actions().values().any(|action| {
                action.call_bindings.iter().any(|effect| {
                    matches!(effect, Effect::BindAddress { source, .. }
            if matches!(source.base, AddressBase::Referent { .. }) && source.offset == 0)
                })
            }))
    );
}

#[test]
fn unproduced_dead_contracts_do_not_allocate_payload_or_address_slots() {
    let (lifecycle, mir, machine) = lower(&ValueSourceFixture::new(
        "func forward(take action: func(i32): i32): func(i32): i32 { action^ }\nfunc run(): i32 {\nlet absent = forward(if true { return 1 } else { return 2 })\nlet adapted: once(i32): i32 = absent\nadapted(1)\n}",
    ));
    let root = lifecycle.instance_program().root();
    let instance = lifecycle.instance_program().instances().get(root).unwrap();
    let absent = instance
        .data()
        .iter()
        .filter_map(|(&data, evidence)| evidence.payload().is_none().then_some(data))
        .collect::<Vec<_>>();
    assert!(!absent.is_empty());
    assert!(mir.bodies()[&root].slots().iter().all(|(_, slot)| {
        slot.representations
            .iter()
            .all(|data| !absent.contains(data))
    }));
    assert!(
        machine.bodies()[&root]
            .actions()
            .values()
            .all(|action| action.call_bindings.is_empty())
    );
}

#[test]
fn source_families_keep_phase_order_and_frame_relative_flag_recipes() {
    for case in [
        "a20-record-branch-restore.nct",
        "a21-record-loop-restore.nct",
        "a50-read-computed-field.nct",
        "a51-take-computed-field.nct",
        "a52-computed-field-replacement.nct",
        "a65-function-take-input.nct",
        "a82-callback-forward-read.nct",
        "a84-callback-forward-adapter.nct",
        "a92-callback-taking-error.nct",
        "a124-dependent-result-relay.nct",
        "a125-dependent-result-recursion.nct",
        "a136-dependent-result-mut.nct",
        "a138-owned-capture-read.nct",
        "a140-owned-capture-take.nct",
        "a141-owned-capture-partial.nct",
        "a147-owned-capture-input-operations.nct",
        "a148-edit-input-computed-field.nct",
    ] {
        let (lifecycle, mir, machine) = lower(&ValueSourceFixture::corpus(case));
        for (&id, body) in machine.bodies() {
            let target = &lifecycle.bodies()[&id];
            assert_eq!(body.frame().initial_flags(), target.flags().initial_words());
            assert_eq!(body.actions().len(), target.actions().len());
            for (&at, action) in body.actions() {
                let original = &target.actions()[&nocter_checking::value_access::flow::Location {
                    point: at.point,
                    action: at.action,
                }];
                for (physical, original) in action.alternatives.iter().zip(original.alternatives())
                {
                    for (values, expected) in [
                        (&*physical.tests, original.flags().tests()),
                        (&*physical.before_writes, original.flags().before_writes()),
                        (&*physical.after_writes, original.flags().after_writes()),
                    ] {
                        assert_eq!(values.len(), expected.len());
                        for (value, expected) in values.iter().zip(expected) {
                            assert_eq!(
                                value.offset,
                                body.frame().flags_offset().unwrap()
                                    + u64::try_from(expected.word()).unwrap() * 8
                            );
                            assert_eq!(
                                (value.mask, value.value),
                                (expected.mask(), expected.value())
                            );
                        }
                    }
                }
            }
            assert_eq!(body.frame().slots().len(), mir.bodies()[&id].slots().len());
        }
    }
}

#[test]
fn frame_composition_checks_overflow_after_individually_valid_stored_layouts() {
    let mut source = String::from("struct Leaf { value: i32 }\n");
    let mut previous = String::from("Leaf");
    for depth in 0..61 {
        let next = format!("Layer{depth}");
        writeln!(
            source,
            "struct {next} {{ first: {previous}\nsecond: {previous} }}"
        )
        .unwrap();
        previous = next;
    }
    write!(
        source,
        "func run(take input: {previous}): {previous} {{ input^ }}"
    )
    .unwrap();
    let lifecycle =
        lifecycle::Program::prepare(Arc::new(close(&ValueSourceFixture::new(&source)))).unwrap();
    let mir = mir::Program::lower(&lifecycle, RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
    assert!(matches!(
        machine::StorageProgram::lower(&mir),
        Err(machine::Failure::FrameOverflow(_))
    ));
}
