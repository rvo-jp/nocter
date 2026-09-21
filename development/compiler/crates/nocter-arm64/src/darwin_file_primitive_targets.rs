use std::collections::BTreeSet;
use std::fmt;

use nocter_runtime_contract::{
    DarwinFileAccess, DarwinFileCompletionAbiSchema, DarwinFileCompletionField,
    DarwinFileOperation, DarwinFileSeekOrigin, PrimitiveRole,
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
    Open(DarwinFileAccess),
    Read,
    ReadDirectory,
    Write,
    Flush,
    Seek(DarwinFileSeekOrigin),
    Truncate,
    ReadAt,
    WriteAt,
    Close,
    Identity,
    RemoveFile,
    Rename,
    CreateDirectory,
    RemoveDirectory,
    Metadata,
    SymlinkMetadata,
    CreateSymlink,
    ReadLink,
    Canonicalize,
    OwnerDispose,
    CompletionTakeOwner,
    CompletionTransferredByteCount,
    CompletionResultPosition,
    CompletionMetadataKind,
    CompletionMetadataLength,
    CompletionMetadataModifiedSeconds,
    CompletionMetadataModifiedNanoseconds,
    CompletionIdentityDevice,
    CompletionIdentityInode,
    CompletionFailureKind,
    CompletionFailureErrno,
    CompletionDispose,
}

/// Register-only Nocter ABI consumed by one generated local-file adapter.
///
/// This is the generated target's side of the ABI contract. Instruction selection compares the
/// machine program against it instead of maintaining a second role-to-shape table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Arm64DarwinFileCallAbi {
    argument_words: &'static [u8],
    result_words: u8,
}

impl Arm64DarwinFileCallAbi {
    #[must_use]
    pub(crate) const fn argument_words(self) -> &'static [u8] {
        self.argument_words
    }

    #[must_use]
    pub(crate) const fn result_words(self) -> u8 {
        self.result_words
    }
}

impl Arm64DarwinFilePrimitive {
    pub(crate) const fn from_role(role: PrimitiveRole) -> Option<Self> {
        match role {
            PrimitiveRole::FileOpenRead => Some(Self::Open(DarwinFileAccess::Read)),
            PrimitiveRole::FileOpenCreate => Some(Self::Open(DarwinFileAccess::Create)),
            PrimitiveRole::FileOpenCreateNew => Some(Self::Open(DarwinFileAccess::CreateNew)),
            PrimitiveRole::FileOpenAppend => Some(Self::Open(DarwinFileAccess::Append)),
            PrimitiveRole::FileOpenCopyDestination => {
                Some(Self::Open(DarwinFileAccess::CopyDestination))
            }
            PrimitiveRole::DirectoryOpen => Some(Self::Open(DarwinFileAccess::Directory)),
            PrimitiveRole::FileRead => Some(Self::Read),
            PrimitiveRole::DirectoryRead => Some(Self::ReadDirectory),
            PrimitiveRole::FileWrite => Some(Self::Write),
            PrimitiveRole::FileFlush => Some(Self::Flush),
            PrimitiveRole::FileSeekStart => Some(Self::Seek(DarwinFileSeekOrigin::Start)),
            PrimitiveRole::FileSeekEnd => Some(Self::Seek(DarwinFileSeekOrigin::End)),
            PrimitiveRole::FileSeekCurrent => Some(Self::Seek(DarwinFileSeekOrigin::Current)),
            PrimitiveRole::FileTruncate => Some(Self::Truncate),
            PrimitiveRole::FileReadAt => Some(Self::ReadAt),
            PrimitiveRole::FileWriteAt => Some(Self::WriteAt),
            PrimitiveRole::FileClose => Some(Self::Close),
            PrimitiveRole::FileIdentity => Some(Self::Identity),
            PrimitiveRole::FilesystemRemoveFile => Some(Self::RemoveFile),
            PrimitiveRole::FilesystemRename => Some(Self::Rename),
            PrimitiveRole::FilesystemCreateDirectory => Some(Self::CreateDirectory),
            PrimitiveRole::FilesystemRemoveDirectory => Some(Self::RemoveDirectory),
            PrimitiveRole::FilesystemMetadata => Some(Self::Metadata),
            PrimitiveRole::FilesystemSymlinkMetadata => Some(Self::SymlinkMetadata),
            PrimitiveRole::FilesystemCreateSymlink => Some(Self::CreateSymlink),
            PrimitiveRole::FilesystemReadLink => Some(Self::ReadLink),
            PrimitiveRole::FilesystemCanonicalize => Some(Self::Canonicalize),
            PrimitiveRole::FileOwnerDispose => Some(Self::OwnerDispose),
            PrimitiveRole::FileCompletionTakeOwner => Some(Self::CompletionTakeOwner),
            PrimitiveRole::FileCompletionTransferredByteCount => {
                Some(Self::CompletionTransferredByteCount)
            }
            PrimitiveRole::FileCompletionResultPosition => Some(Self::CompletionResultPosition),
            PrimitiveRole::FileCompletionMetadataKind => Some(Self::CompletionMetadataKind),
            PrimitiveRole::FileCompletionMetadataLength => Some(Self::CompletionMetadataLength),
            PrimitiveRole::FileCompletionMetadataModifiedSeconds => {
                Some(Self::CompletionMetadataModifiedSeconds)
            }
            PrimitiveRole::FileCompletionMetadataModifiedNanoseconds => {
                Some(Self::CompletionMetadataModifiedNanoseconds)
            }
            PrimitiveRole::FileCompletionIdentityDevice => Some(Self::CompletionIdentityDevice),
            PrimitiveRole::FileCompletionIdentityInode => Some(Self::CompletionIdentityInode),
            PrimitiveRole::FileCompletionFailureKind => Some(Self::CompletionFailureKind),
            PrimitiveRole::FileCompletionFailureErrno => Some(Self::CompletionFailureErrno),
            PrimitiveRole::FileCompletionDispose => Some(Self::CompletionDispose),
            _ => None,
        }
    }

