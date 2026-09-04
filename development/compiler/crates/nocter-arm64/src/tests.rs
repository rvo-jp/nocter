use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64DataRegister, Arm64DataSize, Arm64EncodingError, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MoveWide, Arm64Register, Arm64Shift, Arm64SystemRegister,
};

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).unwrap()
}

fn data(number: u8) -> Arm64DataRegister {
    Arm64DataRegister::General(x(number))
}

fn base(number: u8) -> Arm64BaseRegister {
    Arm64BaseRegister::General(x(number))
}

fn destination(number: u8) -> Arm64AddSubtractDestination {
    Arm64AddSubtractDestination::General(x(number))
}

fn word(instruction: Arm64Instruction) -> u32 {
    u32::from_le_bytes(instruction.encode().unwrap())
}

#[test]
fn encodes_integer_arithmetic_without_untyped_register_31() {
    assert_eq!(
        word(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: destination(0),
            source: base(1),
            immediate: 42,
            shift_12: false,
        }),
        0x9100_a820
    );
    assert_eq!(
        word(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Subtract,
            set_flags: false,
            destination: Arm64AddSubtractDestination::StackPointer,
            source: Arm64BaseRegister::StackPointer,
            immediate: 32,
            shift_12: false,
        }),
        0xd100_83ff
    );
    assert_eq!(
        word(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Subtract,
            set_flags: true,
            destination: Arm64AddSubtractDestination::Zero,
            source: base(2),
            immediate: 7,
            shift_12: false,
        }),
        0xf100_1c5f
    );
    assert_eq!(
        word(Arm64Instruction::AddSubtractRegister {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: data(0),
            left: data(1),
            right: data(2),
        }),
        0x8b02_0020
    );
}

#[test]
fn encodes_closed_monotonic_counter_register_reads() {
    assert_eq!(
        word(Arm64Instruction::InstructionSynchronizationBarrier),
        0xd503_3fdf
    );
    assert_eq!(
        word(Arm64Instruction::ReadSystemRegister {
            destination: x(0),
            register: Arm64SystemRegister::CounterVirtual,
        }),
        0xd53b_e040
    );
    assert_eq!(
        word(Arm64Instruction::ReadSystemRegister {
            destination: x(3),
            register: Arm64SystemRegister::CounterFrequency,
        }),
        0xd53b_e003
    );
}

#[test]
fn monotonic_counter_observation_materializes_an_ordering_barrier() {
    let machine = crate::test_support::lower_machine_with_standard_uses(
        "use std/time\n\n\
         func main(): i32 {\n\
             let _ = time.monotonic_counter_for_test()\n\
             return 0\n\
         }\n",
        &[&["time"]],
    );
    let program = crate::Arm64Program::lower_machine(&machine).unwrap();
    let words = program
        .text()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();
    let counter_read = 0xd53b_e040;

    assert_eq!(
        words.iter().filter(|word| **word == counter_read).count(),
        1
    );
    assert!(
        words
            .windows(2)
            .any(|pair| { pair == [0xd503_3fdf, counter_read,] })
    );
}

#[test]
fn encodes_page_relative_addresses_without_truncation() {
    assert_eq!(
        word(Arm64Instruction::AddressPage {
            destination: x(3),
            displacement: 0,
        }),
        0x9000_0003
    );
    assert_eq!(
        Arm64Instruction::AddressPage {
            destination: x(0),
            displacement: 1,
        }
        .encode(),
        Err(Arm64EncodingError::MisalignedPageAddress)
    );
    assert_eq!(
        Arm64Instruction::AddressPage {
            destination: x(0),
            displacement: 1_i64 << 32,
        }
        .encode(),
        Err(Arm64EncodingError::PageAddressOutOfRange)
    );
}

#[test]
fn encodes_stack_pointer_extended_arithmetic() {
    assert_eq!(
        word(Arm64Instruction::AddSubtractExtendedRegister {
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: destination(16),
            left: Arm64BaseRegister::StackPointer,
            right: x(16),
            shift: 0,
        }),
        0x8b30_63f0
    );
    assert_eq!(
        word(Arm64Instruction::AddSubtractExtendedRegister {
            operation: Arm64AddSubtract::Subtract,
            set_flags: false,
            destination: Arm64AddSubtractDestination::StackPointer,
            left: Arm64BaseRegister::StackPointer,
            right: x(16),
            shift: 0,
        }),
        0xcb30_63ff
    );
    assert_eq!(
        Arm64Instruction::AddSubtractExtendedRegister {
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: destination(0),
            left: base(1),
            right: x(2),
            shift: 5,
        }
        .encode(),
        Err(Arm64EncodingError::InvalidShift)
    );
}

#[test]
fn encodes_multiply_divide_shift_and_wide_immediates() {
    assert_eq!(
        word(Arm64Instruction::MoveWide {
            size: Arm64DataSize::Bits64,
            operation: Arm64MoveWide::Zero,
            destination: x(0),
            immediate: 0x1234,
            shift: 16,
        }),
        0xd2a2_4680
    );
    assert_eq!(
        word(Arm64Instruction::MultiplyAdd {
            size: Arm64DataSize::Bits64,
            destination: x(0),
            left: x(1),
            right: x(2),
            addend: Arm64DataRegister::Zero,
            subtract_product: false,
        }),
        0x9b02_7c20
    );
    assert_eq!(
        word(Arm64Instruction::MultiplyAdd {
            size: Arm64DataSize::Bits64,
            destination: x(3),
            left: x(4),
            right: x(5),
            addend: data(6),
            subtract_product: true,
        }),
        0x9b05_9883
    );
    assert_eq!(
        word(Arm64Instruction::Divide {
            size: Arm64DataSize::Bits64,
            destination: x(0),
            left: x(1),
            right: x(2),
            signed: true,
        }),
        0x9ac2_0c20
    );
    assert_eq!(
        word(Arm64Instruction::VariableShift {
            size: Arm64DataSize::Bits64,
            operation: Arm64Shift::Left,
            destination: x(0),
            value: x(1),
            amount: x(2),
        }),
        0x9ac2_2020
    );
    assert_eq!(
        word(Arm64Instruction::VariableShift {
            size: Arm64DataSize::Bits64,
            operation: Arm64Shift::RotateRight,
            destination: x(0),
            value: x(1),
            amount: x(2),
        }),
        0x9ac2_2c20
    );
}

