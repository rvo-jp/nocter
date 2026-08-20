use crate::{
    Arm64CodeBuilder, Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi, Arm64SelectedFunction, Arm64SelectedInstruction,
};

use crate::error_layout::Arm64ErrorLayout;

const DARWIN_SUPERVISOR_CALL: u16 = 0x80;
const DARWIN_WRITE: u64 = 0x0200_0004;
const STDERR: u64 = 2;
const SEPARATOR_AND_NEWLINE: u64 = u64::from_le_bytes([b':', b' ', b'\n', 0, 0, 0, 0, 0]);

pub(crate) fn emit_selected(
    function: &Arm64SelectedFunction,
    instruction: &Arm64SelectedInstruction,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let Arm64SelectedInstruction::ReportError { error, buffer } = *instruction else {
        unreachable!("error materialization accepts only report-error instructions")
    };
    let error = object_offset(
        function,
        error,
        Arm64ErrorLayout::SIZE,
        Arm64ErrorLayout::ALIGNMENT,
    )?;
    let buffer = object_offset(
        function,
        buffer,
        Arm64ErrorLayout::REPORT_BUFFER_SIZE,
        Arm64ErrorLayout::REPORT_BUFFER_ALIGNMENT,
    )?;

    let temporary = scratch();
    crate::frame_access::load_immediate(
        code,
        temporary,
        SEPARATOR_AND_NEWLINE,
        Arm64DataSize::Bits64,
    );
    crate::frame_access::store_at_stack_offset(code, Arm64LoadStoreSize::Double, temporary, buffer);

    emit_error_view(error, Arm64ErrorLayout::CODE_OFFSET, code)?;
    emit_stack_bytes(buffer, 2, code);
    emit_error_view(error, Arm64ErrorLayout::MESSAGE_OFFSET, code)?;
    emit_stack_bytes(checked_add(buffer, 2)?, 1, code);
    Ok(())
}

fn emit_error_view(
    error: u64,
    view: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    prepare_write(code);
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(1),
        checked_add(
            checked_add(error, view)?,
            Arm64ErrorLayout::VIEW_POINTER_OFFSET,
        )?,
    );
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        argument(2),
        checked_add(
            checked_add(error, view)?,
            Arm64ErrorLayout::VIEW_LENGTH_OFFSET,
        )?,
    );
    emit_write(code);
    Ok(())
}

fn emit_stack_bytes(offset: u64, len: u64, code: &mut Arm64CodeBuilder) {
    prepare_write(code);
    crate::frame_access::form_stack_address(code, argument(1), offset);
    crate::frame_access::load_immediate(code, argument(2), len, Arm64DataSize::Bits64);
    emit_write(code);
}

fn prepare_write(code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_immediate(code, argument(0), STDERR, Arm64DataSize::Bits64);
}

fn emit_write(code: &mut Arm64CodeBuilder) {
    crate::frame_access::load_immediate(code, scratch(), DARWIN_WRITE, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::SupervisorCall {
        immediate: DARWIN_SUPERVISOR_CALL,
    });
    // Reporting is deliberately best-effort: the compiler-owned root exits with status one even
    // if stderr itself is closed or the write is interrupted.
}

fn object_offset(
    function: &Arm64SelectedFunction,
    object: crate::Arm64FrameObjectId,
    size: u64,
    alignment: u64,
) -> Result<u64, Arm64MaterializationError> {
    let object_layout = function
        .frame()
        .layout()
        .object(object)
        .ok_or(Arm64MaterializationError::UnknownFrameObject(object))?;
    if object_layout.size() != size || object_layout.alignment() != alignment {
        return Err(Arm64MaterializationError::InvalidErrorFrame(object));
    }
    Ok(object_layout.offset())
}

fn checked_add(left: u64, right: u64) -> Result<u64, Arm64MaterializationError> {
    left.checked_add(right)
        .ok_or(Arm64MaterializationError::OffsetOverflow)
}

fn argument(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::argument_register(index).expect("error reporting uses ABI argument registers")
}

fn scratch() -> crate::Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(0)
        .expect("the ABI reserves one syscall-number scratch register")
}