    const fn operation(self) -> Option<DarwinFileOperation> {
        match self {
            Self::Read => Some(DarwinFileOperation::Read),
            Self::ReadDirectory => Some(DarwinFileOperation::ReadDirectory),
            Self::Write => Some(DarwinFileOperation::Write),
            Self::Flush => Some(DarwinFileOperation::Flush),
            Self::Truncate => Some(DarwinFileOperation::Truncate),
            Self::ReadAt => Some(DarwinFileOperation::ReadAt),
            Self::WriteAt => Some(DarwinFileOperation::WriteAt),
            Self::RemoveFile => Some(DarwinFileOperation::RemoveFile),
            Self::Rename => Some(DarwinFileOperation::Rename),
            Self::CreateDirectory => Some(DarwinFileOperation::CreateDirectory),
            Self::RemoveDirectory => Some(DarwinFileOperation::RemoveDirectory),
            Self::Metadata => Some(DarwinFileOperation::Metadata),
            Self::SymlinkMetadata => Some(DarwinFileOperation::SymlinkMetadata),
            Self::CreateSymlink => Some(DarwinFileOperation::CreateSymlink),
            Self::ReadLink => Some(DarwinFileOperation::ReadLink),
            Self::Canonicalize => Some(DarwinFileOperation::Canonicalize),
            Self::Identity => Some(DarwinFileOperation::Identity),
            Self::Open(_)
            | Self::Seek(_)
            | Self::Close
            | Self::OwnerDispose
            | Self::CompletionTakeOwner
            | Self::CompletionTransferredByteCount
            | Self::CompletionResultPosition
            | Self::CompletionMetadataKind
            | Self::CompletionMetadataLength
            | Self::CompletionMetadataModifiedSeconds
            | Self::CompletionMetadataModifiedNanoseconds
            | Self::CompletionIdentityDevice
            | Self::CompletionIdentityInode
            | Self::CompletionFailureKind
            | Self::CompletionFailureErrno
            | Self::CompletionDispose => None,
        }
    }