#[test]
fn encodes_lossless_integer_widening_with_exact_signedness() {
    assert_eq!(
        word(Arm64Instruction::BitfieldExtend {
            size: Arm64DataSize::Bits32,
            signed: true,
            source_bits: 8,
            destination: x(0),
            source: x(1),
        }),
        0x1300_1c20
    );
    assert_eq!(
        word(Arm64Instruction::BitfieldExtend {
            size: Arm64DataSize::Bits32,
            signed: false,
            source_bits: 8,
            destination: x(0),
            source: x(1),
        }),
        0x5300_1c20
    );
    assert_eq!(
        word(Arm64Instruction::BitfieldExtend {
            size: Arm64DataSize::Bits64,
            signed: true,
            source_bits: 32,
            destination: x(0),
            source: x(1),
        }),
        0x9340_7c20
    );
}

#[test]
fn encodes_scaled_memory_and_control_instructions() {
    assert_eq!(
        word(Arm64Instruction::LoadUnsigned {
            size: Arm64LoadStoreSize::Double,
            destination: data(0),
            base: Arm64BaseRegister::StackPointer,
            offset: 24,
        }),
        0xf940_0fe0
    );
    assert_eq!(
        word(Arm64Instruction::StoreUnsigned {
            size: Arm64LoadStoreSize::Word,
            source: data(3),
            base: base(4),
            offset: 12,
        }),
        0xb900_0c83
    );
    assert_eq!(
        word(Arm64Instruction::LoadSigned {
            size: Arm64LoadStoreSize::Half,
            destination_size: Arm64DataSize::Bits32,
            destination: data(5),
            base: base(6),
            offset: 10,
        }),
        0x79c0_14c5
    );
    assert_eq!(
        word(Arm64Instruction::ConditionalSet {
            size: Arm64DataSize::Bits32,
            destination: x(0),
            condition: Arm64BranchCondition::Equal,
        }),
        0x1a9f_17e0
    );
    assert_eq!(
        word(Arm64Instruction::Branch {
            displacement: 8,
            link: false,
        }),
        0x1400_0002
    );
    assert_eq!(
        word(Arm64Instruction::Branch {
            displacement: -4,
            link: true,
        }),
        0x97ff_ffff
    );
    assert_eq!(
        word(Arm64Instruction::BranchConditional {
            displacement: 12,
            condition: Arm64BranchCondition::Equal,
        }),
        0x5400_0060
    );
    assert_eq!(
        word(Arm64Instruction::BranchRegister {
            target: x(16),
            link: false,
        }),
        0xd61f_0200
    );
    assert_eq!(
        word(Arm64Instruction::BranchRegister {
            target: x(16),
            link: true,
        }),
        0xd63f_0200
    );
    assert_eq!(
        word(Arm64Instruction::Return { target: x(30) }),
        0xd65f_03c0
    );
    assert_eq!(word(Arm64Instruction::Break { immediate: 1 }), 0xd420_0020);
    assert_eq!(
        word(Arm64Instruction::SupervisorCall { immediate: 0 }),
        0xd400_0001
    );
}

#[test]
fn rejects_values_that_would_be_truncated_or_change_register_meaning() {
    assert_eq!(
        Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: Arm64AddSubtractDestination::Zero,
            source: base(0),
            immediate: 0,
            shift_12: false,
        }
        .encode(),
        Err(Arm64EncodingError::InvalidRegisterRole)
    );
    assert_eq!(
        Arm64Instruction::MoveWide {
            size: Arm64DataSize::Bits32,
            operation: Arm64MoveWide::Keep,
            destination: x(0),
            immediate: 0,
            shift: 32,
        }
        .encode(),
        Err(Arm64EncodingError::InvalidShift)
    );
    assert_eq!(
        Arm64Instruction::LoadUnsigned {
            size: Arm64LoadStoreSize::Double,
            destination: data(0),
            base: base(1),
            offset: 3,
        }
        .encode(),
        Err(Arm64EncodingError::OffsetOutOfRange)
    );
    assert_eq!(
        Arm64Instruction::LoadSigned {
            size: Arm64LoadStoreSize::Double,
            destination_size: Arm64DataSize::Bits64,
            destination: data(0),
            base: base(1),
            offset: 0,
        }
        .encode(),
        Err(Arm64EncodingError::InvalidLoadExtension)
    );
    assert_eq!(
        Arm64Instruction::Branch {
            displacement: 2,
            link: false,
        }
        .encode(),
        Err(Arm64EncodingError::MisalignedBranch)
    );
}

