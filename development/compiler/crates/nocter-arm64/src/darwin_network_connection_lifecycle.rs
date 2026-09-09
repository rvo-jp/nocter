use std::fmt;

use nocter_runtime_contract::{
    DarwinNetworkAdapterFunction, DarwinNetworkAdapterOperation, DarwinNetworkOwnerAbiSchema,
    DarwinNetworkOwnerField, DarwinNetworkOwnerKind,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64CodeError, Arm64DarwinNetworkAdapterImports, Arm64DarwinNetworkOwnerError,
    Arm64DataRegister, Arm64DataSize, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize,
    Arm64ProgramBuilder, Arm64ProgramError, Arm64Register, emit_darwin_network_owner_guard,
    emit_darwin_network_owner_release, emit_darwin_network_owner_transition,
};

/// Native callable targets for the complete terminal lifecycle of one connection owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkConnectionLifecycleTargets {
    start: Arm64FunctionId,
    request_cancel: Arm64FunctionId,
    observe_final_state: Arm64FunctionId,
    complete_release_barrier: Arm64FunctionId,
    release: Arm64FunctionId,
}

impl Arm64DarwinNetworkConnectionLifecycleTargets {
    #[must_use]
    pub const fn start(self) -> Arm64FunctionId {
        self.start
    }

    #[must_use]
    pub const fn request_cancel(self) -> Arm64FunctionId {
        self.request_cancel
    }

    #[must_use]
    pub const fn observe_final_state(self) -> Arm64FunctionId {
        self.observe_final_state
    }

    #[must_use]
    pub const fn complete_release_barrier(self) -> Arm64FunctionId {
        self.complete_release_barrier
    }

    #[must_use]
    pub const fn release(self) -> Arm64FunctionId {
        self.release
    }
}

