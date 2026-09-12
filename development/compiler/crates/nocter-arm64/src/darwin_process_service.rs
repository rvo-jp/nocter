use std::fmt;

use nocter_runtime_contract::{DarwinProcessServiceFunction, PrimitiveRole};

use crate::darwin_kernel_abi::{
    DarwinErrorAbi, DarwinProcessAbi, DarwinSystemCall, emit_system_call,
};
use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64CodeError, Arm64DataRegister, Arm64DataSize, Arm64FunctionId,
    Arm64FunctionImportId, Arm64Instruction, Arm64LoadStoreSize, Arm64ProgramBuilder,
    Arm64ProgramError, Arm64Register,
};

/// Complete typed loader dependencies for the Darwin child-abandonment service.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Arm64DarwinProcessServiceImports {
    functions: Box<[(DarwinProcessServiceFunction, Arm64FunctionImportId)]>,
}

impl Arm64DarwinProcessServiceImports {
    fn declare(program: &mut Arm64ProgramBuilder) -> Result<Self, Arm64ProgramError> {
        let functions = DarwinProcessServiceFunction::ALL
            .iter()
            .copied()
            .map(|role| Ok((role, program.add_function_import(role.import())?)))
            .collect::<Result<Vec<_>, Arm64ProgramError>>()?
            .into_boxed_slice();
        Ok(Self { functions })
    }

    fn function(&self, role: DarwinProcessServiceFunction) -> Arm64FunctionImportId {
        self.functions
            .iter()
            .find_map(|(candidate, target)| (*candidate == role).then_some(*target))
            .expect("complete Darwin process service function catalog")
    }
}

/// Compiler-owned entry points required by process lifecycle primitives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinProcessServiceTargets {
    abandon: Arm64FunctionId,
    worker: Arm64FunctionId,
}

impl Arm64DarwinProcessServiceTargets {
    pub(crate) fn declare(
        roles: &std::collections::BTreeSet<PrimitiveRole>,
        program: &mut Arm64ProgramBuilder,
    ) -> Result<Option<Self>, Arm64DarwinProcessServiceError> {
        if !roles.contains(&PrimitiveRole::ProcessAbandon) {
            return Ok(None);
        }
        let imports = Arm64DarwinProcessServiceImports::declare(program)?;
        let abandon = program.declare_function();
        let worker = program.declare_function();
        program.define_function(worker, abandonment_worker(&imports)?.finish()?)?;
        program.define_function(abandon, abandonment_entry(&imports, worker)?.finish()?)?;
        Ok(Some(Self { abandon, worker }))
    }

    #[must_use]
    pub const fn abandon(self) -> Arm64FunctionId {
        self.abandon
    }

    #[must_use]
    pub const fn worker(self) -> Arm64FunctionId {
        self.worker
    }
}

/// Kills a live child synchronously, then transfers terminal reaping to a private worker.
///
/// The kill-before-transfer order is deliberate: even if the generated process exits before the
/// dispatch worker runs, it cannot leave a live child behind. The operating system adopts and
/// eventually reaps the already terminated child in that shutdown case.
fn abandonment_entry(
    imports: &Arm64DarwinProcessServiceImports,
    worker: Arm64FunctionId,
) -> Result<Arm64CodeBuilder, Arm64DarwinProcessServiceError> {
    const FRAME_SIZE: u16 = 32;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code, FRAME_SIZE);
    move_register(&mut code, x(19), x(0));

    compare_immediate(&mut code, x(19), 0);
    let valid_nonzero = code.create_label();
    code.branch_conditional(valid_nonzero, Arm64BranchCondition::NotEqual);
    abort(&mut code, imports);
    code.bind(valid_nonzero)?;
    crate::frame_access::load_immediate(
        &mut code,
        x(8),
        DarwinProcessAbi::MAX_PROCESS_ID,
        Arm64DataSize::Bits64,
    );
    compare_registers(&mut code, x(19), x(8));
    let valid_range = code.create_label();
    code.branch_conditional(valid_range, Arm64BranchCondition::UnsignedLowerOrSame);
    abort(&mut code, imports);
    code.bind(valid_range)?;

    let kill = code.create_label();
    let transfer = code.create_label();
    code.bind(kill)?;
    move_register(&mut code, x(0), x(19));
    immediate(&mut code, x(1), DarwinProcessAbi::KILL_SIGNAL);
    emit_system_call(&mut code, DarwinSystemCall::Kill);
    code.branch_conditional(transfer, Arm64BranchCondition::CarryClear);
    compare_immediate(&mut code, x(0), DarwinErrorAbi::INTERRUPTED);
    code.branch_conditional(kill, Arm64BranchCondition::Equal);
    compare_immediate(&mut code, x(0), DarwinErrorAbi::MISSING_PROCESS);
    code.branch_conditional(transfer, Arm64BranchCondition::Equal);
    abort(&mut code, imports);

    code.bind(transfer)?;
    immediate(&mut code, x(0), 8);
    call_import(
        &mut code,
        imports.function(DarwinProcessServiceFunction::Malloc),
    );
    let allocated = code.create_label();
    compare_immediate(&mut code, x(0), 0);
    code.branch_conditional(allocated, Arm64BranchCondition::NotEqual);
    abort(&mut code, imports);
    code.bind(allocated)?;
    move_register(&mut code, x(20), x(0));
    store_at(&mut code, x(20), 0, x(19));

    immediate(&mut code, x(0), 0);
    immediate(&mut code, x(1), 0);
    call_import(
        &mut code,
        imports.function(DarwinProcessServiceFunction::DispatchGetGlobalQueue),
    );
    let queue_available = code.create_label();
    compare_immediate(&mut code, x(0), 0);
    code.branch_conditional(queue_available, Arm64BranchCondition::NotEqual);
    abort(&mut code, imports);
    code.bind(queue_available)?;
    move_register(&mut code, x(1), x(20));
    code.load_function_address(worker, x(2));
    call_import(
        &mut code,
        imports.function(DarwinProcessServiceFunction::DispatchAsyncFunction),
    );

    epilogue(&mut code, FRAME_SIZE);
    Ok(code)
}