#[test]
fn code_builder_resolves_forward_and_backward_local_labels() {
    let mut builder = crate::Arm64CodeBuilder::new();
    let start = builder.create_label();
    let end = builder.create_label();
    builder.bind(start).unwrap();
    builder.branch(end, false);
    builder.append(Arm64Instruction::NoOperation);
    builder.bind(end).unwrap();
    builder.branch(start, false);
    let code = builder.finish().unwrap();
    let words = code
        .bytes()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();

    assert_eq!(words, [0x1400_0002, 0xd503_201f, 0x17ff_fffe]);
    assert_eq!(code.label_offset(start), Some(0));
    assert_eq!(code.label_offset(end), Some(8));
    assert_eq!(code.instruction_count(), 3);
}

#[test]
fn code_builder_relaxes_a_distant_conditional_branch_once() {
    let mut builder = crate::Arm64CodeBuilder::new();
    let target = builder.create_label();
    builder.branch_conditional(target, Arm64BranchCondition::Equal);
    for _ in 0..262_144 {
        builder.append(Arm64Instruction::NoOperation);
    }
    builder.bind(target).unwrap();
    let code = builder.finish().unwrap();
    let first = u32::from_le_bytes(code.bytes()[0..4].try_into().unwrap());
    let second = u32::from_le_bytes(code.bytes()[4..8].try_into().unwrap());

    assert_eq!(first, 0x5400_0041);
    assert_eq!(second, 0x1404_0001);
    assert_eq!(code.label_offset(target), Some(1_048_584));
    assert_eq!(code.instruction_count(), 262_146);
}

#[test]
fn code_builder_rejects_duplicate_and_unbound_labels() {
    let mut duplicate = crate::Arm64CodeBuilder::new();
    let label = duplicate.create_label();
    duplicate.bind(label).unwrap();
    assert_eq!(
        duplicate.bind(label),
        Err(crate::Arm64CodeError::DuplicateLabel(label))
    );

    let mut unbound = crate::Arm64CodeBuilder::new();
    let label = unbound.create_label();
    unbound.branch(label, false);
    assert_eq!(
        unbound.finish(),
        Err(crate::Arm64CodeError::UnboundLabel(label))
    );

    let mut owner = crate::Arm64CodeBuilder::new();
    let _first = owner.create_label();
    let foreign = owner.create_label();
    let mut unknown = crate::Arm64CodeBuilder::new();
    unknown.branch(foreign, false);
    assert_eq!(
        unknown.finish(),
        Err(crate::Arm64CodeError::UnknownLabel(foreign))
    );
}

#[test]
fn frame_layout_separates_outgoing_objects_saves_and_frame_record() {
    let mut builder = crate::Arm64FrameLayoutBuilder::new();
    builder.require_outgoing_argument_size(16).unwrap();
    builder.require_outgoing_argument_size(0).unwrap();
    let bytes = builder.add_object(3, 1).unwrap();
    let aligned = builder.add_object(16, 16).unwrap();
    let zero = builder.add_object(0, 8).unwrap();
    builder.preserve(x(21)).unwrap();
    builder.preserve(x(19)).unwrap();
    builder.preserve(x(21)).unwrap();
    let frame = builder.finish().unwrap();

    assert_eq!(frame.outgoing_argument_size(), 16);
    assert_eq!(frame.object(bytes).unwrap().offset(), 16);
    assert_eq!(frame.object(aligned).unwrap().offset(), 32);
    assert_eq!(frame.object(zero).unwrap().offset(), 48);
    assert_eq!(
        frame
            .saved_registers()
            .iter()
            .map(|saved| (saved.register().number(), saved.offset()))
            .collect::<Vec<_>>(),
        [(19, 48), (21, 56)]
    );
    assert_eq!(frame.frame_record_offset(), 64);
    assert_eq!(frame.size(), 80);
}

#[test]
fn frame_layout_rejects_invalid_requests_and_overflow() {
    let mut builder = crate::Arm64FrameLayoutBuilder::new();
    assert_eq!(
        builder.require_outgoing_argument_size(8),
        Err(crate::Arm64FrameLayoutError::MisalignedOutgoingArguments(8))
    );
    assert_eq!(
        builder.add_object(1, 3),
        Err(crate::Arm64FrameLayoutError::InvalidObjectAlignment(3))
    );
    assert_eq!(
        builder.add_object(1, 32),
        Err(crate::Arm64FrameLayoutError::InvalidObjectAlignment(32))
    );
    assert_eq!(
        builder.preserve(x(9)),
        Err(crate::Arm64FrameLayoutError::NotCalleeSaved(x(9)))
    );

    let mut overflow = crate::Arm64FrameLayoutBuilder::new();
    overflow
        .require_outgoing_argument_size(u64::MAX - 15)
        .unwrap();
    assert_eq!(
        overflow.finish(),
        Err(crate::Arm64FrameLayoutError::FrameOverflow)
    );
}

#[test]
fn frame_code_materializes_a_complete_small_prologue_and_epilogue() {
    let mut frame = crate::Arm64FrameLayoutBuilder::new();
    frame.require_outgoing_argument_size(16).unwrap();
    frame.add_object(3, 1).unwrap();
    frame.add_object(16, 16).unwrap();
    frame.add_object(0, 8).unwrap();
    frame.preserve(x(21)).unwrap();
    frame.preserve(x(19)).unwrap();
    let frame = frame.finish().unwrap();
    let mut code = crate::Arm64CodeBuilder::new();
    crate::Arm64FrameCode::emit_prologue(&frame, &mut code);
    crate::Arm64FrameCode::emit_epilogue(&frame, &mut code);
    let code = code.finish().unwrap();
    let words = code
        .bytes()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();

    assert_eq!(
        words,
        [
            0xd101_43ff,
            0xf900_23fd,
            0xf900_27fe,
            0xf900_1bf3,
            0xf900_1ff5,
            0x9101_03fd,
            0xf940_1bf3,
            0xf940_1ff5,
            0xf940_27fe,
            0xf940_23fd,
            0x9101_43ff,
            0xd65f_03c0,
        ]
    );
}

