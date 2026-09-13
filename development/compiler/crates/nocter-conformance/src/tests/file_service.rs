use nocter_arm64::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DarwinFileJobTargets, Arm64DarwinFileRetirementTargets,
    Arm64DarwinFileServiceImports, Arm64DarwinFileServiceRootTargets, Arm64DataRegister,
    Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize, Arm64MoveWide,
    Arm64ProgramBuilder, Arm64Register,
};
use nocter_runtime_contract::{
    DarwinFileAccess, DarwinFileCompletionAbiSchema, DarwinFileCompletionField,
    DarwinFileOperation, DarwinFileServiceAdmission, RuntimeAbiIdentity,
};

fn register(number: u8) -> Arm64Register {
    Arm64Register::new(number).unwrap()
}

fn move_register(code: &mut Arm64CodeBuilder, destination: Arm64Register, source: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate: 0,
        shift_12: false,
    });
}

fn load_small(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u16) {
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination,
        immediate: value,
        shift: 0,
    });
}

fn declare_job_driver(
    program: &mut Arm64ProgramBuilder,
    jobs: Arm64DarwinFileJobTargets,
) -> Arm64FunctionId {
    let driver = program.declare_function();
    let mut code = Arm64CodeBuilder::new();
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: 32,
        shift_12: false,
    });
    for (source, offset) in [(register(19), 8), (register(20), 16), (register(30), 24)] {
        code.append(Arm64Instruction::StoreUnsigned {
            size: Arm64LoadStoreSize::Double,
            source: Arm64DataRegister::General(source),
            base: Arm64BaseRegister::StackPointer,
            offset,
        });
    }
    move_register(&mut code, register(19), register(0));
    move_register(&mut code, register(20), register(1));
    let poll = code.create_label();
    code.bind(poll).unwrap();
    move_register(&mut code, register(0), register(19));
    code.call(jobs.resume());
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(register(0)),
        immediate: u16::try_from(
            RuntimeAbiIdentity::Arm64DarwinV1
                .schema()
                .asynchronous()
                .completed_status(),
        )
        .unwrap(),
        shift_12: false,
    });
    code.branch_conditional(poll, Arm64BranchCondition::NotEqual);
    move_register(&mut code, register(0), register(19));
    move_register(&mut code, register(1), register(20));
    code.call(jobs.consume());
    for (destination, offset) in [(register(19), 8), (register(20), 16), (register(30), 24)] {
        code.append(Arm64Instruction::LoadUnsigned {
            size: Arm64LoadStoreSize::Double,
            destination: Arm64DataRegister::General(destination),
            base: Arm64BaseRegister::StackPointer,
            offset,
        });
    }
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: 32,
        shift_12: false,
    });
    code.append(Arm64Instruction::BranchRegister {
        target: register(30),
        link: false,
    });
    program
        .define_function(driver, code.finish().unwrap())
        .unwrap();
    driver
}

