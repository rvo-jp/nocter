use std::fmt;

use nocter_machine::{MachineArgumentLocation, MachineValueClass};

use crate::{
    Arm64AsyncFunctionPlan, Arm64Code, Arm64CodeBuilder, Arm64CodeError, Arm64DataSize,
    Arm64FrameCode, Arm64FrameLayout, Arm64FrameObject, Arm64FrameObjectId, Arm64FunctionTarget,
    Arm64LoadStoreSize, Arm64NocterAbi, Arm64Register,
};

pub(crate) fn materialize(
    plan: &Arm64AsyncFunctionPlan,
    target: Arm64FunctionTarget,
) -> Result<Arm64Code, Arm64AsyncConstructorError> {
    if target.owner() != plan.owner() {
        return Err(Arm64AsyncConstructorError::ForeignTarget {
            expected: plan.owner(),
            actual: target.owner(),
        });
    }
    let entries = target
        .asynchronous()
        .ok_or(Arm64AsyncConstructorError::ImmediateTarget(plan.owner()))?;
    let staging = plan.constructor_frame();
    let mut code = Arm64CodeBuilder::new();
    Arm64FrameCode::emit_prologue(staging.layout(), &mut code);
    stage_ambient_context(plan, &mut code)?;
    crate::async_pack_capture_code::stage_input(plan, &mut code)?;
    stage_register_parameters(plan, &mut code)?;
    stage_stack_parameters(plan, &mut code)?;
    close_indirect_parameters(plan, &mut code)?;

    crate::frame_access::load_immediate(
        &mut code,
        argument(1)?,
        plan.frame().size(),
        Arm64DataSize::Bits64,
    );
    crate::darwin_memory_code::emit_map(&mut code)?;
    store_stack_object(
        staging.layout(),
        staging.mapping(),
        0,
        Arm64LoadStoreSize::Double,
        argument(0)?,
        &mut code,
    )?;

    initialize_header(plan, entries, &mut code)?;
    copy_captures_to_heap(plan, &mut code)?;
    crate::async_pack_capture_code::capture(plan, &mut code)?;
    let result = abi_general_register(plan.result_register())?;
    load_stack_object(
        staging.layout(),
        staging.mapping(),
        0,
        Arm64LoadStoreSize::Double,
        result,
        &mut code,
    )?;
    Arm64FrameCode::emit_epilogue(staging.layout(), &mut code);
    code.finish().map_err(Arm64AsyncConstructorError::Code)
}

fn stage_ambient_context(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    let staging = plan.constructor_frame();
    store_stack_object(
        staging.layout(),
        staging.allocation_context(),
        0,
        Arm64LoadStoreSize::Double,
        Arm64NocterAbi::allocation_context_register(),
        code,
    )?;
    if let Some(destination) = staging.process_context() {
        store_stack_object(
            staging.layout(),
            destination,
            0,
            Arm64LoadStoreSize::Double,
            Arm64NocterAbi::process_context_register(),
            code,
        )?;
    }
    Ok(())
}

fn stage_register_parameters(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    let frame = plan.constructor_frame();
    for (index, capture) in plan.parameters().iter().copied().enumerate() {
        let destination = parameter_object(plan, index)?;
        let Some(MachineArgumentLocation::Registers(registers)) = capture.transport().location()
        else {
            continue;
        };
        match capture.transport().class() {
            MachineValueClass::Zero => {}
            MachineValueClass::Direct { words } => {
                if words != registers.words() {
                    return Err(Arm64AsyncConstructorError::ParameterTransport(index));
                }
                for lane in 0..words {
                    let offset = u64::from(lane) * Arm64NocterAbi::word_size();
                    let bytes = capture
                        .destination()
                        .size()
                        .saturating_sub(offset)
                        .min(Arm64NocterAbi::word_size());
                    store_general_width(
                        frame.layout(),
                        destination,
                        offset,
                        u8::try_from(bytes)
                            .map_err(|_| Arm64AsyncConstructorError::ParameterTransport(index))?,
                        abi_general_register(registers.first() + lane)?,
                        code,
                    )?;
                }
            }
            class @ (MachineValueClass::Float32 | MachineValueClass::Float64) => {
                if registers.words() != 1 {
                    return Err(Arm64AsyncConstructorError::ParameterTransport(index));
                }
                store_float(
                    frame.layout(),
                    destination,
                    float_size(class)?,
                    Arm64NocterAbi::floating_argument_register(registers.first())
                        .ok_or(Arm64AsyncConstructorError::RegisterOverflow)?,
                    code,
                )?;
            }
            MachineValueClass::Indirect => {
                if registers.words() != 1 || capture.destination().size() < 8 {
                    return Err(Arm64AsyncConstructorError::ParameterTransport(index));
                }
                store_stack_object(
                    frame.layout(),
                    destination,
                    0,
                    Arm64LoadStoreSize::Double,
                    abi_general_register(registers.first())?,
                    code,
                )?;
            }
        }
    }
    Ok(())
}

