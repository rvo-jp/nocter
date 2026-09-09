use nocter_arm64::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DarwinNetworkAdapterImports, Arm64DataSize, Arm64FunctionId,
    Arm64Instruction, Arm64Program, Arm64ProgramBuilder, add_darwin_plain_connection_targets,
};
use nocter_runtime_contract::{DarwinNetworkConnectionState, DarwinNetworkOwnerCreateStatus};

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

fn define_connection_entry(
    program: &mut Arm64ProgramBuilder,
    entry: Arm64FunctionId,
    create: Arm64FunctionId,
    lifecycle: nocter_arm64::Arm64DarwinNetworkConnectionLifecycleTargets,
    events: nocter_arm64::Arm64DarwinNetworkConnectionEventTargets,
    address: nocter_arm64::Arm64DataId,
) {
    const OWNER_OFFSET: u32 = 0;
    const OBSERVATION_OFFSET: u32 = 48;
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 80);
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
    call_function(&mut code, lifecycle.start());
    move_register(&mut code, x(0), x(26));
    call_function(&mut code, lifecycle.request_cancel());

    let receive = code.create_label();
    code.bind(receive).unwrap();
    move_register(&mut code, x(0), x(26));
    stack_address(&mut code, OBSERVATION_OFFSET, x(8));
    call_function(&mut code, events.receive_state());
    load(&mut code, x(27), OBSERVATION_OFFSET);
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
