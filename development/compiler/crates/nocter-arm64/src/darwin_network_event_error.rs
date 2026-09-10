use std::fmt;

use nocter_runtime_contract::DarwinNetworkAdapterFunction;

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DarwinNetworkAdapterImports, Arm64Instruction,
    Arm64Register,
};

/// Consumes one optional retained provider error into stable `(domain, code)` words.
///
/// Source layers never observe a provider object. Register validation makes the destructive input
/// and two outputs non-aliasing and reserves the emitter's call registers, so callers cannot
/// silently overwrite an ownership-bearing pointer before it is released.
pub(crate) fn emit_darwin_network_consume_error(
    code: &mut Arm64CodeBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
    error: Arm64Register,
    domain: Arm64Register,
    error_code: Arm64Register,
) -> Result<(), Arm64DarwinNetworkErrorConsumptionError> {
    let registers = [error, domain, error_code];
    for (index, register) in registers.iter().copied().enumerate() {
        if register == x(0) || register == x(16) {
            return Err(Arm64DarwinNetworkErrorConsumptionError::ReservedRegister(
                register,
            ));
        }
        if registers[..index].contains(&register) {
            return Err(Arm64DarwinNetworkErrorConsumptionError::AliasedRegister(
                register,
            ));
        }
    }
    immediate(code, domain, 0);
    immediate(code, error_code, 0);
    let consumed = code.create_label();
    compare_zero(code, error);
    code.branch_conditional(consumed, Arm64BranchCondition::Equal);
    move_register(code, x(0), error);
    call_import(
        code,
        imports.function(DarwinNetworkAdapterFunction::NetworkErrorGetDomain),
    );
    move_register(code, domain, x(0));
    move_register(code, x(0), error);
    call_import(
        code,
        imports.function(DarwinNetworkAdapterFunction::NetworkErrorGetCode),
    );
    move_register(code, error_code, x(0));
    move_register(code, x(0), error);
    call_import(
        code,
        imports.function(DarwinNetworkAdapterFunction::NetworkRelease),
    );
    code.bind(consumed)?;
    Ok(())
}

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u16) {
    code.append(Arm64Instruction::MoveWide {
        size: crate::Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate: value,
        shift: 0,
    });
}

fn compare_zero(code: &mut Arm64CodeBuilder, value: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: crate::Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: 0,
        shift_12: false,
    });
}

fn move_register(code: &mut Arm64CodeBuilder, destination: Arm64Register, source: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: crate::Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate: 0,
        shift_12: false,
    });
}

fn call_import(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

const fn x(number: u8) -> Arm64Register {
    match Arm64Register::new(number) {
        Some(register) => register,
        None => panic!("closed ARM64 register is valid"),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64DarwinNetworkErrorConsumptionError {
    ReservedRegister(Arm64Register),
    AliasedRegister(Arm64Register),
    Code(Arm64CodeError),
}

impl fmt::Display for Arm64DarwinNetworkErrorConsumptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 Darwin network event error consumption failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64DarwinNetworkErrorConsumptionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Code(error) => Some(error),
            Self::ReservedRegister(_) | Self::AliasedRegister(_) => None,
        }
    }
}

impl From<Arm64CodeError> for Arm64DarwinNetworkErrorConsumptionError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{Arm64DarwinNetworkErrorConsumptionError, emit_darwin_network_consume_error};
    use crate::{
        Arm64CodeBuilder, Arm64DarwinNetworkAdapterImports, Arm64ProgramBuilder, Arm64Register,
    };

    fn x(number: u8) -> Arm64Register {
        Arm64Register::new(number).unwrap()
    }

    #[test]
    fn error_consumption_rejects_aliases_and_private_call_registers() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        assert_eq!(
            emit_darwin_network_consume_error(
                &mut Arm64CodeBuilder::new(),
                &imports,
                x(19),
                x(19),
                x(20),
            ),
            Err(Arm64DarwinNetworkErrorConsumptionError::AliasedRegister(x(
                19
            )))
        );
        assert_eq!(
            emit_darwin_network_consume_error(
                &mut Arm64CodeBuilder::new(),
                &imports,
                x(0),
                x(19),
                x(20),
            ),
            Err(Arm64DarwinNetworkErrorConsumptionError::ReservedRegister(
                x(0)
            ))
        );
    }
}