    /// Returns the exact direct-register shape expected by this generated adapter.
    #[must_use]
    pub(crate) fn call_abi(self) -> Arm64DarwinFileCallAbi {
        let (argument_words, result_words) = match self {
            Self::Open(_)
            | Self::RemoveFile
            | Self::CreateDirectory
            | Self::RemoveDirectory
            | Self::Metadata
            | Self::SymlinkMetadata => (&[2][..], 1),
            Self::Read | Self::ReadDirectory | Self::Write => (&[1, 2][..], 1),
            Self::Flush
            | Self::Identity
            | Self::Close
            | Self::CompletionTakeOwner
            | Self::CompletionTransferredByteCount
            | Self::CompletionResultPosition
            | Self::CompletionMetadataKind
            | Self::CompletionMetadataLength
            | Self::CompletionMetadataModifiedSeconds
            | Self::CompletionMetadataModifiedNanoseconds
            | Self::CompletionIdentityDevice
            | Self::CompletionIdentityInode
            | Self::CompletionFailureKind
            | Self::CompletionFailureErrno => (&[1][..], 1),
            Self::Seek(_) | Self::Truncate => (&[1, 1][..], 1),
            Self::ReadAt | Self::WriteAt => (&[1, 2, 1][..], 1),
            Self::Rename | Self::CreateSymlink | Self::ReadLink | Self::Canonicalize => {
                (&[2, 2][..], 1)
            }
            Self::OwnerDispose | Self::CompletionDispose => (&[1][..], 0),
        };
        Arm64DarwinFileCallAbi {
            argument_words,
            result_words,
        }
    }
}

/// Complete target family behind the source-visible local-file primitive contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinFilePrimitiveTargets {
    root: Arm64DarwinFileServiceRootTargets,
    retirement: Arm64DarwinFileRetirementTargets,
    jobs: Arm64DarwinFileJobTargets,
    open: [Arm64FunctionId; DarwinFileAccess::ALL.len()],
    seek: [Arm64FunctionId; DarwinFileSeekOrigin::ALL.len()],
    owner_dispose: Arm64FunctionId,
    completion_take_owner: Arm64FunctionId,
    completion_transferred_byte_count: Arm64FunctionId,
    completion_result_position: Arm64FunctionId,
    completion_metadata_kind: Arm64FunctionId,
    completion_metadata_length: Arm64FunctionId,
    completion_metadata_modified_seconds: Arm64FunctionId,
    completion_metadata_modified_nanoseconds: Arm64FunctionId,
    completion_identity_device: Arm64FunctionId,
    completion_identity_inode: Arm64FunctionId,
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
        let open = declare_open_adapters(program, jobs)?;
        let seek = [
            declare_seek_adapter(
                program,
                jobs.constructor(DarwinFileOperation::Seek),
                DarwinFileSeekOrigin::Start,
            )?,
            declare_seek_adapter(
                program,
                jobs.constructor(DarwinFileOperation::Seek),
                DarwinFileSeekOrigin::End,
            )?,
            declare_seek_adapter(
                program,
                jobs.constructor(DarwinFileOperation::Seek),
                DarwinFileSeekOrigin::Current,
            )?,
        ];
        let owner_dispose = declare_owner_dispose(program, retirement.drop_owner())?;
        let completion_take_owner = declare_completion_take_owner(program)?;
        let completion_transferred_byte_count =
            declare_completion_reader(program, DarwinFileCompletionField::TransferredByteCount)?;
        let completion_result_position =
            declare_completion_reader(program, DarwinFileCompletionField::ResultPosition)?;
        let completion_metadata_kind =
            declare_completion_reader(program, DarwinFileCompletionField::MetadataKind)?;
        let completion_metadata_length =
            declare_completion_reader(program, DarwinFileCompletionField::MetadataLength)?;
        let completion_metadata_modified_seconds =
            declare_completion_reader(program, DarwinFileCompletionField::MetadataModifiedSeconds)?;
        let completion_metadata_modified_nanoseconds = declare_completion_reader(
            program,
            DarwinFileCompletionField::MetadataModifiedNanoseconds,
        )?;
        let completion_identity_device =
            declare_completion_reader(program, DarwinFileCompletionField::IdentityDevice)?;
        let completion_identity_inode =
            declare_completion_reader(program, DarwinFileCompletionField::IdentityInode)?;
        let completion_failure_kind =
            declare_completion_reader(program, DarwinFileCompletionField::FailureKind)?;
        let completion_failure_errno =
            declare_completion_reader(program, DarwinFileCompletionField::FailureErrno)?;
        let completion_dispose = declare_completion_dispose(program, retirement.drop_owner())?;
        Ok(Some(Self {
            root,
            retirement,
            jobs,
            open,
            seek,
            owner_dispose,
            completion_take_owner,
            completion_transferred_byte_count,
            completion_result_position,
            completion_metadata_kind,
            completion_metadata_length,
            completion_metadata_modified_seconds,
            completion_metadata_modified_nanoseconds,
            completion_identity_device,
            completion_identity_inode,
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
        match primitive {
            Arm64DarwinFilePrimitive::Open(access) => return self.open[access.code() as usize],
            Arm64DarwinFilePrimitive::Seek(origin) => return self.seek[origin.code() as usize],
            _ => {}
        }
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
            Arm64DarwinFilePrimitive::CompletionMetadataKind => self.completion_metadata_kind,
            Arm64DarwinFilePrimitive::CompletionMetadataLength => self.completion_metadata_length,
            Arm64DarwinFilePrimitive::CompletionMetadataModifiedSeconds => {
                self.completion_metadata_modified_seconds
            }
            Arm64DarwinFilePrimitive::CompletionMetadataModifiedNanoseconds => {
                self.completion_metadata_modified_nanoseconds
            }
            Arm64DarwinFilePrimitive::CompletionIdentityDevice => self.completion_identity_device,
            Arm64DarwinFilePrimitive::CompletionIdentityInode => self.completion_identity_inode,
            Arm64DarwinFilePrimitive::CompletionFailureKind => self.completion_failure_kind,
            Arm64DarwinFilePrimitive::CompletionFailureErrno => self.completion_failure_errno,
            Arm64DarwinFilePrimitive::CompletionDispose => self.completion_dispose,
            Arm64DarwinFilePrimitive::Read
            | Arm64DarwinFilePrimitive::ReadDirectory
            | Arm64DarwinFilePrimitive::Write
            | Arm64DarwinFilePrimitive::Flush
            | Arm64DarwinFilePrimitive::Truncate
            | Arm64DarwinFilePrimitive::ReadAt
            | Arm64DarwinFilePrimitive::WriteAt
            | Arm64DarwinFilePrimitive::Identity
            | Arm64DarwinFilePrimitive::RemoveFile
            | Arm64DarwinFilePrimitive::Rename
            | Arm64DarwinFilePrimitive::CreateDirectory
            | Arm64DarwinFilePrimitive::RemoveDirectory
            | Arm64DarwinFilePrimitive::Metadata
            | Arm64DarwinFilePrimitive::SymlinkMetadata
            | Arm64DarwinFilePrimitive::CreateSymlink
            | Arm64DarwinFilePrimitive::ReadLink
            | Arm64DarwinFilePrimitive::Canonicalize => unreachable!("operation handled above"),
            Arm64DarwinFilePrimitive::Open(_) | Arm64DarwinFilePrimitive::Seek(_) => {
                unreachable!("semantic adapter handled above")
            }
        }
    }
}