#[test]
fn frame_code_materializes_large_sizes_and_distant_slots_through_scratch() {
    let mut frame = crate::Arm64FrameLayoutBuilder::new();
    frame.add_object(32_768, 16).unwrap();
    let frame = frame.finish().unwrap();
    assert_eq!(frame.frame_record_offset(), 32_768);
    assert_eq!(frame.size(), 32_784);
    let mut code = crate::Arm64CodeBuilder::new();
    crate::Arm64FrameCode::emit_prologue(&frame, &mut code);
    crate::Arm64FrameCode::emit_epilogue(&frame, &mut code);
    let code = code.finish().unwrap();
    let words = code
        .bytes()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();

    assert_eq!(words[0], 0xd290_0210);
    assert_eq!(words[1], 0xcb30_63ff);
    assert!(words.contains(&0x8b30_63f0));
    assert_eq!(words.last(), Some(&0xd65f_03c0));
}

#[test]
fn abi_register_roles_form_one_closed_partition() {
    use crate::Arm64AbiRegisterRole;

    let schema = nocter_runtime_contract::RuntimeAbiIdentity::Arm64DarwinV1.schema();
    assert_eq!(crate::Arm64NocterAbi::word_size(), schema.word_size());
    assert_eq!(
        crate::Arm64NocterAbi::stack_alignment(),
        schema.stack_alignment()
    );
    assert_eq!(
        crate::Arm64NocterAbi::direct_value_word_limit(),
        schema.direct_value_word_limit()
    );
    assert_eq!(
        crate::Arm64NocterAbi::indirect_result_register().number(),
        schema.indirect_result_register()
    );

    let roles = (0..31)
        .map(|number| crate::Arm64NocterAbi::role(x(number)))
        .collect::<Vec<_>>();
    assert!(
        roles[0..8]
            .iter()
            .all(|role| *role == Arm64AbiRegisterRole::ArgumentAndResult)
    );
    assert_eq!(roles[8], Arm64AbiRegisterRole::IndirectResult);
    assert_eq!(roles[9], Arm64AbiRegisterRole::AllocationContext);
    assert_eq!(roles[10], Arm64AbiRegisterRole::ProcessContext);
    assert!(
        roles[11..16]
            .iter()
            .all(|role| *role == Arm64AbiRegisterRole::CallerSaved)
    );
    assert!(
        roles[16..18]
            .iter()
            .all(|role| *role == Arm64AbiRegisterRole::CompilerScratch)
    );
    assert_eq!(roles[18], Arm64AbiRegisterRole::Reserved);
    assert!(
        roles[19..29]
            .iter()
            .all(|role| *role == Arm64AbiRegisterRole::CalleeSaved)
    );
    assert_eq!(roles[29], Arm64AbiRegisterRole::FramePointer);
    assert_eq!(roles[30], Arm64AbiRegisterRole::Link);
    assert_eq!(crate::Arm64NocterAbi::argument_register(7), Some(x(7)));
    assert_eq!(crate::Arm64NocterAbi::argument_register(8), None);
    assert_eq!(crate::Arm64NocterAbi::indirect_result_register(), x(8));
    assert_eq!(crate::Arm64NocterAbi::allocation_context_register(), x(9));
    assert_eq!(crate::Arm64NocterAbi::process_context_register(), x(10));
    assert!(crate::Arm64NocterAbi::is_allocatable(x(19)));
    assert!(!crate::Arm64NocterAbi::is_allocatable(x(0)));
    assert!(!crate::Arm64NocterAbi::is_allocatable(x(8)));
    assert!(!crate::Arm64NocterAbi::is_allocatable(x(9)));
    assert!(!crate::Arm64NocterAbi::is_allocatable(x(10)));
    assert!(!crate::Arm64NocterAbi::is_allocatable(x(16)));
    assert!(!crate::Arm64NocterAbi::is_allocatable(x(18)));
}

#[test]
fn register_allocation_reuses_expired_caller_saved_registers() {
    let mut builder = crate::Arm64RegisterAllocationBuilder::new();
    let first = builder.define(0);
    builder.use_at(first, 1).unwrap();
    let second = builder.define(2);
    builder.use_at(second, 3).unwrap();
    let allocation = builder.finish();

    assert_eq!(
        allocation.location(first),
        Some(crate::Arm64AllocatedLocation::Register(x(11)))
    );
    assert_eq!(allocation.location(second), allocation.location(first));
    assert!(allocation.preserved_registers().is_empty());
    assert_eq!(allocation.spill_count(), 0);
}

#[test]
fn register_allocation_keeps_call_crossing_ranges_in_preserved_registers() {
    let mut builder = crate::Arm64RegisterAllocationBuilder::new();
    let local = builder.define(0);
    builder.use_at(local, 1).unwrap();
    let crossing = builder.define(0);
    builder.record_call(2);
    builder.use_at(crossing, 3).unwrap();
    let allocation = builder.finish();

    assert_eq!(
        allocation.location(local),
        Some(crate::Arm64AllocatedLocation::Register(x(11)))
    );
    assert_eq!(
        allocation.location(crossing),
        Some(crate::Arm64AllocatedLocation::Register(x(19)))
    );
    assert_eq!(allocation.preserved_registers(), [x(19)]);
}

