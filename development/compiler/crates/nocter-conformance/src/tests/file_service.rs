use nocter_arm64::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DarwinFileServiceImports, Arm64DarwinFileServiceRootTargets,
    Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize, Arm64MoveWide,
    Arm64ProgramBuilder, Arm64Register,
};
use nocter_runtime_contract::RuntimeAbiIdentity;

#[test]
fn generated_root_is_reused_and_released_natively() {
    let register = |number| Arm64Register::new(number).unwrap();
    let context = RuntimeAbiIdentity::Arm64DarwinV1.schema().process_context();
    let frame_size = u16::try_from(context.size().next_multiple_of(16)).unwrap();
    let service_offset = u32::try_from(context.blocking_service_pointer_offset()).unwrap();
    let mut program = Arm64ProgramBuilder::new();
    let imports = Arm64DarwinFileServiceImports::declare(&mut program).unwrap();
    let service = Arm64DarwinFileServiceRootTargets::declare(&mut program, &imports).unwrap();
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