#[test]
fn generated_root_is_reused_and_released_natively() {
    let register = |number| Arm64Register::new(number).unwrap();
    let context = RuntimeAbiIdentity::Arm64DarwinV1.schema().process_context();
    let frame_size = u16::try_from(context.size().next_multiple_of(16)).unwrap();
    let service_offset = u32::try_from(context.blocking_service_pointer_offset()).unwrap();
    let mut program = Arm64ProgramBuilder::new();
    let imports = Arm64DarwinFileServiceImports::declare(&mut program).unwrap();
    let retirement = Arm64DarwinFileRetirementTargets::declare(&mut program, &imports).unwrap();
    let service =
        Arm64DarwinFileServiceRootTargets::declare(&mut program, &imports, retirement).unwrap();
    let entry = program.declare_function();
    let mut code = Arm64CodeBuilder::new();
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: frame_size,
        shift_12: false,
    });
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::StackPointer,
        offset: service_offset,
    });
    let call_with_context = |code: &mut Arm64CodeBuilder, target| {
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: Arm64AddSubtractDestination::General(register(0)),
            source: Arm64BaseRegister::StackPointer,
            immediate: 0,
            shift_12: false,
        });
        code.call(target);
    };
    call_with_context(&mut code, service.ensure());
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(register(19)),
        source: Arm64BaseRegister::General(register(0)),
        immediate: 0,
        shift_12: false,
    });
    call_with_context(&mut code, service.ensure());
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(register(0)),
        right: Arm64DataRegister::General(register(19)),
    });
    let mismatch = code.create_label();
    code.branch_conditional(mismatch, Arm64BranchCondition::NotEqual);
    for _ in 0..2 {
        call_with_context(&mut code, service.shutdown());
    }
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: register(0),
        immediate: 0,
        shift: 0,
    });
    let exit = code.create_label();
    code.branch(exit, false);
    code.bind(mismatch).unwrap();
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: register(0),
        immediate: 1,
        shift: 0,
    });
    code.bind(exit).unwrap();
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination: register(16),
        immediate: 1,
        shift: 0,
    });
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    program
        .define_function(entry, code.finish().unwrap())
        .unwrap();
    program.set_entry(entry).unwrap();
    let image = nocter_macho::MachOImage::build(&program.finish().unwrap()).unwrap();

    super::execute_and_assert_status(&image, 0);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one native entry keeps the retirement transition order visible in the conformance case"
)]
fn generated_retirement_reserves_owns_and_closes_a_descriptor_natively() {
    let register = |number| Arm64Register::new(number).unwrap();
    let context = RuntimeAbiIdentity::Arm64DarwinV1.schema().process_context();
    let asynchronous = RuntimeAbiIdentity::Arm64DarwinV1.schema().asynchronous();
    let context_size = context.size().next_multiple_of(16);
    let completion = DarwinFileCompletionAbiSchema::ARM64_DARWIN;
    let completion_offset = u32::try_from(context_size).unwrap();
    let frame_size =
        u16::try_from((context_size + completion.size()).next_multiple_of(16)).unwrap();
    let service_offset = u32::try_from(context.blocking_service_pointer_offset()).unwrap();
    let mut program = Arm64ProgramBuilder::new();
    let imports = Arm64DarwinFileServiceImports::declare(&mut program).unwrap();
    let retirement = Arm64DarwinFileRetirementTargets::declare(&mut program, &imports).unwrap();
    let service =
        Arm64DarwinFileServiceRootTargets::declare(&mut program, &imports, retirement).unwrap();
    let entry = program.declare_function();
    let mut code = Arm64CodeBuilder::new();
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: frame_size,
        shift_12: false,
    });
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::StackPointer,
        offset: service_offset,
    });
    let move_register = |code: &mut Arm64CodeBuilder, destination, source| {
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: Arm64AddSubtractDestination::General(destination),
            source: Arm64BaseRegister::General(source),
            immediate: 0,
            shift_12: false,
        });
    };
    let load_small = |code: &mut Arm64CodeBuilder, destination, value| {
        code.append(Arm64Instruction::MoveWide {
            size: Arm64DataSize::Bits64,
            operation: Arm64MoveWide::Zero,
            destination,
            immediate: value,
            shift: 0,
        });
    };
    let call_with_context = |code: &mut Arm64CodeBuilder, target| {
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: Arm64AddSubtractDestination::General(register(0)),
            source: Arm64BaseRegister::StackPointer,
            immediate: 0,
            shift_12: false,
        });
        code.call(target);
    };
    let fail = code.create_label();
    let finish = code.create_label();

    call_with_context(&mut code, service.ensure());
    move_register(&mut code, register(19), register(0));
    move_register(&mut code, register(0), register(19));
    load_small(&mut code, register(1), 0);
    code.call(retirement.reserve());
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(register(1)),
        immediate: u16::try_from(DarwinFileServiceAdmission::Ready.code()).unwrap(),
        shift_12: false,
    });
    code.branch_conditional(fail, Arm64BranchCondition::NotEqual);
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(register(0)),
        immediate: 0,
        shift_12: false,
    });
    code.branch_conditional(fail, Arm64BranchCondition::Equal);
    move_register(&mut code, register(20), register(0));

    load_small(&mut code, register(16), 0x2a);
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Keep,
        destination: register(16),
        immediate: 0x0200,
        shift: 16,
    });
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    code.branch_conditional(fail, Arm64BranchCondition::CarrySet);
    move_register(&mut code, register(21), register(0));
    move_register(&mut code, register(22), register(1));

    move_register(&mut code, register(0), register(20));
    move_register(&mut code, register(1), register(21));
    code.call(retirement.publish_owner());
    move_register(&mut code, register(0), register(20));
    code.call(retirement.begin_close());
    let poll = code.create_label();
    code.bind(poll).unwrap();
    move_register(&mut code, register(0), register(20));
    code.call(retirement.resume_close());
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(register(0)),
        immediate: u16::try_from(asynchronous.completed_status()).unwrap(),
        shift_12: false,
    });
    code.branch_conditional(poll, Arm64BranchCondition::NotEqual);
    move_register(&mut code, register(0), register(20));
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(register(1)),
        source: Arm64BaseRegister::StackPointer,
        immediate: u16::try_from(completion_offset).unwrap(),
        shift_12: false,
    });
    code.call(retirement.consume_close());
    for field in DarwinFileCompletionField::ALL {
        code.append(Arm64Instruction::LoadUnsigned {
            size: Arm64LoadStoreSize::Double,
            destination: Arm64DataRegister::General(register(0)),
            base: Arm64BaseRegister::StackPointer,
            offset: completion_offset + u32::try_from(completion.offset(*field)).unwrap(),
        });
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Subtract,
            set_flags: true,
            destination: Arm64AddSubtractDestination::Zero,
            source: Arm64BaseRegister::General(register(0)),
            immediate: 0,
            shift_12: false,
        });
        code.branch_conditional(fail, Arm64BranchCondition::NotEqual);
    }

    move_register(&mut code, register(0), register(22));
    load_small(&mut code, register(16), 6);
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Keep,
        destination: register(16),
        immediate: 0x0200,
        shift: 16,
    });
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    call_with_context(&mut code, service.shutdown());
    load_small(&mut code, register(0), 0);
    code.branch(finish, false);
    code.bind(fail).unwrap();
    load_small(&mut code, register(0), 1);
    code.bind(finish).unwrap();
    load_small(&mut code, register(16), 1);
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    program
        .define_function(entry, code.finish().unwrap())
        .unwrap();
    program.set_entry(entry).unwrap();
    let image = nocter_macho::MachOImage::build(&program.finish().unwrap()).unwrap();

    super::execute_and_assert_status(&image, 0);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one native path proves construction, worker publication, consumption, and retirement"
)]
fn generated_file_jobs_open_and_read_without_executor_thread_syscalls() {
    let context = RuntimeAbiIdentity::Arm64DarwinV1.schema().process_context();
    let completion = DarwinFileCompletionAbiSchema::ARM64_DARWIN;
    let context_size = context.size().next_multiple_of(16);
    let completion_offset = u32::try_from(context_size).unwrap();
    let read_offset = completion_offset + u32::try_from(completion.size()).unwrap();
    let frame_size = u16::try_from((u64::from(read_offset) + 8).next_multiple_of(16)).unwrap();
    let service_offset = u32::try_from(context.blocking_service_pointer_offset()).unwrap();
    let mut program = Arm64ProgramBuilder::new();
    let path = program.add_data(b"/dev/null".as_slice(), 1).unwrap();
    let imports = Arm64DarwinFileServiceImports::declare(&mut program).unwrap();
    let retirement = Arm64DarwinFileRetirementTargets::declare(&mut program, &imports).unwrap();
    let service =
        Arm64DarwinFileServiceRootTargets::declare(&mut program, &imports, retirement).unwrap();
    let jobs =
        Arm64DarwinFileJobTargets::declare(&mut program, &imports, service, retirement).unwrap();
    let drive = declare_job_driver(&mut program, jobs);
    let entry = program.declare_function();
    let mut code = Arm64CodeBuilder::new();
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: frame_size,
        shift_12: false,
    });
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::Zero,
        base: Arm64BaseRegister::StackPointer,
        offset: service_offset,
    });
    load_small(&mut code, register(9), 0);
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(register(10)),
        source: Arm64BaseRegister::StackPointer,
        immediate: 0,
        shift_12: false,
    });
    let fail = code.create_label();
    let finish = code.create_label();

    code.load_data_address(path, register(0));
    load_small(&mut code, register(1), 9);
    load_small(
        &mut code,
        register(2),
        u16::from(DarwinFileAccess::Read.code()),
    );
    code.call(jobs.constructor(DarwinFileOperation::Open));
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(register(1)),
        source: Arm64BaseRegister::StackPointer,
        immediate: u16::try_from(completion_offset).unwrap(),
        shift_12: false,
    });
    code.call(drive);
    for field in [
        DarwinFileCompletionField::FailureKind,
        DarwinFileCompletionField::FailureErrno,
    ] {
        code.append(Arm64Instruction::LoadUnsigned {
            size: Arm64LoadStoreSize::Double,
            destination: Arm64DataRegister::General(register(0)),
            base: Arm64BaseRegister::StackPointer,
            offset: completion_offset + u32::try_from(completion.offset(field)).unwrap(),
        });
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Subtract,
            set_flags: true,
            destination: Arm64AddSubtractDestination::Zero,
            source: Arm64BaseRegister::General(register(0)),
            immediate: 0,
            shift_12: false,
        });
        code.branch_conditional(fail, Arm64BranchCondition::NotEqual);
    }
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(register(19)),
        base: Arm64BaseRegister::StackPointer,
        offset: completion_offset
            + u32::try_from(completion.offset(DarwinFileCompletionField::RetirementRecord))
                .unwrap(),
    });
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(register(19)),
        immediate: 0,
        shift_12: false,
    });
    code.branch_conditional(fail, Arm64BranchCondition::Equal);

    move_register(&mut code, register(0), register(19));
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(register(1)),
        source: Arm64BaseRegister::StackPointer,
        immediate: u16::try_from(read_offset).unwrap(),
        shift_12: false,
    });
    load_small(&mut code, register(2), 8);
    code.call(jobs.constructor(DarwinFileOperation::Read));
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(register(1)),
        source: Arm64BaseRegister::StackPointer,
        immediate: u16::try_from(completion_offset).unwrap(),
        shift_12: false,
    });
    code.call(drive);
    for field in [
        DarwinFileCompletionField::TransferredByteCount,
        DarwinFileCompletionField::FailureKind,
        DarwinFileCompletionField::FailureErrno,
    ] {
        code.append(Arm64Instruction::LoadUnsigned {
            size: Arm64LoadStoreSize::Double,
            destination: Arm64DataRegister::General(register(0)),
            base: Arm64BaseRegister::StackPointer,
            offset: completion_offset + u32::try_from(completion.offset(field)).unwrap(),
        });
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Subtract,
            set_flags: true,
            destination: Arm64AddSubtractDestination::Zero,
            source: Arm64BaseRegister::General(register(0)),
            immediate: 0,
            shift_12: false,
        });
        code.branch_conditional(fail, Arm64BranchCondition::NotEqual);
    }
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(register(19)),
        base: Arm64BaseRegister::StackPointer,
        offset: completion_offset,
    });
    move_register(&mut code, register(0), register(19));
    code.call(retirement.begin_close());
    let poll_close = code.create_label();
    code.bind(poll_close).unwrap();
    move_register(&mut code, register(0), register(19));
    code.call(retirement.resume_close());
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(register(0)),
        immediate: u16::try_from(
            RuntimeAbiIdentity::Arm64DarwinV1
                .schema()
                .asynchronous()
                .completed_status(),
        )
        .unwrap(),
        shift_12: false,
    });
    code.branch_conditional(poll_close, Arm64BranchCondition::NotEqual);
    move_register(&mut code, register(0), register(19));
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(register(1)),
        source: Arm64BaseRegister::StackPointer,
        immediate: u16::try_from(completion_offset).unwrap(),
        shift_12: false,
    });
    code.call(retirement.consume_close());
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(register(0)),
        source: Arm64BaseRegister::StackPointer,
        immediate: 0,
        shift_12: false,
    });
    code.call(service.shutdown());
    load_small(&mut code, register(0), 0);
    code.branch(finish, false);
    code.bind(fail).unwrap();
    load_small(&mut code, register(0), 1);
    code.bind(finish).unwrap();
    load_small(&mut code, register(16), 1);
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    program
        .define_function(entry, code.finish().unwrap())
        .unwrap();
    program.set_entry(entry).unwrap();
    let image = nocter_macho::MachOImage::build(&program.finish().unwrap()).unwrap();

    super::execute_and_assert_status(&image, 0);
}