#[test]
fn register_allocation_spills_deterministically_after_closed_pool_pressure() {
    let mut builder = crate::Arm64RegisterAllocationBuilder::new();
    let registers = (0..17)
        .map(|_| {
            let register = builder.define(0);
            builder.use_at(register, 10).unwrap();
            register
        })
        .collect::<Vec<_>>();
    let allocation = builder.finish();

    assert_eq!(allocation.spill_count(), 2);
    assert!(matches!(
        allocation.location(registers[15]),
        Some(crate::Arm64AllocatedLocation::Spill(slot)) if slot.index() == 0
    ));
    assert!(matches!(
        allocation.location(registers[16]),
        Some(crate::Arm64AllocatedLocation::Spill(slot)) if slot.index() == 1
    ));
    assert!(registers[..15].iter().all(|register| matches!(
        allocation.location(*register),
        Some(crate::Arm64AllocatedLocation::Register(physical))
            if physical != x(9) && physical != x(10) && physical != x(16) && physical != x(17)
    )));
}

#[test]
fn register_allocation_rejects_unknown_and_predefinition_uses() {
    let mut builder = crate::Arm64RegisterAllocationBuilder::new();
    let register = builder.define(4);
    assert!(matches!(
        builder.use_at(register, 3),
        Err(crate::Arm64RegisterAllocationError::UseBeforeDefinition { .. })
    ));
    let mut empty = crate::Arm64RegisterAllocationBuilder::new();
    assert!(matches!(
        empty.use_at(register, 5),
        Err(crate::Arm64RegisterAllocationError::UnknownVirtualRegister(
            _
        ))
    ));
}

#[test]
fn machine_value_plan_uses_exact_call_crossing_facts() {
    let program = crate::test_support::lower_machine(
        "func identity(value: i32): i32 { value }\n\
         func main(): i32 { 1 + identity(2) }\n",
    );
    let nocter_machine::MachineProgramRoot::Process { entry, .. } = *program.root() else {
        panic!("fixture must produce a process root")
    };
    let function = program.function(entry).unwrap();
    let first = function
        .body()
        .operations()
        .find_map(|(_, operation)| {
            matches!(
                operation.kind(),
                nocter_machine::MachineOperationKind::Constant(
                    nocter_machine::MachineConstant::Integer(1)
                )
            )
            .then(|| operation.result().unwrap())
        })
        .unwrap();
    let call_result = function
        .body()
        .operations()
        .find_map(|(_, operation)| {
            matches!(
                operation.kind(),
                nocter_machine::MachineOperationKind::Call(_)
            )
            .then(|| operation.result().unwrap())
        })
        .unwrap();
    let plan = crate::Arm64ValuePlan::build(function).unwrap();
    let first_register = plan.value(first).unwrap().direct_registers().unwrap()[0];
    let call_result_register = plan.value(call_result).unwrap().direct_registers().unwrap()[0];

    assert!(matches!(
        plan.registers().location(first_register),
        Some(crate::Arm64AllocatedLocation::Register(register))
            if crate::Arm64NocterAbi::is_callee_saved(register)
    ));
    assert!(matches!(
        plan.registers().location(call_result_register),
        Some(crate::Arm64AllocatedLocation::Register(register))
            if !crate::Arm64NocterAbi::is_callee_saved(register)
    ));
}

#[test]
fn machine_value_plan_treats_user_destruction_as_a_call_boundary() {
    let program = crate::test_support::lower_machine(
        "struct Resource {}\n\
         drop Resource(&+self) { return }\n\
         func main(): i32 {\n\
             let status = 42\n\
             let resource = Resource {}\n\
             return status\n\
         }\n",
    );
    let nocter_machine::MachineProgramRoot::Process { entry, .. } = *program.root() else {
        panic!("fixture must produce a process root")
    };
    let function = program.function(entry).unwrap();
    let drop = function
        .body()
        .operations()
        .find_map(|(operation_id, operation)| {
            matches!(
                operation.kind(),
                nocter_machine::MachineOperationKind::InvokeDrop { .. }
            )
            .then_some(operation_id)
        })
        .unwrap();
    let live_after = function.dataflow().operation(drop).unwrap().live_after();
    assert!(!live_after.is_empty());

    let plan = crate::Arm64ValuePlan::build(function).unwrap();
    for value in live_after {
        for register in plan
            .value(*value)
            .and_then(crate::Arm64ValueStorage::direct_registers)
            .unwrap_or(&[])
        {
            assert!(matches!(
                plan.registers().location(*register),
                Some(crate::Arm64AllocatedLocation::Register(register))
                    if crate::Arm64NocterAbi::is_callee_saved(register)
            ));
        }
    }
}

