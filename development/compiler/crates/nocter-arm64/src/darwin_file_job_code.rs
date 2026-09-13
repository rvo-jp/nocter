use nocter_runtime_contract::{
    DarwinFileJobAbiSchema, DarwinFileJobField, DarwinFileServiceAbiSchema, DarwinFileServiceField,
    DarwinFileServiceFunction,
};

use crate::darwin_kernel_abi::{DarwinSystemCall, emit_system_call};
use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64AtomicUpdateRegisters, Arm64BaseRegister,
    Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64Register,
};

pub(crate) const STACK_SIZE: u64 = 112;
const NOTIFICATION_BYTE_OFFSET: u64 = 0;

pub(crate) fn prologue(code: &mut Arm64CodeBuilder) {
    crate::frame_access::adjust_stack(code, STACK_SIZE, Arm64AddSubtract::Subtract);
    for (register, offset) in [
        (19, 16),
        (20, 24),
        (21, 32),
        (22, 40),
        (23, 48),
        (24, 56),
        (25, 64),
        (26, 72),
        (27, 80),
        (28, 88),
        (9, 96),
        (10, 104),
    ] {
        crate::frame_access::store_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            x(register),
            offset,
        );
    }
    crate::frame_access::store_at_stack_offset(code, Arm64LoadStoreSize::Double, x(30), 8);
}

pub(crate) fn epilogue(code: &mut Arm64CodeBuilder) {
    for (register, offset) in [
        (19, 16),
        (20, 24),
        (21, 32),
        (22, 40),
        (23, 48),
        (24, 56),
        (25, 64),
        (26, 72),
        (27, 80),
        (28, 88),
        (9, 96),
        (10, 104),
    ] {
        crate::frame_access::load_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            x(register),
            offset,
        );
    }
    crate::frame_access::load_at_stack_offset(code, Arm64LoadStoreSize::Double, x(30), 8);
    crate::frame_access::adjust_stack(code, STACK_SIZE, Arm64AddSubtract::Add);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
}

pub(crate) fn pending(code: &mut Arm64CodeBuilder, job: Arm64Register) {
    let schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    immediate(code, x(0), schema.asynchronous().pending_status());
    address(
        code,
        x(1),
        job,
        schema.offset(DarwinFileJobField::InterestKind),
    );
    immediate(code, x(2), 1);
    epilogue(code);
}

pub(crate) fn completed(code: &mut Arm64CodeBuilder) {
    let asynchronous = DarwinFileJobAbiSchema::ARM64_DARWIN.asynchronous();
    immediate(code, x(0), asynchronous.completed_status());
    immediate(code, x(1), 0);
    immediate(code, x(2), 0);
    epilogue(code);
}

pub(crate) fn signal(code: &mut Arm64CodeBuilder, job: Arm64Register) {
    let job_schema = DarwinFileJobAbiSchema::ARM64_DARWIN;
    let service_schema = DarwinFileServiceAbiSchema::ARM64_DARWIN;
    load(
        code,
        x(20),
        job,
        job_schema.offset(DarwinFileJobField::Service),
    );
    load(
        code,
        x(0),
        x(20),
        service_schema.offset(DarwinFileServiceField::NotificationWriter),
    );
    signal_descriptor(code, x(0));
}

/// Signals a previously retained notification descriptor without consulting released storage.
pub(crate) fn signal_descriptor(code: &mut Arm64CodeBuilder, descriptor: Arm64Register) {
    move_register(code, x(0), descriptor);
    immediate(code, x(8), 1);
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Byte,
        x(8),
        NOTIFICATION_BYTE_OFFSET,
    );
    crate::frame_access::form_stack_address(code, x(1), NOTIFICATION_BYTE_OFFSET);
    immediate(code, x(2), 1);
    emit_system_call(code, DarwinSystemCall::Write);
}

pub(crate) fn call_import(
    code: &mut Arm64CodeBuilder,
    imports: &crate::Arm64DarwinFileServiceImports,
    role: DarwinFileServiceFunction,
) {
    code.load_function_import(imports.function(role), x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

pub(crate) fn abort(code: &mut Arm64CodeBuilder, imports: &crate::Arm64DarwinFileServiceImports) {
    call_import(code, imports, DarwinFileServiceFunction::Abort);
}

pub(crate) fn address(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) {
    move_register(code, destination, base);
    crate::address_code::add_offset(code, destination, offset);
}

pub(crate) fn load(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) {
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        destination,
        base,
        offset,
    );
}

pub(crate) fn store(
    code: &mut Arm64CodeBuilder,
    base: Arm64Register,
    offset: u64,
    source: Arm64Register,
) {
    crate::address_code::store_native(code, Arm64LoadStoreSize::Double, source, base, offset);
}

pub(crate) fn move_register(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    source: Arm64Register,
) {
    crate::address_code::move_register(code, source, destination);
}

pub(crate) fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u64) {
    crate::frame_access::load_immediate(code, destination, value, Arm64DataSize::Bits64);
}

pub(crate) fn compare_immediate(code: &mut Arm64CodeBuilder, value: Arm64Register, expected: u64) {
    let expected_register = if value == x(8) { x(9) } else { x(8) };
    immediate(code, expected_register, expected);
    compare_register(code, value, expected_register);
}

pub(crate) fn compare_register(
    code: &mut Arm64CodeBuilder,
    left: Arm64Register,
    right: Arm64Register,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

pub(crate) fn add_register(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    left: Arm64Register,
    right: Arm64Register,
    set_flags: bool,
) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags,
        destination: Arm64DataRegister::General(destination),
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

pub(crate) fn add_immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    source: Arm64Register,
    value: u16,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate: value,
        shift_12: false,
    });
}

pub(crate) fn atomic_registers(
    address: Arm64Register,
    observed: Arm64Register,
    status: Arm64Register,
) -> Arm64AtomicUpdateRegisters {
    Arm64AtomicUpdateRegisters::new(address, observed, status)
        .expect("closed file-job registers do not alias")
}

pub(crate) fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}