/// Adds the only production call targets that advance and release a connection owner.
///
/// Every target accepts the owner address in `x0`. The barrier target performs
/// `dispatch_sync_f` on the owner's serial queue before committing the quiesced state; release is
/// consequently impossible while a callback can still be executing.
///
/// # Errors
///
/// Propagates malformed runtime contracts and ARM64 program/code construction failures.
pub fn add_darwin_network_connection_lifecycle_targets(
    program: &mut Arm64ProgramBuilder,
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<Arm64DarwinNetworkConnectionLifecycleTargets, Arm64DarwinNetworkConnectionLifecycleError>
{
    let start = program.declare_function();
    let request_cancel = program.declare_function();
    let observe_final_state = program.declare_function();
    let complete_release_barrier = program.declare_function();
    let release = program.declare_function();
    let barrier_callback = program.declare_function();

    program.define_function(
        start,
        owner_import_operation(
            imports,
            DarwinNetworkAdapterOperation::Start,
            DarwinNetworkAdapterFunction::ConnectionStart,
        )?,
    )?;
    program.define_function(
        request_cancel,
        owner_import_operation(
            imports,
            DarwinNetworkAdapterOperation::RequestCancel,
            DarwinNetworkAdapterFunction::ConnectionCancel,
        )?,
    )?;
    program.define_function(
        observe_final_state,
        owner_transition(imports, DarwinNetworkAdapterOperation::ObserveFinalState)?,
    )?;
    program.define_function(barrier_callback, return_only()?)?;
    program.define_function(
        complete_release_barrier,
        complete_barrier(imports, barrier_callback)?,
    )?;
    program.define_function(release, release_owner(imports)?)?;

    Ok(Arm64DarwinNetworkConnectionLifecycleTargets {
        start,
        request_cancel,
        observe_final_state,
        complete_release_barrier,
        release,
    })
}

fn owner_import_operation(
    imports: &Arm64DarwinNetworkAdapterImports,
    operation: DarwinNetworkAdapterOperation,
    imported: DarwinNetworkAdapterFunction,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionLifecycleError> {
    let mut code = Arm64CodeBuilder::new();
    owner_prologue(&mut code);
    emit_darwin_network_owner_transition(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        operation,
        imports,
    )?;
    load_owner_field(
        &mut code,
        x(0),
        x(19),
        DarwinNetworkOwnerField::NativeObject,
    )?;
    call_import(&mut code, imports.function(imported));
    owner_epilogue(&mut code);
    code.finish().map_err(Into::into)
}

fn owner_transition(
    imports: &Arm64DarwinNetworkAdapterImports,
    operation: DarwinNetworkAdapterOperation,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionLifecycleError> {
    let mut code = Arm64CodeBuilder::new();
    owner_prologue(&mut code);
    emit_darwin_network_owner_transition(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        operation,
        imports,
    )?;
    owner_epilogue(&mut code);
    code.finish().map_err(Into::into)
}

fn complete_barrier(
    imports: &Arm64DarwinNetworkAdapterImports,
    barrier_callback: Arm64FunctionId,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionLifecycleError> {
    let mut code = Arm64CodeBuilder::new();
    owner_prologue(&mut code);
    emit_darwin_network_owner_guard(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::CompleteReleaseBarrier,
        imports,
    )?;
    load_owner_field(&mut code, x(0), x(19), DarwinNetworkOwnerField::SerialQueue)?;
    immediate(&mut code, x(1), 0)?;
    code.load_function_address(barrier_callback, x(2));
    call_import(
        &mut code,
        imports.function(DarwinNetworkAdapterFunction::DispatchSync),
    );
    emit_darwin_network_owner_transition(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        DarwinNetworkAdapterOperation::CompleteReleaseBarrier,
        imports,
    )?;
    owner_epilogue(&mut code);
    code.finish().map_err(Into::into)
}

fn release_owner(
    imports: &Arm64DarwinNetworkAdapterImports,
) -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionLifecycleError> {
    let mut code = Arm64CodeBuilder::new();
    owner_prologue(&mut code);
    emit_darwin_network_owner_release(
        &mut code,
        x(19),
        DarwinNetworkOwnerKind::Connection,
        imports,
    )?;
    owner_epilogue(&mut code);
    code.finish().map_err(Into::into)
}

fn return_only() -> Result<crate::Arm64Code, Arm64DarwinNetworkConnectionLifecycleError> {
    let mut code = Arm64CodeBuilder::new();
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
    code.finish().map_err(Into::into)
}

fn owner_prologue(code: &mut Arm64CodeBuilder) {
    adjust_stack(code, Arm64AddSubtract::Subtract);
    store_stack(code, x(19), 0);
    store_stack(code, x(30), 8);
    move_register(code, x(19), x(0));
}

fn owner_epilogue(code: &mut Arm64CodeBuilder) {
    load_stack(code, x(19), 0);
    load_stack(code, x(30), 8);
    adjust_stack(code, Arm64AddSubtract::Add);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
}

fn load_owner_field(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    owner: Arm64Register,
    field: DarwinNetworkOwnerField,
) -> Result<(), Arm64DarwinNetworkConnectionLifecycleError> {
    let offset = u32::try_from(DarwinNetworkOwnerAbiSchema::ARM64_DARWIN.offset(field))
        .map_err(|_| Arm64DarwinNetworkConnectionLifecycleError::ContractLayout)?;
    code.append(Arm64Instruction::LoadUnsigned {
        size: Arm64LoadStoreSize::Double,
        destination: Arm64DataRegister::General(destination),
        base: Arm64BaseRegister::General(owner),
        offset,
    });
    Ok(())
}

fn immediate(
    code: &mut Arm64CodeBuilder,
    destination: Arm64Register,
    value: u64,
) -> Result<(), Arm64DarwinNetworkConnectionLifecycleError> {
    let immediate = u16::try_from(value)
        .map_err(|_| Arm64DarwinNetworkConnectionLifecycleError::ContractLayout)?;
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits64,
        operation: crate::Arm64MoveWide::Zero,
        destination,
        immediate,
        shift: 0,
    });
    Ok(())
}

fn adjust_stack(code: &mut Arm64CodeBuilder, operation: Arm64AddSubtract) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation,
        set_flags: false,
        destination: Arm64AddSubtractDestination::StackPointer,
        source: Arm64BaseRegister::StackPointer,
        immediate: 16,
        shift_12: false,
    });
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

fn call_import(code: &mut Arm64CodeBuilder, target: crate::Arm64FunctionImportId) {
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
pub enum Arm64DarwinNetworkConnectionLifecycleError {
    ContractLayout,
    Owner(Arm64DarwinNetworkOwnerError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinNetworkConnectionLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 Darwin connection lifecycle failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64DarwinNetworkConnectionLifecycleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Owner(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout => None,
        }
    }
}

impl From<Arm64DarwinNetworkOwnerError> for Arm64DarwinNetworkConnectionLifecycleError {
    fn from(error: Arm64DarwinNetworkOwnerError) -> Self {
        Self::Owner(error)
    }
}

impl From<Arm64CodeError> for Arm64DarwinNetworkConnectionLifecycleError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinNetworkConnectionLifecycleError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use super::add_darwin_network_connection_lifecycle_targets;
    use crate::{Arm64DarwinNetworkAdapterImports, Arm64ProgramBuilder};

    #[test]
    fn lifecycle_targets_are_one_complete_distinct_set() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        let targets =
            add_darwin_network_connection_lifecycle_targets(&mut program, &imports).unwrap();
        let functions = [
            targets.start(),
            targets.request_cancel(),
            targets.observe_final_state(),
            targets.complete_release_barrier(),
            targets.release(),
        ];
        for (index, function) in functions.iter().enumerate() {
            assert!(!functions[..index].contains(function));
        }
    }
}