#[test]
fn machine_value_plan_separates_multiword_and_memory_values() {
    let program = crate::test_support::lower_machine(
        "copy struct Pair { first: u64\n    second: u64 }\n\
         struct Large { first: u64\n    second: u64\n    third: u64 }\n\
         func main(): i32 {\n\
             let pair = Pair { first: 1, second: 2 }\n\
             let large = Large { first: 3, second: 4, third: 5 }\n\
             let _ = pair\n\
             drop large\n\
             0\n\
         }\n",
    );
    let nocter_machine::MachineProgramRoot::Process { entry, .. } = *program.root() else {
        panic!("fixture must produce a process root")
    };
    let function = program.function(entry).unwrap();
    let aggregates = function
        .body()
        .operations()
        .filter_map(|(_, operation)| {
            if matches!(
                operation.kind(),
                nocter_machine::MachineOperationKind::Aggregate(_)
            ) {
                operation.result()
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let plan = crate::Arm64ValuePlan::build(function).unwrap();

    assert!(aggregates.iter().any(|value| {
        matches!(
            plan.value(*value),
            Some(crate::Arm64ValueStorage::Direct(registers)) if registers.len() == 2
        )
    }));
    assert!(aggregates.iter().any(|value| {
        matches!(
            plan.value(*value),
            Some(crate::Arm64ValueStorage::Memory {
                size: 24,
                alignment: 8,
            })
        )
    }));

    let frame = crate::Arm64FunctionFrame::build(&program, entry, &plan).unwrap();
    let staging = frame.direct_aggregate_staging().unwrap();
    let staging = frame.layout().object(staging).unwrap();
    assert_eq!(staging.size(), 16);
    assert_eq!(staging.alignment(), 8);

    let large = aggregates
        .iter()
        .copied()
        .find(|value| {
            matches!(
                plan.value(*value),
                Some(crate::Arm64ValueStorage::Memory { .. })
            )
        })
        .unwrap();
    let large_object = frame
        .layout()
        .object(frame.memory_value(large).unwrap())
        .unwrap();
    assert_eq!(large_object.size(), 24);
    assert_eq!(large_object.alignment(), 8);
}

#[test]
fn machine_value_plan_accepts_edge_defined_join_values() {
    let program = crate::test_support::lower_machine(
        "func choose(condition: bool): i32 {\n\
             if condition { 1 } else { 2 }\n\
         }\n\
         func main(): i32 { choose(true) }\n",
    );
    let function = program
        .functions()
        .map(|(_, function)| function)
        .find(|function| {
            function
                .body()
                .blocks()
                .any(|(_, block)| !block.parameters().is_empty())
        })
        .unwrap();
    let plan = crate::Arm64ValuePlan::build(function).unwrap();

    for parameter in function
        .body()
        .blocks()
        .flat_map(|(_, block)| block.parameters())
    {
        assert!(matches!(
            plan.value(*parameter),
            Some(crate::Arm64ValueStorage::Direct(registers)) if registers.len() == 1
        ));
    }
}

#[test]
fn function_frame_places_outgoing_arguments_spills_and_preserved_registers_together() {
    let program = crate::test_support::lower_machine(
        "func sink(\n\
             a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32,\n\
             i: i32, j: i32, k: i32, l: i32, m: i32, n: i32, o: i32, p: i32,\n\
             q: i32,\n\
         ): i32 { 0 }\n\
         func main(): i32 {\n\
             sink(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17)\n\
         }\n",
    );
    let nocter_machine::MachineProgramRoot::Process { entry, .. } = *program.root() else {
        panic!("fixture must produce a process root")
    };
    let values = crate::Arm64ValuePlan::build(program.function(entry).unwrap()).unwrap();
    assert!(values.registers().spill_count() > 0);
    let frame = crate::Arm64FunctionFrame::build(&program, entry, &values).unwrap();

    assert_eq!(frame.layout().outgoing_argument_size(), 80);
    for register in values.registers().preserved_registers() {
        assert!(
            frame
                .layout()
                .saved_registers()
                .iter()
                .any(|saved| saved.register() == *register)
        );
    }
    for (_, operation) in program.function(entry).unwrap().body().operations() {
        let Some(result) = operation.result() else {
            continue;
        };
        let Some(registers) = values
            .value(result)
            .and_then(crate::Arm64ValueStorage::direct_registers)
        else {
            continue;
        };
        for register in registers {
            if let Some(crate::Arm64AllocatedLocation::Spill(spill)) =
                values.registers().location(*register)
            {
                assert!(frame.spill(spill).is_some());
            }
        }
    }
}

#[test]
fn function_frame_places_pack_descriptor_and_source_order_state() {
    let program = crate::test_support::lower_machine(
        "struct Vec<T> {}\n\
         construct Vec<T> {\n\
             pub literal [](...items: T): Self {\n\
                 let _ = items.len()\n\
                 for item in items {}\n\
                 return Self {}\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             let values = Vec [1, 2]\n\
             drop values\n\
             0\n\
         }\n",
    );
    let (function_id, function, pack_id) = program
        .functions()
        .find_map(|(function_id, function)| {
            function
                .body()
                .packs()
                .next()
                .map(|(pack_id, _)| (function_id, function, pack_id))
        })
        .unwrap();
    let values = crate::Arm64ValuePlan::build(function).unwrap();
    let frame = crate::Arm64FunctionFrame::build(&program, function_id, &values).unwrap();
    let pack = frame.pack(pack_id).unwrap();
    let descriptor = frame.layout().object(pack.descriptor()).unwrap();
    let state = frame.layout().object(pack.state()).unwrap();

    assert_eq!(
        (descriptor.size(), descriptor.alignment()),
        (
            crate::Arm64PackDescriptorLayout::SIZE,
            crate::Arm64PackDescriptorLayout::ALIGNMENT,
        )
    );
    assert_eq!(pack.state_layout().cursor_offset(), 0);
    assert_eq!(pack.state_layout().segments().len(), 2);
    assert!(matches!(
        pack.state_layout().segments(),
        [
            crate::Arm64PackSegmentLayout::Value {
                value_offset: 8,
                size: 4,
                alignment: 4,
            },
            crate::Arm64PackSegmentLayout::Value {
                value_offset: 12,
                size: 4,
                alignment: 4,
            },
        ]
    ));
    assert_eq!((state.size(), state.alignment()), (16, 8));
}

#[test]
fn function_frame_retains_hidden_abi_pointers_and_root_context() {
    let program = crate::test_support::lower_machine(
        "struct Large { first: u64\n    second: u64\n    third: u64 }\n\
         func make(): Large { Large { first: 1, second: 2, third: 3 } }\n\
         func main(): i32 {\n\
             let value = make()\n\
             drop value\n\
             0\n\
         }\n",
    );
    let nocter_machine::MachineProgramRoot::Process { root, .. } = *program.root() else {
        panic!("fixture must produce a process root")
    };
    let root_values = crate::Arm64ValuePlan::build(program.function(root).unwrap()).unwrap();
    let root_frame = crate::Arm64FunctionFrame::build(&program, root, &root_values).unwrap();
    assert!(matches!(
        root_frame.allocation_context(),
        crate::Arm64AllocationContextFrame::ProgramRoot(object)
            if root_frame.layout().object(object).is_some_and(|object| object.size() == 16)
    ));

    let (function_id, function) = program
        .functions()
        .find(|(_, function)| {
            matches!(
                function.kind(),
                nocter_machine::MachineFunctionKind::Callable(abi)
                    if matches!(
                        abi.result(),
                        nocter_machine::MachineResultAbi::Value(result)
                            if matches!(
                                result.location(),
                                nocter_machine::MachineResultLocation::CallerStorage { .. }
                            )
                    )
            )
        })
        .unwrap();
    let values = crate::Arm64ValuePlan::build(function).unwrap();
    let frame = crate::Arm64FunctionFrame::build(&program, function_id, &values).unwrap();
    assert!(frame.indirect_result_pointer().is_some());
}

#[test]
fn lowers_a_constant_process_through_selection_and_spill_materialization() {
    let machine = crate::test_support::lower_machine("func main(): i32 { 42 }\n");
    let program = crate::Arm64Program::lower_machine(&machine).unwrap();
    let entry = program.function(program.entry()).unwrap();
    let entry_start = usize::try_from(entry.offset()).unwrap();
    let entry_end = usize::try_from(entry.offset() + entry.size()).unwrap();
    let words = program.text()[entry_start..entry_end]
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();

    assert!(words.contains(&0xd280_0030));
    assert_eq!(words.last(), Some(&0xd400_1001));
    assert!(program.text().chunks_exact(4).any(|bytes| {
        let word = u32::from_le_bytes(bytes.try_into().unwrap());
        word & 0xff80_0000 == 0xd280_0000 && (word >> 5) & 0xffff == 42
    }));
}

#[test]
fn test_suite_retains_declaration_order_and_independent_entries() {
    let machine = crate::test_support::lower_tests(
        "test first { return }\n\
         test second { return }\n",
    );
    let suite = crate::Arm64TestSuite::lower_machine(&machine).unwrap();

    assert_eq!(
        suite
            .tests()
            .iter()
            .map(crate::Arm64TestExecutable::name)
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_ne!(
        suite.tests()[0].program().entry(),
        suite.tests()[1].program().entry()
    );
    assert_eq!(
        suite.tests()[0].program().text().as_ptr(),
        suite.tests()[1].program().text().as_ptr(),
        "test entries must share one completed code payload"
    );
}

#[test]
fn process_and_test_lowering_reject_the_opposite_root_kind() {
    let process = crate::test_support::lower_machine("func main(): i32 { 0 }\n");
    let tests = crate::test_support::lower_tests("test only { return }\n");

    assert!(matches!(
        crate::Arm64TestSuite::lower_machine(&process),
        Err(crate::Arm64LoweringError::ExpectedTestProgram)
    ));
    assert!(matches!(
        crate::Arm64Program::lower_machine(&tests),
        Err(crate::Arm64LoweringError::ExpectedProcessProgram)
    ));
}

#[test]
fn empty_test_target_produces_no_synthetic_entry() {
    let machine = crate::test_support::lower_tests("");
    let suite = crate::Arm64TestSuite::lower_machine(&machine).unwrap();

    assert!(suite.tests().is_empty());
}

#[test]
fn program_layout_resolves_calls_and_section_addresses() {
    let mut builder = crate::Arm64ProgramBuilder::new();
    let target = builder.declare_function();
    let caller = builder.declare_function();
    let _prefix = builder.add_data([1, 2, 3], 1).unwrap();
    let text = builder.add_data([9], 8).unwrap();

    let mut target_code = crate::Arm64CodeBuilder::new();
    target_code.append(Arm64Instruction::Return { target: x(30) });
    builder
        .define_function(target, target_code.finish().unwrap())
        .unwrap();

    let mut caller_code = crate::Arm64CodeBuilder::new();
    caller_code.append(Arm64Instruction::NoOperation);
    caller_code.call(target);
    caller_code.load_function_address(target, x(4));
    caller_code.load_data_address(text, x(3));
    caller_code.append(Arm64Instruction::Return { target: x(30) });
    builder
        .define_function(caller, caller_code.finish().unwrap())
        .unwrap();
    builder.set_entry(caller).unwrap();
    let program = builder.finish().unwrap();
    let words = program
        .text()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();

    assert_eq!(
        words,
        [
            0xd65f_03c0,
            0xd503_201f,
            0x97ff_fffe,
            0x9000_0004,
            0x9100_0084,
            0x9000_0003,
            0x9100_0063,
            0xd65f_03c0,
        ]
    );
    assert_eq!(program.function(target).unwrap().offset(), 0);
    assert_eq!(program.function(caller).unwrap().offset(), 4);
    assert_eq!(program.entry(), caller);
    assert_eq!(program.read_only_data(), [1, 2, 3, 0, 0, 0, 0, 0, 9]);
    assert_eq!(program.data(text).unwrap().offset(), 8);
    assert_eq!(program.data_fixups().len(), 1);
    assert_eq!(program.data_fixups()[0].instruction_offset(), 20);
    assert_eq!(program.data_fixups()[0].target_offset(), 8);
    assert_eq!(program.data_fixups()[0].destination(), x(3));

    let relocated = program
        .relocate_sections(0x1_0000_0250, 0x1_0000_2000)
        .unwrap();
    let relocated_words = relocated
        .text()
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(
        relocated_words[3],
        u32::from_le_bytes(
            Arm64Instruction::AddressPage {
                destination: x(4),
                displacement: 0,
            }
            .encode()
            .unwrap(),
        )
    );
    assert_eq!(
        relocated_words[4],
        u32::from_le_bytes(
            Arm64Instruction::AddSubtractImmediate {
                size: Arm64DataSize::Bits64,
                operation: crate::Arm64AddSubtract::Add,
                set_flags: false,
                destination: crate::Arm64AddSubtractDestination::General(x(4)),
                source: crate::Arm64BaseRegister::General(x(4)),
                immediate: 0x250,
                shift_12: false,
            }
            .encode()
            .unwrap(),
        )
    );
    assert_eq!(relocated_words[5], 0xd000_0003);
    assert_eq!(relocated_words[6], 0x9100_2063);
    assert_eq!(
        program.relocate_sections(0, 1_u64 << 32),
        Err(crate::Arm64ProgramError::Encoding(
            Arm64EncodingError::PageAddressOutOfRange
        ))
    );
}

#[test]
fn data_pointer_relocations_resolve_section_local_targets() {
    let mut builder = crate::Arm64ProgramBuilder::new();
    let target = builder.add_data([9], 8).unwrap();
    let pointer = builder.add_data([0; 8], 8).unwrap();
    builder.add_data_relocation(pointer, 0, target).unwrap();
    let entry = builder.declare_function();
    let mut code = crate::Arm64CodeBuilder::new();
    code.append(Arm64Instruction::Return { target: x(30) });
    builder
        .define_function(entry, code.finish().unwrap())
        .unwrap();
    builder.set_entry(entry).unwrap();

    let program = builder.finish().unwrap();
    assert_eq!(program.data(target).unwrap().offset(), 0);
    assert_eq!(program.data(pointer).unwrap().offset(), 8);
    assert_eq!(program.data_pointer_fixups()[0].location_offset(), 8);
    assert_eq!(program.data_pointer_fixups()[0].target_offset(), 0);

    let relocated = program.relocate_sections(0, 0x1_0000_2000).unwrap();
    assert_eq!(
        &relocated.read_only_data()[8..16],
        &(0x1_0000_2000_u64).to_le_bytes()
    );
}

#[test]
fn data_relocations_are_validated_at_the_builder_boundary() {
    let mut builder = crate::Arm64ProgramBuilder::new();
    let target = builder.add_data([1], 1).unwrap();
    let source = builder.add_data([0; 16], 8).unwrap();

    assert_eq!(
        builder.add_data_relocation(source, 9, target),
        Err(crate::Arm64ProgramError::InvalidDataRelocation(9))
    );
    builder.add_data_relocation(source, 0, target).unwrap();
    assert_eq!(
        builder.add_data_relocation(source, 4, target),
        Err(crate::Arm64ProgramError::OverlappingDataRelocation {
            source,
            first: 0,
            second: 4,
        })
    );
}

#[test]
fn program_layout_rejects_incomplete_and_foreign_identities() {
    let empty = crate::Arm64ProgramBuilder::new();
    assert_eq!(empty.finish(), Err(crate::Arm64ProgramError::MissingEntry));

    let mut missing = crate::Arm64ProgramBuilder::new();
    let missing_body = missing.declare_function();
    missing.set_entry(missing_body).unwrap();
    assert_eq!(
        missing.finish(),
        Err(crate::Arm64ProgramError::MissingFunction(missing_body))
    );

    let mut owner = crate::Arm64ProgramBuilder::new();
    let _first = owner.declare_function();
    let foreign = owner.declare_function();
    let mut branches = crate::Arm64ProgramBuilder::new();
    let entry = branches.declare_function();
    let mut code = crate::Arm64CodeBuilder::new();
    code.call(foreign);
    branches
        .define_function(entry, code.finish().unwrap())
        .unwrap();
    branches.set_entry(entry).unwrap();
    assert_eq!(
        branches.finish(),
        Err(crate::Arm64ProgramError::UnknownFunction(foreign))
    );

    let mut addresses = crate::Arm64ProgramBuilder::new();
    let entry = addresses.declare_function();
    let mut code = crate::Arm64CodeBuilder::new();
    code.load_function_address(foreign, x(0));
    addresses
        .define_function(entry, code.finish().unwrap())
        .unwrap();
    addresses.set_entry(entry).unwrap();
    assert_eq!(
        addresses.finish(),
        Err(crate::Arm64ProgramError::UnknownFunction(foreign))
    );

    let mut data_owner = crate::Arm64ProgramBuilder::new();
    let _first = data_owner.add_data([], 1).unwrap();
    let foreign = data_owner.add_data([], 1).unwrap();
    let mut references = crate::Arm64ProgramBuilder::new();
    let entry = references.declare_function();
    let mut code = crate::Arm64CodeBuilder::new();
    code.load_data_address(foreign, x(0));
    references
        .define_function(entry, code.finish().unwrap())
        .unwrap();
    references.set_entry(entry).unwrap();
    assert_eq!(
        references.finish(),
        Err(crate::Arm64ProgramError::UnknownData(foreign))
    );
}