fn stage_stack_parameters(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    let frame = plan.constructor_frame();
    for (index, capture) in plan.parameters().iter().copied().enumerate() {
        let Some(MachineArgumentLocation::Stack(source)) = capture.transport().location() else {
            continue;
        };
        let destination = parameter_object(plan, index)?;
        let transport_bytes = match capture.transport().class() {
            MachineValueClass::Zero => 0,
            MachineValueClass::Direct { words } => u64::from(words) * Arm64NocterAbi::word_size(),
            MachineValueClass::Float32 => 4,
            MachineValueClass::Float64 | MachineValueClass::Indirect => 8,
        };
        if source.size() < transport_bytes {
            return Err(Arm64AsyncConstructorError::ParameterTransport(index));
        }
        let stored = if capture.transport().class() == MachineValueClass::Indirect {
            8
        } else {
            capture.destination().size()
        };
        let source = frame
            .layout()
            .size()
            .checked_add(source.offset())
            .ok_or(Arm64AsyncConstructorError::OffsetOverflow)?;
        copy_stack_to_object(frame.layout(), source, destination, stored, code)?;
    }
    Ok(())
}

fn close_indirect_parameters(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    let frame = plan.constructor_frame();
    for (index, capture) in plan.parameters().iter().copied().enumerate() {
        if capture.transport().class() != MachineValueClass::Indirect {
            continue;
        }
        let destination = parameter_object(plan, index)?;
        // Keep the source pointer outside the frame-address scratch bank. Large stack offsets may
        // consume either compiler scratch register while each copied chunk is stored.
        let pointer = argument(0)?;
        load_stack_object(
            frame.layout(),
            destination,
            0,
            Arm64LoadStoreSize::Double,
            pointer,
            code,
        )?;
        for (offset, bytes) in crate::memory_code::exact_memory_chunks(capture.destination().size())
        {
            let transfer = scratch(0)?;
            let size = load_store_size(bytes)?;
            crate::address_code::load_native(code, size, None, transfer, pointer, offset);
            store_stack_object(frame.layout(), destination, offset, size, transfer, code)?;
        }
    }
    Ok(())
}

fn initialize_header(
    plan: &Arm64AsyncFunctionPlan,
    entries: crate::Arm64AsyncFunctionTargets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    for (target, field) in [
        (entries.resume(), plan.frame().resume_function()),
        (entries.cancel(), plan.frame().cancel_function()),
        (entries.consume(), plan.frame().consume_function()),
    ] {
        let mapping = load_mapping(plan, code)?;
        let value = scratch_avoiding(mapping)?;
        code.load_function_address(target, value);
        crate::address_code::store_native(
            code,
            Arm64LoadStoreSize::Double,
            value,
            mapping,
            field.offset(),
        );
    }
    let mapping = load_mapping(plan, code)?;
    let value = scratch_avoiding(mapping)?;
    crate::frame_access::load_immediate(
        code,
        value,
        plan.frame().initial_tag(),
        Arm64DataSize::Bits64,
    );
    crate::address_code::store_native(
        code,
        Arm64LoadStoreSize::Double,
        value,
        mapping,
        plan.frame().state_tag().offset(),
    );
    copy_object_to_heap(
        plan,
        plan.constructor_frame().allocation_context(),
        plan.frame().allocation_context(),
        code,
    )?;
    if let (Some(source), Some(destination)) = (
        plan.constructor_frame().process_context(),
        plan.frame().process_context(),
    ) {
        copy_object_to_heap(plan, source, destination, code)?;
    }
    Ok(())
}

fn copy_captures_to_heap(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    for (index, capture) in plan.parameters().iter().copied().enumerate() {
        copy_object_to_heap(
            plan,
            parameter_object(plan, index)?,
            capture.destination(),
            code,
        )?;
    }
    Ok(())
}

