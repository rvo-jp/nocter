use nocter_runtime_contract::{
    DarwinBlockAbiSchema, DarwinTlsAdapterData, DarwinTlsAdapterFunction,
    DarwinTlsTrustAnchorAbiSchema,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DarwinTlsAdapterImports, Arm64DarwinTlsCallbackError, Arm64DataRegister,
    Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LabelId, Arm64LoadStoreSize,
    Arm64MoveWide, Arm64ProgramBuilder, Arm64Register,
};

/// Adds the sole custom-anchor verification callback admitted by the TLS adapter.
///
/// The callback materializes Security.framework values only for the duration of one provider
/// verification request. It augments the system trust store with the captured DER certificate,
/// evaluates the provider-created trust object, releases every temporary, and invokes the
/// provider completion exactly once.
pub(crate) fn add_darwin_tls_verify_callback(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinTlsAdapterImports,
) -> Result<Arm64FunctionId, Arm64DarwinTlsCallbackError> {
    const FRAME_SIZE: u16 = 96;
    const CERTIFICATE_SLOT: u32 = 0;
    let saved = [
        (x(19), 32),
        (x(20), 40),
        (x(21), 48),
        (x(22), 56),
        (x(23), 64),
        (x(24), 72),
        (x(30), 88),
    ];
    let target = program.declare_function();
    let mut code = Arm64CodeBuilder::new();
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, FRAME_SIZE);
    for (register, offset) in saved {
        store_stack(&mut code, register, offset);
    }
    let block = DarwinBlockAbiSchema::ARM64_DARWIN;
    let anchor = DarwinTlsTrustAnchorAbiSchema::ARM64_DARWIN;
    load_from(&mut code, x(19), x(0), offset(block.captures_offset())?);
    move_register(&mut code, x(21), x(3));
    immediate(&mut code, x(24), 0);

    let complete = code.create_label();
    let release_trust = code.create_label();
    let release_certificate = code.create_label();
    let release_array = code.create_label();
    compare_zero(&mut code, x(19));
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    move_register(&mut code, x(0), x(2));
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::CopyTrustReference),
    );
    move_register(&mut code, x(20), x(0));
    compare_zero(&mut code, x(20));
    code.branch_conditional(complete, Arm64BranchCondition::Equal);

    emit_custom_anchor_array(
        &mut code,
        imports,
        anchor,
        CERTIFICATE_SLOT,
        release_trust,
        release_certificate,
    )?;
    emit_custom_anchor_evaluation(&mut code, imports, release_array);

    code.bind(release_array)?;
    move_register(&mut code, x(0), x(22));
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::CoreFoundationRelease),
    );
    code.bind(release_certificate)?;
    move_register(&mut code, x(0), x(23));
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::CoreFoundationRelease),
    );

    code.bind(release_trust)?;
    move_register(&mut code, x(0), x(20));
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::CoreFoundationRelease),
    );

    code.bind(complete)?;
    load_from(&mut code, x(16), x(21), offset(block.invoke_offset())?);
    move_register(&mut code, x(0), x(21));
    move_register(&mut code, x(1), x(24));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
    for (register, offset) in saved {
        load_stack(&mut code, register, offset);
    }
    adjust_stack(&mut code, Arm64AddSubtract::Add, FRAME_SIZE);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    program.define_function(target, code.finish()?)?;
    Ok(target)
}