fn declare_open_adapters(
    program: &mut Arm64ProgramBuilder,
    jobs: Arm64DarwinFileJobTargets,
) -> Result<[Arm64FunctionId; DarwinFileAccess::ALL.len()], Arm64DarwinFilePrimitiveError> {
    let constructor = jobs.constructor(DarwinFileOperation::Open);
    Ok([
        declare_open_adapter(program, constructor, DarwinFileAccess::Read)?,
        declare_open_adapter(program, constructor, DarwinFileAccess::Create)?,
        declare_open_adapter(program, constructor, DarwinFileAccess::CreateNew)?,
        declare_open_adapter(program, constructor, DarwinFileAccess::Append)?,
        declare_open_adapter(program, constructor, DarwinFileAccess::Directory)?,
        declare_open_adapter(program, constructor, DarwinFileAccess::CopyDestination)?,
    ])
}

fn declare_open_adapter(
    program: &mut Arm64ProgramBuilder,
    constructor: Arm64FunctionId,
    access: DarwinFileAccess,
) -> Result<Arm64FunctionId, Arm64DarwinFilePrimitiveError> {
    let mut code = adapter_prologue();
    crate::darwin_file_job_code::immediate(&mut code, x(2), u64::from(access.code()));
    code.call(constructor);
    adapter_epilogue(&mut code);
    declare(program, code)
}

fn declare_seek_adapter(
    program: &mut Arm64ProgramBuilder,
    constructor: Arm64FunctionId,
    origin: DarwinFileSeekOrigin,
) -> Result<Arm64FunctionId, Arm64DarwinFilePrimitiveError> {
    let mut code = adapter_prologue();
    move_register(&mut code, x(2), x(1));
    crate::darwin_file_job_code::immediate(&mut code, x(1), u64::from(origin.code()));
    code.call(constructor);
    adapter_epilogue(&mut code);
    declare(program, code)
}

