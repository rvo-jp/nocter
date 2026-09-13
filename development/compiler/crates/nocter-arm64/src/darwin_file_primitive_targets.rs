use std::collections::BTreeSet;
use std::fmt;

use nocter_runtime_contract::{
    DarwinFileCompletionAbiSchema, DarwinFileCompletionField, DarwinFileOperation, PrimitiveRole,
};

use crate::{
    Arm64AddSubtract, Arm64CodeBuilder, Arm64CodeError, Arm64DarwinFileJobError,
    Arm64DarwinFileJobTargets, Arm64DarwinFileRetirementError, Arm64DarwinFileRetirementTargets,
    Arm64DarwinFileServiceImports, Arm64DarwinFileServiceRootError,
    Arm64DarwinFileServiceRootTargets, Arm64FunctionId, Arm64Instruction, Arm64LoadStoreSize,
    Arm64ProgramBuilder, Arm64ProgramError, Arm64Register,
};

/// One source primitive implemented by the generated Darwin local-file service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64DarwinFilePrimitive {
    Open,
    Read,
    Write,
    Flush,
    Seek,
    Truncate,
    ReadAt,
    WriteAt,
    Close,
    OwnerDispose,
    CompletionTakeOwner,
    CompletionTransferredByteCount,
    CompletionResultPosition,
    CompletionFailureKind,
    CompletionFailureErrno,
    CompletionDispose,
}

impl Arm64DarwinFilePrimitive {
    pub(crate) const fn from_role(role: PrimitiveRole) -> Option<Self> {
        match role {
            PrimitiveRole::FileOpen => Some(Self::Open),
            PrimitiveRole::FileRead => Some(Self::Read),
            PrimitiveRole::FileWrite => Some(Self::Write),
            PrimitiveRole::FileFlush => Some(Self::Flush),
            PrimitiveRole::FileSeek => Some(Self::Seek),
            PrimitiveRole::FileTruncate => Some(Self::Truncate),
            PrimitiveRole::FileReadAt => Some(Self::ReadAt),
            PrimitiveRole::FileWriteAt => Some(Self::WriteAt),
            PrimitiveRole::FileClose => Some(Self::Close),
            PrimitiveRole::FileOwnerDispose => Some(Self::OwnerDispose),
            PrimitiveRole::FileCompletionTakeOwner => Some(Self::CompletionTakeOwner),
            PrimitiveRole::FileCompletionTransferredByteCount => {
                Some(Self::CompletionTransferredByteCount)
            }
            PrimitiveRole::FileCompletionResultPosition => Some(Self::CompletionResultPosition),
            PrimitiveRole::FileCompletionFailureKind => Some(Self::CompletionFailureKind),
            PrimitiveRole::FileCompletionFailureErrno => Some(Self::CompletionFailureErrno),
            PrimitiveRole::FileCompletionDispose => Some(Self::CompletionDispose),
            _ => None,
        }
    }

    const fn operation(self) -> Option<DarwinFileOperation> {
        match self {
            Self::Open => Some(DarwinFileOperation::Open),
            Self::Read => Some(DarwinFileOperation::Read),
            Self::Write => Some(DarwinFileOperation::Write),
            Self::Flush => Some(DarwinFileOperation::Flush),
            Self::Seek => Some(DarwinFileOperation::Seek),
            Self::Truncate => Some(DarwinFileOperation::Truncate),
            Self::ReadAt => Some(DarwinFileOperation::ReadAt),
            Self::WriteAt => Some(DarwinFileOperation::WriteAt),
            Self::Close
            | Self::OwnerDispose
            | Self::CompletionTakeOwner
            | Self::CompletionTransferredByteCount
            | Self::CompletionResultPosition
            | Self::CompletionFailureKind
            | Self::CompletionFailureErrno
            | Self::CompletionDispose => None,
        }
    }
}

/// Complete target family behind the source-visible local-file primitive contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinFilePrimitiveTargets {
    root: Arm64DarwinFileServiceRootTargets,
    retirement: Arm64DarwinFileRetirementTargets,
    jobs: Arm64DarwinFileJobTargets,
    owner_dispose: Arm64FunctionId,
    completion_take_owner: Arm64FunctionId,
    completion_transferred_byte_count: Arm64FunctionId,
    completion_result_position: Arm64FunctionId,
    completion_failure_kind: Arm64FunctionId,
    completion_failure_errno: Arm64FunctionId,
    completion_dispose: Arm64FunctionId,
}

