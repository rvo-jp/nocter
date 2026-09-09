use std::fmt;

use nocter_runtime_contract::DarwinBlockAbiSchema;

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64DataId, Arm64DataImportId, Arm64DataRegister, Arm64DataSize, Arm64FunctionId,
    Arm64Instruction, Arm64LoadStoreSize, Arm64MoveWide, Arm64ProgramBuilder, Arm64ProgramError,
    Arm64Register,
};

/// One validated immutable descriptor for the admitted one-pointer Darwin block shape.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Arm64DarwinBlockDescriptorId(Arm64DataId);

impl Arm64DarwinBlockDescriptorId {
    const fn data(self) -> Arm64DataId {
        self.0
    }
}

/// Adds one immutable descriptor for a Darwin block with pointer-sized, non-owning captures.
///
/// The signature must be a NUL-terminated Objective-C type encoding admitted by the closed native
/// adapter. The descriptor stores the literal size and points at that signature. It deliberately
/// has no copy/dispose helpers: adapter blocks capture only a mailbox pointer whose lifetime is
/// owned independently by the adapter.
///
/// # Errors
///
/// Rejects an unrepresentable literal size or an invalid ARM64 data reference.
pub fn add_darwin_pointer_capture_block_descriptor(
    program: &mut Arm64ProgramBuilder,
    signature: &[u8],
) -> Result<Arm64DarwinBlockDescriptorId, Arm64DarwinBlockError> {
    if signature.len() <= 1
        || signature.last() != Some(&0)
        || signature[..signature.len() - 1].contains(&0)
    {
        return Err(Arm64DarwinBlockError::InvalidSignature);
    }
    let schema = DarwinBlockAbiSchema::ARM64_DARWIN;
    let literal_size = schema
        .literal_size(1)
        .ok_or(Arm64DarwinBlockError::LiteralSizeOverflow)?;
    let descriptor_size = usize::try_from(schema.descriptor_size())
        .map_err(|_| Arm64DarwinBlockError::DescriptorLayout)?;
    let mut descriptor = vec![0_u8; descriptor_size];
    write_u64(
        &mut descriptor,
        schema.descriptor_size_offset(),
        literal_size,
    )?;
    let signature = program.add_data(signature, 1)?;
    let descriptor = program.add_data(descriptor, schema.pointer_alignment())?;
    program.add_data_relocation(descriptor, schema.descriptor_signature_offset(), signature)?;
    Ok(Arm64DarwinBlockDescriptorId(descriptor))
}

/// Materializes one stack block with exactly one non-owning pointer capture.
///
/// `capture` is stored before `scratch` is used, so both arguments may name the same register.
/// The caller owns the surrounding stack allocation and must keep it live for the duration of the
/// native call. An escaping native API must copy the block before that call returns.
///
/// # Errors
///
/// Rejects stack offsets that cannot be represented by the selected ARM64 load/store form.
pub fn materialize_darwin_pointer_capture_stack_block(
    code: &mut Arm64CodeBuilder,
    block_offset: u32,
    stack_block_class: Arm64DataImportId,
    invoke: Arm64FunctionId,
    descriptor: Arm64DarwinBlockDescriptorId,
    capture: Arm64Register,
    scratch: Arm64Register,
) -> Result<(), Arm64DarwinBlockError> {
    let schema = DarwinBlockAbiSchema::ARM64_DARWIN;
    let last_field = stack_field(block_offset, schema.captures_offset())?;
    let pointer_size = u32::try_from(schema.pointer_size())
        .map_err(|_| Arm64DarwinBlockError::StackOffset(block_offset))?;
    if !block_offset.is_multiple_of(pointer_size) || last_field / pointer_size > 0x0fff {
        return Err(Arm64DarwinBlockError::StackOffset(block_offset));
    }

    store(
        code,
        Arm64LoadStoreSize::Double,
        Arm64DataRegister::General(capture),
        last_field,
    );
    code.load_data_import(stack_block_class, scratch);
    store(
        code,
        Arm64LoadStoreSize::Double,
        Arm64DataRegister::General(scratch),
        stack_field(block_offset, schema.isa_offset())?,
    );
    let flags = schema.has_signature_flag();
    if flags & 0xffff != 0 {
        return Err(Arm64DarwinBlockError::FlagsLayout);
    }
    code.append(Arm64Instruction::MoveWide {
        size: Arm64DataSize::Bits32,
        operation: Arm64MoveWide::Zero,
        destination: scratch,
        immediate: u16::try_from(flags >> 16).map_err(|_| Arm64DarwinBlockError::FlagsLayout)?,
        shift: 16,
    });
    store(
        code,
        Arm64LoadStoreSize::Word,
        Arm64DataRegister::General(scratch),
        stack_field(block_offset, schema.flags_offset())?,
    );
    store(
        code,
        Arm64LoadStoreSize::Word,
        Arm64DataRegister::Zero,
        stack_field(block_offset, schema.reserved_offset())?,
    );
    code.load_function_address(invoke, scratch);
    store(
        code,
        Arm64LoadStoreSize::Double,
        Arm64DataRegister::General(scratch),
        stack_field(block_offset, schema.invoke_offset())?,
    );
    code.load_data_address(descriptor.data(), scratch);
    store(
        code,
        Arm64LoadStoreSize::Double,
        Arm64DataRegister::General(scratch),
        stack_field(block_offset, schema.descriptor_offset())?,
    );
    Ok(())
}

