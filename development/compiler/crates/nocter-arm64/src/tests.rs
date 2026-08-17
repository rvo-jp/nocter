use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64DataRegister, Arm64DataSize, Arm64EncodingError, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MoveWide, Arm64Register, Arm64Shift,
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

    let roles = (0..31)
        .map(|number| crate::Arm64NocterAbi::role(x(number)))
        .collect::<Vec<_>>();
    assert!(
        roles[0..8]
            .iter()
            .all(|role| *role == Arm64AbiRegisterRole::ArgumentAndResult)
    );
    assert_eq!(roles[8], Arm64AbiRegisterRole::IndirectResult);
    assert!(
        roles[9..16]
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
    assert!(crate::Arm64NocterAbi::is_allocatable(x(19)));
    assert!(!crate::Arm64NocterAbi::is_allocatable(x(16)));
    assert!(!crate::Arm64NocterAbi::is_allocatable(x(18)));
}

#[test]
fn program_layout_resolves_calls_and_retains_only_section_address_fixups() {
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
    assert_eq!(program.data_fixups()[0].instruction_offset(), 12);
    assert_eq!(program.data_fixups()[0].target_offset(), 8);
    assert_eq!(program.data_fixups()[0].destination(), x(3));

    let relocated = program
        .relocate_data_addresses(0x1_0000_0000, 0x1_0000_2000)
        .unwrap();
    let relocated_words = relocated
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(relocated_words[3], 0xd000_0003);
    assert_eq!(relocated_words[4], 0x9100_2063);
    assert_eq!(
        program.relocate_data_addresses(0, 1_u64 << 32),
        Err(crate::Arm64ProgramError::Encoding(
            Arm64EncodingError::PageAddressOutOfRange
        ))
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