fn declare_owner_dispose(
    program: &mut Arm64ProgramBuilder,
    drop_owner: Arm64FunctionId,
) -> Result<Arm64FunctionId, Arm64DarwinFilePrimitiveError> {
    let mut code = adapter_prologue();
    load(&mut code, x(19), x(0), 0);
    store_zero(&mut code, x(0), 0, x(8));
    call_if_nonzero(&mut code, x(19), drop_owner)?;
    adapter_epilogue(&mut code);
    declare(program, code)
}

fn declare_completion_take_owner(
    program: &mut Arm64ProgramBuilder,
) -> Result<Arm64FunctionId, Arm64DarwinFilePrimitiveError> {
    declare(program, build_completion_take_owner())
}

fn build_completion_take_owner() -> Arm64CodeBuilder {
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
        x(9),
    );
    move_register(&mut code, x(0), x(8));
    ret(&mut code);
    code
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
    store_zero(&mut code, x(0), offset, x(8));
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

fn store_zero(
    code: &mut Arm64CodeBuilder,
    base: Arm64Register,
    offset: u64,
    scratch: Arm64Register,
) {
    debug_assert_ne!(base, scratch);
    crate::frame_access::load_immediate(code, scratch, 0, crate::Arm64DataSize::Bits64);
    crate::address_code::store_native(code, Arm64LoadStoreSize::Double, scratch, base, offset);
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

#[cfg(test)]
mod tests {
    use super::Arm64DarwinFilePrimitive as Primitive;

    #[test]
    fn generated_file_adapter_abis_cover_every_operation_shape() {
        for (primitive, arguments, result) in [
            (
                Primitive::Open(nocter_runtime_contract::DarwinFileAccess::Read),
                &[2][..],
                1,
            ),
            (
                Primitive::Open(nocter_runtime_contract::DarwinFileAccess::Directory),
                &[2][..],
                1,
            ),
            (
                Primitive::Open(nocter_runtime_contract::DarwinFileAccess::CopyDestination),
                &[2][..],
                1,
            ),
            (Primitive::Read, &[1, 2][..], 1),
            (Primitive::ReadDirectory, &[1, 2][..], 1),
            (Primitive::Write, &[1, 2][..], 1),
            (Primitive::Flush, &[1][..], 1),
            (Primitive::Identity, &[1][..], 1),
            (
                Primitive::Seek(nocter_runtime_contract::DarwinFileSeekOrigin::Start),
                &[1, 1][..],
                1,
            ),
            (Primitive::Truncate, &[1, 1][..], 1),
            (Primitive::ReadAt, &[1, 2, 1][..], 1),
            (Primitive::WriteAt, &[1, 2, 1][..], 1),
            (Primitive::Close, &[1][..], 1),
            (Primitive::RemoveFile, &[2][..], 1),
            (Primitive::Rename, &[2, 2][..], 1),
            (Primitive::CreateDirectory, &[2][..], 1),
            (Primitive::RemoveDirectory, &[2][..], 1),
            (Primitive::Metadata, &[2][..], 1),
            (Primitive::SymlinkMetadata, &[2][..], 1),
            (Primitive::CreateSymlink, &[2, 2][..], 1),
            (Primitive::ReadLink, &[2, 2][..], 1),
            (Primitive::Canonicalize, &[2, 2][..], 1),
            (Primitive::CompletionIdentityDevice, &[1][..], 1),
            (Primitive::CompletionIdentityInode, &[1][..], 1),
        ] {
            let abi = primitive.call_abi();
            assert_eq!(abi.argument_words(), arguments);
            assert_eq!(abi.result_words(), result);
        }
    }

    #[test]
    fn completion_owner_transfer_clears_storage_without_clobbering_the_result() {
        let code = super::build_completion_take_owner().finish().unwrap();
        let words = code
            .bytes()
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();

        assert_eq!(
            words,
            [
                0xf940_0008, // ldr x8, [x0]
                0xd280_0009, // mov x9, #0
                0xf900_0009, // str x9, [x0]
                0x9100_0100, // mov x0, x8
                0xd61f_03c0, // br x30
            ]
        );
    }
}