/// Places the address of a stack-relative block in a general-purpose register.
///
/// # Errors
///
/// Rejects offsets outside the add-immediate encoding used by the closed adapter.
pub fn load_darwin_stack_block_address(
    code: &mut Arm64CodeBuilder,
    block_offset: u32,
    destination: Arm64Register,
) -> Result<(), Arm64DarwinBlockError> {
    let immediate = u16::try_from(block_offset)
        .ok()
        .filter(|offset| *offset <= 4095)
        .ok_or(Arm64DarwinBlockError::StackOffset(block_offset))?;
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::StackPointer,
        immediate,
        shift_12: false,
    });
    Ok(())
}

fn store(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    source: Arm64DataRegister,
    offset: u32,
) {
    code.append(Arm64Instruction::StoreUnsigned {
        size,
        source,
        base: Arm64BaseRegister::StackPointer,
        offset,
    });
}

fn stack_field(block_offset: u32, field_offset: u64) -> Result<u32, Arm64DarwinBlockError> {
    let field_offset = u32::try_from(field_offset)
        .map_err(|_| Arm64DarwinBlockError::StackOffset(block_offset))?;
    block_offset
        .checked_add(field_offset)
        .ok_or(Arm64DarwinBlockError::StackOffset(block_offset))
}

fn write_u64(bytes: &mut [u8], offset: u64, value: u64) -> Result<(), Arm64DarwinBlockError> {
    let start = usize::try_from(offset).map_err(|_| Arm64DarwinBlockError::DescriptorLayout)?;
    let end = start
        .checked_add(8)
        .ok_or(Arm64DarwinBlockError::DescriptorLayout)?;
    bytes
        .get_mut(start..end)
        .ok_or(Arm64DarwinBlockError::DescriptorLayout)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64DarwinBlockError {
    InvalidSignature,
    LiteralSizeOverflow,
    DescriptorLayout,
    FlagsLayout,
    StackOffset(u32),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinBlockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 Darwin block materialization failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64DarwinBlockError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Program(error) => Some(error),
            Self::InvalidSignature
            | Self::LiteralSizeOverflow
            | Self::DescriptorLayout
            | Self::FlagsLayout
            | Self::StackOffset(_) => None,
        }
    }
}

impl From<Arm64ProgramError> for Arm64DarwinBlockError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::{
        DarwinBlockAbiSchema, RuntimeDataImport, RuntimeLibraryIdentity,
    };

    use super::{
        add_darwin_pointer_capture_block_descriptor, materialize_darwin_pointer_capture_stack_block,
    };
    use crate::{Arm64CodeBuilder, Arm64ProgramBuilder, Arm64Register};

    #[test]
    fn descriptor_and_literal_share_the_closed_schema() {
        let mut program = Arm64ProgramBuilder::new();
        let invoke = program.declare_function();
        let descriptor =
            add_darwin_pointer_capture_block_descriptor(&mut program, b"v16@?0^v8\0".as_slice())
                .unwrap();
        let stack_class = program
            .add_data_import(
                RuntimeDataImport::new(
                    RuntimeLibraryIdentity::DarwinSystem,
                    "__NSConcreteStackBlock",
                )
                .unwrap(),
            )
            .unwrap();
        let mut code = Arm64CodeBuilder::new();
        materialize_darwin_pointer_capture_stack_block(
            &mut code,
            0,
            stack_class,
            invoke,
            descriptor,
            Arm64Register::new(8).unwrap(),
            Arm64Register::new(8).unwrap(),
        )
        .unwrap();

        assert_eq!(DarwinBlockAbiSchema::ARM64_DARWIN.literal_size(1), Some(40));
        assert!(!code.finish().unwrap().bytes().is_empty());
    }

    #[test]
    fn descriptor_rejects_noncanonical_signatures() {
        let mut program = Arm64ProgramBuilder::new();
        assert_eq!(
            add_darwin_pointer_capture_block_descriptor(&mut program, b"\0"),
            Err(super::Arm64DarwinBlockError::InvalidSignature)
        );
        assert_eq!(
            add_darwin_pointer_capture_block_descriptor(&mut program, b"v16@?0^v8"),
            Err(super::Arm64DarwinBlockError::InvalidSignature)
        );
        assert_eq!(
            add_darwin_pointer_capture_block_descriptor(&mut program, b"v16\0tail\0"),
            Err(super::Arm64DarwinBlockError::InvalidSignature)
        );
    }
}