/// Blocks only on the private dispatch queue until the exact killed child has been reaped.
fn abandonment_worker(
    imports: &Arm64DarwinProcessServiceImports,
) -> Result<Arm64CodeBuilder, Arm64DarwinProcessServiceError> {
    const FRAME_SIZE: u16 = 32;
    let mut code = Arm64CodeBuilder::new();
    prologue(&mut code, FRAME_SIZE);
    move_register(&mut code, x(19), x(0));
    load_at(&mut code, x(20), x(19), 0);

    let wait = code.create_label();
    let returned = code.create_label();
    code.bind(wait)?;
    move_register(&mut code, x(0), x(20));
    immediate(&mut code, x(1), 0);
    immediate(&mut code, x(2), 0);
    immediate(&mut code, x(3), 0);
    emit_system_call(&mut code, DarwinSystemCall::Wait4);
    code.branch_conditional(returned, Arm64BranchCondition::CarryClear);
    compare_immediate(&mut code, x(0), DarwinErrorAbi::INTERRUPTED);
    code.branch_conditional(wait, Arm64BranchCondition::Equal);
    abort(&mut code, imports);
    code.bind(returned)?;
    compare_registers(&mut code, x(0), x(20));
    let exact_child = code.create_label();
    code.branch_conditional(exact_child, Arm64BranchCondition::Equal);
    abort(&mut code, imports);
    code.bind(exact_child)?;

    move_register(&mut code, x(0), x(19));
    call_import(
        &mut code,
        imports.function(DarwinProcessServiceFunction::Free),
    );
    epilogue(&mut code, FRAME_SIZE);
    Ok(code)
}

fn prologue(code: &mut Arm64CodeBuilder, frame_size: u16) {
    adjust_stack(code, Arm64AddSubtract::Subtract, frame_size);
    store_stack(code, x(19), 0);
    store_stack(code, x(20), 8);
    store_stack(code, x(30), 24);
}

fn epilogue(code: &mut Arm64CodeBuilder, frame_size: u16) {
    load_stack(code, x(19), 0);
    load_stack(code, x(20), 8);
    load_stack(code, x(30), 24);
    adjust_stack(code, Arm64AddSubtract::Add, frame_size);
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
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

fn immediate(code: &mut Arm64CodeBuilder, destination: Arm64Register, value: u64) {
    crate::frame_access::load_immediate(code, destination, value, Arm64DataSize::Bits64);
}

fn compare_immediate(code: &mut Arm64CodeBuilder, value: Arm64Register, expected: u64) {
    immediate(code, x(8), expected);
    compare_registers(code, value, x(8));
}

fn compare_registers(code: &mut Arm64CodeBuilder, left: Arm64Register, right: Arm64Register) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(left),
        right: Arm64DataRegister::General(right),
    });
}

fn store_at(code: &mut Arm64CodeBuilder, base: Arm64Register, offset: u32, source: Arm64Register) {
    code.append(Arm64Instruction::StoreUnsigned {
        size: Arm64LoadStoreSize::Double,
        source: Arm64DataRegister::General(source),
        base: Arm64BaseRegister::General(base),
        offset,
    });
}

fn load_at(
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

fn call_import(code: &mut Arm64CodeBuilder, target: Arm64FunctionImportId) {
    code.load_function_import(target, x(16));
    code.append(Arm64Instruction::BranchRegister {
        target: x(16),
        link: true,
    });
}

fn abort(code: &mut Arm64CodeBuilder, imports: &Arm64DarwinProcessServiceImports) {
    call_import(code, imports.function(DarwinProcessServiceFunction::Abort));
}

fn x(number: u8) -> Arm64Register {
    Arm64Register::new(number).expect("closed ARM64 register is valid")
}

#[derive(Debug)]
pub enum Arm64DarwinProcessServiceError {
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinProcessServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin process service failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinProcessServiceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
        }
    }
}

impl From<Arm64CodeError> for Arm64DarwinProcessServiceError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinProcessServiceError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::{DarwinProcessServiceFunction, RuntimeImport};

    use super::Arm64DarwinProcessServiceImports;
    use crate::{Arm64CodeBuilder, Arm64Instruction, Arm64ProgramBuilder, Arm64Register};

    #[test]
    fn process_service_imports_are_complete_and_deduplicated() {
        let mut program = Arm64ProgramBuilder::new();
        let first = Arm64DarwinProcessServiceImports::declare(&mut program).unwrap();
        let second = Arm64DarwinProcessServiceImports::declare(&mut program).unwrap();
        assert_eq!(first, second);

        let entry = program.declare_function();
        let mut code = Arm64CodeBuilder::new();
        code.append(Arm64Instruction::BranchRegister {
            target: Arm64Register::new(30).unwrap(),
            link: false,
        });
        program
            .define_function(entry, code.finish().unwrap())
            .unwrap();
        program.set_entry(entry).unwrap();
        let program = program.finish().unwrap();
        let expected = DarwinProcessServiceFunction::ALL
            .iter()
            .copied()
            .map(|role| RuntimeImport::from(role.import()))
            .collect::<Vec<_>>();
        assert_eq!(
            program
                .runtime_imports()
                .iter()
                .map(|runtime_import| runtime_import.import().clone())
                .collect::<Vec<_>>(),
            expected,
        );
    }
}
