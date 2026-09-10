use std::fmt;

use nocter_runtime_contract::{DarwinTlsAdapterFunction, DarwinTlsTrustAnchorAbiSchema};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinTlsAdapterImports, Arm64DataRegister,
    Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize, Arm64MoveWide,
    Arm64ProgramBuilder, Arm64ProgramError, Arm64Register,
};

/// Adds the bounded compiler-owned copy used by an escaping TLS verification callback.
///
/// The target accepts non-empty DER bytes in `(x0, x1)` and returns either a complete context in
/// `x0` or null. It validates both the pointer and allocation-size overflow before
/// allocation, so its caller cannot publish a context whose encoded length is malformed.
pub(crate) fn add_darwin_tls_trust_context_create_target(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinTlsAdapterImports,
) -> Result<Arm64FunctionId, Arm64DarwinTlsTrustContextError> {
    let target = program.declare_function();
    let mut code = Arm64CodeBuilder::new();
    let saved = [(x(19), 0), (x(20), 8), (x(21), 16), (x(30), 24)];
    adjust_stack(&mut code, Arm64AddSubtract::Subtract, 32);
    for (register, offset) in saved {
        store_stack(&mut code, register, offset);
    }
    move_register(&mut code, x(19), x(0));
    move_register(&mut code, x(20), x(1));

    let failed = code.create_label();
    let complete = code.create_label();
    compare_zero(&mut code, x(19));
    code.branch_conditional(failed, Arm64BranchCondition::Equal);
    compare_zero(&mut code, x(20));
    code.branch_conditional(failed, Arm64BranchCondition::Equal);
    add_immediate_checked(
        &mut code,
        x(0),
        x(20),
        u16::try_from(DarwinTlsTrustAnchorAbiSchema::ARM64_DARWIN.bytes_offset())
            .map_err(|_| Arm64DarwinTlsTrustContextError::ContractLayout)?,
    );
    code.branch_conditional(failed, Arm64BranchCondition::CarrySet);
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::Malloc),
    );
    move_register(&mut code, x(21), x(0));
    compare_zero(&mut code, x(21));
    code.branch_conditional(failed, Arm64BranchCondition::Equal);
    store_to(
        &mut code,
        x(20),
        x(21),
        u32::try_from(DarwinTlsTrustAnchorAbiSchema::ARM64_DARWIN.length_offset())
            .map_err(|_| Arm64DarwinTlsTrustContextError::ContractLayout)?,
    );
    add_immediate(
        &mut code,
        x(0),
        x(21),
        u16::try_from(DarwinTlsTrustAnchorAbiSchema::ARM64_DARWIN.bytes_offset())
            .map_err(|_| Arm64DarwinTlsTrustContextError::ContractLayout)?,
    );
    move_register(&mut code, x(1), x(19));
    move_register(&mut code, x(2), x(20));
    call(
        &mut code,
        imports.function(DarwinTlsAdapterFunction::MemoryCopy),
    );
    move_register(&mut code, x(0), x(21));
    code.branch(complete, false);

    code.bind(failed)?;
    immediate(&mut code, x(0), 0, 0);
    code.bind(complete)?;
    for (register, offset) in saved {
        load_stack(&mut code, register, offset);
    }
    adjust_stack(&mut code, Arm64AddSubtract::Add, 32);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    program.define_function(target, code.finish()?)?;
    Ok(target)
}

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u16, shift: u8) {
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: Arm64MoveWide::Zero,
        destination,
        immediate: value,
        shift,
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

fn add_immediate_checked(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    source: Arm64Register,
    value: u16,
) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: true,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate: value,
        shift_12: false,
    });
}

fn move_register(code: &mut Arm64CodeBuilder, destination: Arm64Register, source: Arm64Register) {
    add_immediate(code, destination, source, 0);
}

fn store_to(code: &mut Arm64CodeBuilder, source: Arm64Register, base: Arm64Register, offset: u32) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
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

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}

#[derive(Debug)]
pub enum Arm64DarwinTlsTrustContextError {
    ContractLayout,
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinTlsTrustContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin TLS trust context failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinTlsTrustContextError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

impl From<Arm64CodeError> for Arm64DarwinTlsTrustContextError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinTlsTrustContextError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use super::add_darwin_tls_trust_context_create_target;
    use crate::{Arm64DarwinTlsAdapterImports, Arm64ProgramBuilder};

    #[test]
    fn trust_context_constructor_uses_the_tls_allocation_catalog() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinTlsAdapterImports::declare(&mut program).unwrap();
        add_darwin_tls_trust_context_create_target(&mut program, &imports).unwrap();
    }
}