fn copy_object_to_heap(
    plan: &Arm64AsyncFunctionPlan,
    source: Arm64FrameObjectId,
    destination: crate::Arm64AsyncFrameField,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    let source = object(plan.constructor_frame().layout(), source)?;
    if source.size() != destination.size() {
        return Err(Arm64AsyncConstructorError::CaptureLayout);
    }
    for (offset, bytes) in crate::memory_code::exact_memory_chunks(source.size()) {
        let transfer = scratch(0)?;
        let size = load_store_size(bytes)?;
        crate::frame_access::load_at_stack_offset(
            code,
            size,
            transfer,
            source
                .offset()
                .checked_add(offset)
                .ok_or(Arm64AsyncConstructorError::OffsetOverflow)?,
        );
        let mapping = load_mapping(plan, code)?;
        crate::address_code::store_native(
            code,
            size,
            transfer,
            mapping,
            destination
                .offset()
                .checked_add(offset)
                .ok_or(Arm64AsyncConstructorError::OffsetOverflow)?,
        );
    }
    Ok(())
}

fn load_mapping(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<Arm64Register, Arm64AsyncConstructorError> {
    let mapping = scratch(1)?;
    load_stack_object(
        plan.constructor_frame().layout(),
        plan.constructor_frame().mapping(),
        0,
        Arm64LoadStoreSize::Double,
        mapping,
        code,
    )?;
    Ok(mapping)
}

fn parameter_object(
    plan: &Arm64AsyncFunctionPlan,
    index: usize,
) -> Result<Arm64FrameObjectId, Arm64AsyncConstructorError> {
    plan.constructor_frame()
        .parameter(index)
        .ok_or(Arm64AsyncConstructorError::MissingParameter(index))
}

fn copy_stack_to_object(
    frame: &Arm64FrameLayout,
    source: u64,
    destination: Arm64FrameObjectId,
    bytes: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    for (offset, width) in crate::memory_code::exact_memory_chunks(bytes) {
        let transfer = scratch(0)?;
        let size = load_store_size(width)?;
        crate::frame_access::load_at_stack_offset(
            code,
            size,
            transfer,
            source
                .checked_add(offset)
                .ok_or(Arm64AsyncConstructorError::OffsetOverflow)?,
        );
        store_stack_object(frame, destination, offset, size, transfer, code)?;
    }
    Ok(())
}

fn store_general_width(
    frame: &Arm64FrameLayout,
    object: Arm64FrameObjectId,
    offset: u64,
    bytes: u8,
    source: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    let destination = object_offset(frame, object, offset, u64::from(bytes))?;
    if let Ok(size) = load_store_size(bytes) {
        crate::frame_access::store_at_stack_offset(code, size, source, destination);
    } else {
        crate::memory_code::emit_fragmented_store(code, bytes, source, destination)?;
    }
    Ok(())
}

fn store_float(
    frame: &Arm64FrameLayout,
    object_id: Arm64FrameObjectId,
    size: Arm64DataSize,
    source: crate::Arm64FloatRegister,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    crate::floating_code::store_to_stack(source, object(frame, object_id)?.offset(), size, code);
    Ok(())
}

fn store_stack_object(
    frame: &Arm64FrameLayout,
    object: Arm64FrameObjectId,
    offset: u64,
    size: Arm64LoadStoreSize,
    source: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    crate::frame_access::store_at_stack_offset(
        code,
        size,
        source,
        object_offset(frame, object, offset, load_store_bytes(size))?,
    );
    Ok(())
}

fn load_stack_object(
    frame: &Arm64FrameLayout,
    object: Arm64FrameObjectId,
    offset: u64,
    size: Arm64LoadStoreSize,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncConstructorError> {
    crate::frame_access::load_at_stack_offset(
        code,
        size,
        destination,
        object_offset(frame, object, offset, load_store_bytes(size))?,
    );
    Ok(())
}

fn object(
    frame: &Arm64FrameLayout,
    id: Arm64FrameObjectId,
) -> Result<Arm64FrameObject, Arm64AsyncConstructorError> {
    frame
        .object(id)
        .ok_or(Arm64AsyncConstructorError::UnknownFrameObject(id))
}

fn object_offset(
    frame: &Arm64FrameLayout,
    id: Arm64FrameObjectId,
    offset: u64,
    bytes: u64,
) -> Result<u64, Arm64AsyncConstructorError> {
    let object = object(frame, id)?;
    let end = offset
        .checked_add(bytes)
        .ok_or(Arm64AsyncConstructorError::OffsetOverflow)?;
    if end > object.size() {
        return Err(Arm64AsyncConstructorError::FrameObjectBounds(id));
    }
    object
        .offset()
        .checked_add(offset)
        .ok_or(Arm64AsyncConstructorError::OffsetOverflow)
}

fn load_store_size(bytes: u8) -> Result<Arm64LoadStoreSize, Arm64AsyncConstructorError> {
    match bytes {
        1 => Ok(Arm64LoadStoreSize::Byte),
        2 => Ok(Arm64LoadStoreSize::Half),
        4 => Ok(Arm64LoadStoreSize::Word),
        8 => Ok(Arm64LoadStoreSize::Double),
        _ => Err(Arm64AsyncConstructorError::InvalidMemoryWidth(bytes)),
    }
}

const fn load_store_bytes(size: Arm64LoadStoreSize) -> u64 {
    match size {
        Arm64LoadStoreSize::Byte => 1,
        Arm64LoadStoreSize::Half => 2,
        Arm64LoadStoreSize::Word => 4,
        Arm64LoadStoreSize::Double => 8,
    }
}

fn float_size(class: MachineValueClass) -> Result<Arm64DataSize, Arm64AsyncConstructorError> {
    match class {
        MachineValueClass::Float32 => Ok(Arm64DataSize::Bits32),
        MachineValueClass::Float64 => Ok(Arm64DataSize::Bits64),
        _ => Err(Arm64AsyncConstructorError::InvalidFloatClass),
    }
}

fn abi_general_register(index: u8) -> Result<Arm64Register, Arm64AsyncConstructorError> {
    Arm64NocterAbi::argument_register(index).ok_or(Arm64AsyncConstructorError::RegisterOverflow)
}

fn argument(index: u8) -> Result<Arm64Register, Arm64AsyncConstructorError> {
    abi_general_register(index)
}

fn scratch(index: u8) -> Result<Arm64Register, Arm64AsyncConstructorError> {
    Arm64NocterAbi::compiler_scratch_register(index)
        .ok_or(Arm64AsyncConstructorError::RegisterOverflow)
}

fn scratch_avoiding(occupied: Arm64Register) -> Result<Arm64Register, Arm64AsyncConstructorError> {
    [scratch(0)?, scratch(1)?]
        .into_iter()
        .find(|candidate| *candidate != occupied)
        .ok_or(Arm64AsyncConstructorError::RegisterOverflow)
}

#[derive(Debug)]
pub enum Arm64AsyncConstructorError {
    ForeignTarget {
        expected: nocter_machine::MachineFunctionId,
        actual: nocter_machine::MachineFunctionId,
    },
    ImmediateTarget(nocter_machine::MachineFunctionId),
    MissingPackInput,
    MissingParameter(usize),
    ParameterTransport(usize),
    RegisterOverflow,
    InvalidFloatClass,
    InvalidMemoryWidth(u8),
    UnknownFrameObject(Arm64FrameObjectId),
    FrameObjectBounds(Arm64FrameObjectId),
    CaptureLayout,
    OffsetOverflow,
    Materialization(crate::Arm64MaterializationError),
    Code(Arm64CodeError),
}

impl fmt::Display for Arm64AsyncConstructorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 async constructor emission failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64AsyncConstructorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Materialization(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::ForeignTarget { .. }
            | Self::ImmediateTarget(_)
            | Self::MissingPackInput
            | Self::MissingParameter(_)
            | Self::ParameterTransport(_)
            | Self::RegisterOverflow
            | Self::InvalidFloatClass
            | Self::InvalidMemoryWidth(_)
            | Self::UnknownFrameObject(_)
            | Self::FrameObjectBounds(_)
            | Self::CaptureLayout
            | Self::OffsetOverflow => None,
        }
    }
}

impl From<Arm64CodeError> for Arm64AsyncConstructorError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<crate::Arm64MaterializationError> for Arm64AsyncConstructorError {
    fn from(error: crate::Arm64MaterializationError) -> Self {
        Self::Materialization(error)
    }
}
