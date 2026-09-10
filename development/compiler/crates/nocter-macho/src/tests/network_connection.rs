use nocter_arm64::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DarwinNetworkAdapterImports, Arm64DataSize, Arm64FunctionId,
    Arm64Instruction, Arm64Program, Arm64ProgramBuilder, add_darwin_plain_connection_targets,
};
use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkConnectionEventPollAbiSchema,
    DarwinNetworkConnectionState, DarwinNetworkOwnerCreateStatus,
};

use super::network_callback::{adjust_stack, immediate, load, move_register, stack_address, x};
use crate::MachOImage;

fn connection_lifecycle_program() -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let address = program
        .add_data(
            [16, 2, 0, 1, 127, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0].as_slice(),
            4,
        )
        .unwrap();
    let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
    let connection = add_darwin_plain_connection_targets(&mut program, &imports).unwrap();
    define_connection_entry(
        &mut program,
        entry,
        connection.create(),
        connection.lifecycle(),
        connection.events(),
        address,
    );
    program.set_entry(entry).unwrap();
    program.finish().unwrap()
}

fn connection_disposal_worker_program() -> Arm64Program {
    let mut program = Arm64ProgramBuilder::new();
    let entry = program.declare_function();
    let address = program
        .add_data(
            [16, 2, 0, 1, 127, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0].as_slice(),
            4,
        )
        .unwrap();
    let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
    let connection = add_darwin_plain_connection_targets(&mut program, &imports).unwrap();
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 16);
    code.append(Arm64Instruction::StoreUnsigned {
        size: nocter_arm64::Arm64LoadStoreSize::Double,
        source: nocter_arm64::Arm64DataRegister::General(x(30)),
        base: Arm64BaseRegister::StackPointer,
        offset: 8,
    });
    immediate(&mut code, x(0), 48);
    code.load_function_import(
        imports.function(DarwinNetworkAdapterFunction::Malloc),
        x(16),
    );
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
    move_register(&mut code, x(26), x(0));
    move_register(&mut code, x(0), x(26));
    code.load_data_address(address, x(1));
    call_function(&mut code, connection.create());
    compare_immediate(
        &mut code,
        x(0),
        DarwinNetworkOwnerCreateStatus::Created.code(),
    );
    let created = code.create_label();
    code.branch_conditional(created, Arm64BranchCondition::Equal);
    immediate(&mut code, x(0), 4);
    immediate(&mut code, x(16), 1);
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    code.bind(created).unwrap();
    move_register(&mut code, x(0), x(26));
    call_function(&mut code, connection.disposal().worker());
    immediate(&mut code, x(0), 0);
    immediate(&mut code, x(16), 1);
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    program
        .define_function(entry, code.finish().unwrap())
        .unwrap();
    program.set_entry(entry).unwrap();
    program.finish().unwrap()
}

fn define_connection_entry(
    program: &mut Arm64ProgramBuilder,
    entry: Arm64FunctionId,
    create: Arm64FunctionId,
    lifecycle: nocter_arm64::Arm64DarwinNetworkOwnerLifecycleTargets,
    events: nocter_arm64::Arm64DarwinNetworkConnectionEventTargets,
    address: nocter_arm64::Arm64DataId,
) {
    const OWNER_OFFSET: u32 = 0;
    const OBSERVATION_OFFSET: u32 = 48;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 96);
    stack_address(&mut code, OWNER_OFFSET, x(26));
    move_register(&mut code, x(0), x(26));
    code.load_data_address(address, x(1));
    call_function(&mut code, create);
    compare_immediate(
        &mut code,
        x(0),
        DarwinNetworkOwnerCreateStatus::Created.code(),
    );
    let created = code.create_label();
    code.branch_conditional(created, Arm64BranchCondition::Equal);
    immediate(&mut code, x(0), 2);
    immediate(&mut code, x(16), 1);
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    code.bind(created).unwrap();
    move_register(&mut code, x(0), x(26));
    immediate(&mut code, x(1), 0);
    immediate(&mut code, x(2), 0);
    stack_address(&mut code, OBSERVATION_OFFSET, x(8));
    call_function(&mut code, events.try_receive());
    load(
        &mut code,
        x(27),
        OBSERVATION_OFFSET
            + u32::try_from(
                DarwinNetworkConnectionEventPollAbiSchema::ARM64_DARWIN.available_offset(),
            )
            .unwrap(),
    );
    let empty = code.create_label();
    compare_immediate(&mut code, x(27), 0);
    code.branch_conditional(empty, Arm64BranchCondition::Equal);
    immediate(&mut code, x(0), 3);
    immediate(&mut code, x(16), 1);
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    code.bind(empty).unwrap();
    move_register(&mut code, x(0), x(26));
    call_function(&mut code, lifecycle.start());
    move_register(&mut code, x(0), x(26));
    call_function(&mut code, lifecycle.request_cancel());

    let receive = code.create_label();
    code.bind(receive).unwrap();
    move_register(&mut code, x(0), x(26));
    immediate(&mut code, x(1), 0);
    immediate(&mut code, x(2), 0);
    stack_address(&mut code, OBSERVATION_OFFSET, x(8));
    call_function(&mut code, events.receive());
    load(&mut code, x(27), OBSERVATION_OFFSET);
    compare_immediate(
        &mut code,
        x(27),
        nocter_runtime_contract::DarwinNetworkEventKind::ConnectionState.code(),
    );
    code.branch_conditional(receive, Arm64BranchCondition::NotEqual);
    load(&mut code, x(27), OBSERVATION_OFFSET + 8);
    compare_immediate(
        &mut code,
        x(27),
        DarwinNetworkConnectionState::Cancelled.code(),
    );
    code.branch_conditional(receive, Arm64BranchCondition::NotEqual);
    move_register(&mut code, x(0), x(26));
    call_function(&mut code, lifecycle.complete_release_barrier());
    move_register(&mut code, x(0), x(26));
    call_function(&mut code, lifecycle.release());
    immediate(&mut code, x(0), 0);
    immediate(&mut code, x(16), 1);
    code.append(Arm64Instruction::SupervisorCall { immediate: 0x80 });
    program
        .define_function(entry, code.finish().unwrap())
        .unwrap();
}

fn call_function(code: &mut Arm64CodeBuilder, target: Arm64FunctionId) {
    code.load_function_address(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

fn compare_immediate(
    code: &mut Arm64CodeBuilder,
    value: nocter_arm64::Arm64Register,
    immediate_value: u64,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: u16::try_from(immediate_value).unwrap(),
        shift_12: false,
    });
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn generated_image_releases_network_ownership_after_cancelled_and_queue_barrier() {
    use std::os::unix::fs::PermissionsExt;

    let image = MachOImage::build(&connection_lifecycle_program()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "nocter-macho-network-lifecycle-test-{}",
        std::process::id()
    ));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(status.code(), Some(0));
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn generated_disposal_worker_drains_and_releases_a_connection_owner() {
    use std::os::unix::fs::PermissionsExt;

    let image = MachOImage::build(&connection_disposal_worker_program()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "nocter-macho-network-disposal-test-{}",
        std::process::id()
    ));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(status.code(), Some(0));
}