impl Arm64DarwinFilePrimitiveTargets {
    pub(crate) fn declare(
        roles: &BTreeSet<PrimitiveRole>,
        program: &mut Arm64ProgramBuilder,
    ) -> Result<Option<Self>, Arm64DarwinFilePrimitiveError> {
        if !roles
            .iter()
            .copied()
            .any(|role| Arm64DarwinFilePrimitive::from_role(role).is_some())
        {
            return Ok(None);
        }
        let imports = Arm64DarwinFileServiceImports::declare(program)?;
        let retirement = Arm64DarwinFileRetirementTargets::declare(program, &imports)?;
        let root = Arm64DarwinFileServiceRootTargets::declare(program, &imports, retirement)?;
        let jobs = Arm64DarwinFileJobTargets::declare(program, &imports, root, retirement)?;
        let owner_dispose = declare_owner_dispose(program, retirement.drop_owner())?;
        let completion_take_owner = declare_completion_take_owner(program)?;
        let completion_transferred_byte_count =
            declare_completion_reader(program, DarwinFileCompletionField::TransferredByteCount)?;
        let completion_result_position =
            declare_completion_reader(program, DarwinFileCompletionField::ResultPosition)?;
        let completion_failure_kind =
            declare_completion_reader(program, DarwinFileCompletionField::FailureKind)?;
        let completion_failure_errno =
            declare_completion_reader(program, DarwinFileCompletionField::FailureErrno)?;
        let completion_dispose = declare_completion_dispose(program, retirement.drop_owner())?;
        Ok(Some(Self {
            root,
            retirement,
            jobs,
            owner_dispose,
            completion_take_owner,
            completion_transferred_byte_count,
            completion_result_position,
            completion_failure_kind,
            completion_failure_errno,
            completion_dispose,
        }))
    }

    #[must_use]
    pub const fn root(self) -> Arm64DarwinFileServiceRootTargets {
        self.root
    }

    #[must_use]
    pub fn target(self, primitive: Arm64DarwinFilePrimitive) -> Arm64FunctionId {
        if let Some(operation) = primitive.operation() {
            return self.jobs.constructor(operation);
        }
        match primitive {
            Arm64DarwinFilePrimitive::Close => self.retirement.begin_close(),
            Arm64DarwinFilePrimitive::OwnerDispose => self.owner_dispose,
            Arm64DarwinFilePrimitive::CompletionTakeOwner => self.completion_take_owner,
            Arm64DarwinFilePrimitive::CompletionTransferredByteCount => {
                self.completion_transferred_byte_count
            }
            Arm64DarwinFilePrimitive::CompletionResultPosition => self.completion_result_position,
            Arm64DarwinFilePrimitive::CompletionFailureKind => self.completion_failure_kind,
            Arm64DarwinFilePrimitive::CompletionFailureErrno => self.completion_failure_errno,
            Arm64DarwinFilePrimitive::CompletionDispose => self.completion_dispose,
            Arm64DarwinFilePrimitive::Open
            | Arm64DarwinFilePrimitive::Read
            | Arm64DarwinFilePrimitive::Write
            | Arm64DarwinFilePrimitive::Flush
            | Arm64DarwinFilePrimitive::Seek
            | Arm64DarwinFilePrimitive::Truncate
            | Arm64DarwinFilePrimitive::ReadAt
            | Arm64DarwinFilePrimitive::WriteAt => unreachable!("operation handled above"),
        }
    }
}

fn declare_owner_dispose(
    program: &mut Arm64ProgramBuilder,
    drop_owner: Arm64FunctionId,
) -> Result<Arm64FunctionId, Arm64DarwinFilePrimitiveError> {
    let mut code = adapter_prologue();
    load(&mut code, x(19), x(0), 0);
    store_zero(&mut code, x(0), 0);
    call_if_nonzero(&mut code, x(19), drop_owner)?;
    adapter_epilogue(&mut code);
    declare(program, code)
}

fn declare_completion_take_owner(
    program: &mut Arm64ProgramBuilder,
) -> Result<Arm64FunctionId, Arm64DarwinFilePrimitiveError> {
    let schema = DarwinFileCompletionAbiSchema::ARM64_DARWIN;
    let mut code = Arm64CodeBuilder::new();
    load(
        &mut code,
        x(8),
        x(0),
        schema.offset(DarwinFileCompletionField::RetirementRecord),
    );
    store_zero(
        &mut code,
        x(0),
        schema.offset(DarwinFileCompletionField::RetirementRecord),
    );
    move_register(&mut code, x(0), x(8));
    ret(&mut code);
    declare(program, code)
}

fn declare_completion_reader(
    program: &mut Arm64ProgramBuilder,
    field: DarwinFileCompletionField,
) -> Result<Arm64FunctionId, Arm64DarwinFilePrimitiveError> {
    let mut code = Arm64CodeBuilder::new();
    load(
        &mut code,
        x(0),
        x(0),
        DarwinFileCompletionAbiSchema::ARM64_DARWIN.offset(field),
    );
    ret(&mut code);
    declare(program, code)
}