fn emit_custom_anchor_array(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinTlsAdapterImports,
    anchor: DarwinTlsTrustAnchorAbiSchema,
    certificate_slot: u32,
    release_trust: Arm64LabelId,
    release_certificate: Arm64LabelId,
) -> Result<(), Arm64DarwinTlsCallbackError> {
    immediate(code, x(0), 0);
    add_immediate(
        code,
        x(1),
        x(19),
        u16::try_from(anchor.bytes_offset())
            .map_err(|_| Arm64DarwinTlsCallbackError::ContractLayout)?,
    );
    load_from(code, x(2), x(19), offset(anchor.length_offset())?);
    call(
        code,
        imports.function(DarwinTlsAdapterFunction::CoreFoundationDataCreate),
    );
    move_register(code, x(22), x(0));
    compare_zero(code, x(22));
    code.branch_conditional(release_trust, Arm64BranchCondition::Equal);

    immediate(code, x(0), 0);
    move_register(code, x(1), x(22));
    call(
        code,
        imports.function(DarwinTlsAdapterFunction::SecurityCertificateCreateWithData),
    );
    move_register(code, x(23), x(0));
    move_register(code, x(0), x(22));
    call(
        code,
        imports.function(DarwinTlsAdapterFunction::CoreFoundationRelease),
    );
    compare_zero(code, x(23));
    code.branch_conditional(release_trust, Arm64BranchCondition::Equal);

    store_stack(code, x(23), certificate_slot);
    immediate(code, x(0), 0);
    stack_address(code, x(1), certificate_slot);
    immediate(code, x(2), 1);
    code.load_data_import(
        imports.data(DarwinTlsAdapterData::CoreFoundationTypeArrayCallbacks),
        x(3),
    );
    call(
        code,
        imports.function(DarwinTlsAdapterFunction::CoreFoundationArrayCreate),
    );
    move_register(code, x(22), x(0));
    compare_zero(code, x(22));
    code.branch_conditional(release_certificate, Arm64BranchCondition::Equal);
    Ok(())
}

fn emit_custom_anchor_evaluation(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinTlsAdapterImports,
    failed: Arm64LabelId,
) {
    move_register(code, x(0), x(20));
    move_register(code, x(1), x(22));
    call(
        code,
        imports.function(DarwinTlsAdapterFunction::SecurityTrustSetAnchorCertificates),
    );
    compare_zero(code, x(0));
    code.branch_conditional(failed, Arm64BranchCondition::NotEqual);
    move_register(code, x(0), x(20));
    immediate(code, x(1), 0);
    call(
        code,
        imports.function(DarwinTlsAdapterFunction::SecurityTrustSetAnchorCertificatesOnly),
    );
    compare_zero(code, x(0));
    code.branch_conditional(failed, Arm64BranchCondition::NotEqual);
    move_register(code, x(0), x(20));
    immediate(code, x(1), 0);
    call(
        code,
        imports.function(DarwinTlsAdapterFunction::SecurityTrustEvaluateWithError),
    );
    move_register(code, x(24), x(0));
}

fn offset(value: u64) -> Result<u32, Arm64DarwinTlsCallbackError> {
    u32::try_from(value).map_err(|_| Arm64DarwinTlsCallbackError::ContractLayout)
}

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u16) {
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination,
        immediate: value,
        shift: 0,
    });
}

fn compare_zero(code: &mut Arm64CodeBuilder, value: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: 0,
        shift_12: false,
    });
}

fn add_immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    source: Arm64Register,
    immediate: u16,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate,
        shift_12: false,
    });
}

fn move_register(code: &mut Arm64CodeBuilder, destination: Arm64Register, source: Arm64Register) {
    add_immediate(code, destination, source, 0);
}

fn stack_address(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::StackPointer,
        immediate: u16::try_from(offset).expect("closed callback frame offset fits ARM64 add"),
        shift_12: false,
    });
}

fn load_from(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u32,
) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(base),
        offset,
    });
}

fn adjust_stack(code: &mut Arm64CodeBuilder, operation: Arm64AddSubtract, amount: u16) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: amount,
        shift_12: false,
    });
}

fn store_stack(code: &mut Arm64CodeBuilder, source: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn load_stack(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn call(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

#[cfg(test)]
mod tests {
    use super::add_darwin_tls_verify_callback;
    use crate::{Arm64DarwinTlsAdapterImports, Arm64ProgramBuilder};

    #[test]
    fn trust_callback_consumes_only_the_tls_capability_catalog() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinTlsAdapterImports::declare(&mut program).unwrap();
        add_darwin_tls_verify_callback(&mut program, &imports).unwrap();
    }
}
