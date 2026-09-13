use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64CodeBuilder,
    Arm64DataSize, Arm64Instruction, Arm64LoadStoreSize, Arm64MaterializationError, Arm64NocterAbi,
    Arm64ProcessContextFrame, Arm64SelectedFunction,
};

/// Emits every target-service finalizer owned by one process root.
///
/// Finalization runs after source cleanup and result selection but before the ordinary frame
/// epilogue. Non-root functions cannot observe or invoke the process-lifetime service catalog.
pub(crate) fn emit(
    function: &Arm64SelectedFunction,
    primitives: &crate::primitive_targets::Arm64PrimitiveTargets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let Arm64ProcessContextFrame::ProgramRoot(context) = function.frame().process_context() else {
        return Ok(());
    };
    let context_offset = function
        .frame()
        .layout()
        .object(context)
        .ok_or(Arm64MaterializationError::InvalidProcessContextFrame(
            context,
        ))?
        .offset();
    for finalizer in primitives.process_finalizers() {
        // Preserve the complete direct-result lane. The transient stack allocation remains
        // aligned and is removed before the normal epilogue addresses the fixed frame again.
        crate::frame_access::form_stack_address(code, argument(2)?, context_offset);
        crate::frame_access::adjust_stack(code, 16, Arm64AddSubtract::Subtract);
        crate::frame_access::store_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            argument(0)?,
            0,
        );
        crate::frame_access::store_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            argument(1)?,
            8,
        );
        code.append(Arm64Instruction::AddSubtractImmediate {
            size: Arm64DataSize::Bits64,
            operation: Arm64AddSubtract::Add,
            set_flags: false,
            destination: Arm64AddSubtractDestination::General(argument(0)?),
            source: Arm64BaseRegister::General(argument(2)?),
            immediate: 0,
            shift_12: false,
        });
        code.call(finalizer);
        crate::frame_access::load_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            argument(0)?,
            0,
        );
        crate::frame_access::load_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            argument(1)?,
            8,
        );
        crate::frame_access::adjust_stack(code, 16, Arm64AddSubtract::Add);
    }
    Ok(())
}

fn argument(index: u8) -> Result<crate::Arm64Register, Arm64MaterializationError> {
    Arm64NocterAbi::argument_register(index)
        .ok_or(Arm64MaterializationError::MissingArgumentRegister(index))
}