fn declare_completion_dispose(
    program: &mut Arm64ProgramBuilder,
    drop_owner: Arm64FunctionId,
) -> Result<Arm64FunctionId, Arm64DarwinFilePrimitiveError> {
    let offset = DarwinFileCompletionAbiSchema::ARM64_DARWIN
        .offset(DarwinFileCompletionField::RetirementRecord);
    let mut code = adapter_prologue();
    load(&mut code, x(19), x(0), offset);
    store_zero(&mut code, x(0), offset);
    call_if_nonzero(&mut code, x(19), drop_owner)?;
    adapter_epilogue(&mut code);
    declare(program, code)
}

fn call_if_nonzero(
    code: &mut Arm64CodeBuilder,
    owner: Arm64Register,
    target: Arm64FunctionId,
) -> Result<(), Arm64DarwinFilePrimitiveError> {
    let complete = code.create_label();
    crate::darwin_file_job_code::compare_immediate(code, owner, 0);
    code.branch_conditional(complete, crate::Arm64BranchCondition::Equal);
    move_register(code, x(0), owner);
    code.call(target);
    code.bind(complete)?;
    Ok(())
}

fn adapter_prologue() -> Arm64CodeBuilder {
    let mut code = Arm64CodeBuilder::new();
    crate::frame_access::adjust_stack(&mut code, 32, Arm64AddSubtract::Subtract);
    crate::frame_access::store_at_stack_offset(&mut code, Arm64LoadStoreSize::Double, x(19), 16);
    crate::frame_access::store_at_stack_offset(&mut code, Arm64LoadStoreSize::Double, x(30), 24);
    code
}

fn adapter_epilogue(code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_at_stack_offset(code, Arm64LoadStoreSize::Double, x(19), 16);
    crate::frame_access::load_at_stack_offset(code, Arm64LoadStoreSize::Double, x(30), 24);
    crate::frame_access::adjust_stack(code, 32, Arm64AddSubtract::Add);
    ret(code);
}

fn declare(
    program: &mut Arm64ProgramBuilder,
    code: Arm64CodeBuilder,
) -> Result<Arm64FunctionId, Arm64DarwinFilePrimitiveError> {
    let function = program.declare_function();
    program.define_function(function, code.finish()?)?;
    Ok(function)
}

fn load(code: &mut Arm64CodeBuilder, destination: Arm64Register, base: Arm64Register, offset: u64) {
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        destination,
        base,
        offset,
    );
}

fn store_zero(code: &mut Arm64CodeBuilder, base: Arm64Register, offset: u64) {
    crate::frame_access::load_immediate(code, x(8), 0, crate::Arm64DataSize::Bits64);
    crate::address_code::store_native(code, Arm64LoadStoreSize::Double, x(8), base, offset);
}

fn move_register(code: &mut Arm64CodeBuilder, destination: Arm64Register, source: Arm64Register) {
    crate::darwin_file_job_code::move_register(code, destination, source);
}

fn ret(code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::BranchRegister {
        target: x(30),
        link: false,
    });
}

fn x(index: u8) -> Arm64Register {
    Arm64Register::new(index).expect("closed generated file adapter register")
}

#[derive(Debug)]
pub enum Arm64DarwinFilePrimitiveError {
    RuntimeAbi,
    Code(Arm64CodeError),
    Job(Arm64DarwinFileJobError),
    Retirement(Arm64DarwinFileRetirementError),
    Root(Arm64DarwinFileServiceRootError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinFilePrimitiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin file primitive failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinFilePrimitiveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Code(error) => Some(error),
            Self::Job(error) => Some(error),
            Self::Retirement(error) => Some(error),
            Self::Root(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::RuntimeAbi => None,
        }
    }
}

impl From<Arm64CodeError> for Arm64DarwinFilePrimitiveError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64DarwinFileJobError> for Arm64DarwinFilePrimitiveError {
    fn from(error: Arm64DarwinFileJobError) -> Self {
        Self::Job(error)
    }
}

impl From<Arm64DarwinFileRetirementError> for Arm64DarwinFilePrimitiveError {
    fn from(error: Arm64DarwinFileRetirementError) -> Self {
        Self::Retirement(error)
    }
}

impl From<Arm64DarwinFileServiceRootError> for Arm64DarwinFilePrimitiveError {
    fn from(error: Arm64DarwinFileServiceRootError) -> Self {
        Self::Root(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinFilePrimitiveError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}
