use nocter_machine::{MachineFunctionId, MachinePackId};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64Code, Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize, Arm64FrameCode,
    Arm64FrameLayout, Arm64FrameLayoutBuilder, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi, Arm64Register, Arm64SelectedFunction,
};

mod spread;

/// One of the two callbacks stored in every argument-pack descriptor.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Arm64PackCallbackKind {
    Next,
    Destroy,
}

/// Stable target callback identity derived only from its machine owner and body-local pack.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Arm64PackCallbackKey {
    owner: MachineFunctionId,
    pack: MachinePackId,
    kind: Arm64PackCallbackKind,
}

impl Arm64PackCallbackKey {
    #[must_use]
    pub const fn new(
        owner: MachineFunctionId,
        pack: MachinePackId,
        kind: Arm64PackCallbackKind,
    ) -> Self {
        Self { owner, pack, kind }
    }

    #[must_use]
    pub const fn owner(self) -> MachineFunctionId {
        self.owner
    }

    #[must_use]
    pub const fn pack(self) -> MachinePackId {
        self.pack
    }

    #[must_use]
    pub const fn kind(self) -> Arm64PackCallbackKind {
        self.kind
    }
}

/// Materializes one target-owned callback over a caller-owned pack state. Callback code consumes
/// only closed machine pack facts and generated function identities; it never sees MIR or a
/// recursive destruction plan.
pub(crate) fn materialize(
    machine: &nocter_machine::MachineProgram,
    selected: &Arm64SelectedFunction,
    key: Arm64PackCallbackKey,
    functions: &[(MachineFunctionId, crate::Arm64FunctionId)],
) -> Result<Arm64Code, Arm64MaterializationError> {
    if selected.owner() != key.owner() {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    }
    let body = machine
        .function(key.owner())
        .ok_or(Arm64MaterializationError::UnknownFunction(key.owner()))?
        .body();
    let pack = body
        .pack(key.pack())
        .ok_or(Arm64MaterializationError::InvalidPackCallback(key))?;
    let frame = selected
        .frame()
        .pack(key.pack())
        .ok_or(Arm64MaterializationError::InvalidPackCallback(key))?;
    if pack.segments().len() != frame.state_layout().segments().len() {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    }
    let has_spread = pack
        .segments()
        .iter()
        .any(|segment| matches!(segment, nocter_machine::MachinePackSegment::Spread(_)));
    match key.kind() {
        Arm64PackCallbackKind::Next if has_spread => {
            spread::materialize_next(machine, pack, frame.state_layout(), key, functions)
        }
        Arm64PackCallbackKind::Next => {
            materialize_fixed_next(machine, pack, frame.state_layout(), key)
        }
        Arm64PackCallbackKind::Destroy => {
            materialize_destroy(pack, frame.state_layout(), key, functions)
        }
    }
}

fn materialize_fixed_next(
    machine: &nocter_machine::MachineProgram,
    pack: &nocter_machine::MachinePack,
    state: &crate::Arm64PackStateLayout,
    key: Arm64PackCallbackKey,
) -> Result<Arm64Code, Arm64MaterializationError> {
    let FixedNextShape {
        layout: next_layout,
        tag_offset,
        payload_offset,
        destination,
    } = fixed_next_shape(machine, pack, key)?;
    let mut frame_builder = Arm64FrameLayoutBuilder::new();
    frame_builder.preserve(state_register())?;
    frame_builder.preserve(cursor_register())?;
    let staging = match destination {
        NextDestination::Direct { .. } => {
            Some(frame_builder.add_object(next_layout.size(), next_layout.alignment())?)
        }
        NextDestination::Indirect => None,
    };
    let frame = frame_builder.finish()?;
    let destination = destination.close(&frame, staging, key)?;
    let mut code = Arm64CodeBuilder::new();
    Arm64FrameCode::emit_prologue(&frame, &mut code);
    move_register(&mut code, state_register(), argument(0));
    load_register_offset(
        &mut code,
        Arm64LoadStoreSize::Double,
        cursor_register(),
        state_register(),
        state.cursor_offset(),
    );

    let finish = code.create_label();
    let none = code.create_label();
    let cases = (0..pack.segments().len())
        .map(|_| code.create_label())
        .collect::<Vec<_>>();
    for (index, label) in cases.iter().copied().enumerate() {
        compare_immediate(
            &mut code,
            cursor_register(),
            u64::try_from(index).map_err(|_| Arm64MaterializationError::OffsetOverflow)?,
        );
        code.branch_conditional(label, Arm64BranchCondition::Equal);
    }
    code.branch(none, false);

    for (index, ((segment, layout), label)) in pack
        .segments()
        .iter()
        .zip(state.segments())
        .zip(cases)
        .enumerate()
    {
        code.bind(label)?;
        let (
            nocter_machine::MachinePackSegment::Value { .. },
            crate::Arm64PackSegmentLayout::Value {
                value_offset, size, ..
            },
        ) = (segment, layout)
        else {
            return Err(Arm64MaterializationError::InvalidPackCallback(key));
        };
        destination.zero(&frame, next_layout.size(), &mut code)?;
        destination.store_byte(&frame, tag_offset, 0, &mut code)?;
        destination.copy_from_register(
            &frame,
            payload_offset,
            state_register(),
            *value_offset,
            *size,
            &mut code,
        )?;
        let next = u64::try_from(index)
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or(Arm64MaterializationError::OffsetOverflow)?;
        crate::frame_access::load_immediate(
            &mut code,
            cursor_register(),
            next,
            Arm64DataSize::Bits64,
        );
        store_register_offset(
            &mut code,
            Arm64LoadStoreSize::Double,
            cursor_register(),
            state_register(),
            state.cursor_offset(),
        );
        destination.load_result(&frame, pack.next_result(), &mut code)?;
        code.branch(finish, false);
    }

    code.bind(none)?;
    destination.zero(&frame, next_layout.size(), &mut code)?;
    destination.store_byte(
        &frame,
        tag_offset,
        u64::from(nocter_machine::MachineOutcomeKind::Optional.alternate_tag()),
        &mut code,
    )?;
    destination.load_result(&frame, pack.next_result(), &mut code)?;
    code.bind(finish)?;
    Arm64FrameCode::emit_epilogue(&frame, &mut code);
    code.finish().map_err(Arm64MaterializationError::Code)
}

struct FixedNextShape<'layout> {
    layout: &'layout nocter_machine::MachineLayout,
    tag_offset: u64,
    payload_offset: u64,
    destination: NextDestination,
}

fn fixed_next_shape<'layout>(
    machine: &'layout nocter_machine::MachineProgram,
    pack: &nocter_machine::MachinePack,
    key: Arm64PackCallbackKey,
) -> Result<FixedNextShape<'layout>, Arm64MaterializationError> {
    let layout = machine
        .layouts()
        .get(pack.next())
        .ok_or(Arm64MaterializationError::InvalidPackCallback(key))?;
    let nocter_machine::MachineLayoutKind::Outcome {
        kind: nocter_machine::MachineOutcomeKind::Optional,
        tag_offset,
        payload_offset,
        primary: Some(element),
        alternate: None,
    } = layout.kind()
    else {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    };
    let nocter_machine::MachineResultAbi::Value(returned) = pack.next_result() else {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    };
    if *element != pack.element() || returned.ty() != pack.next() {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    }
    Ok(FixedNextShape {
        layout,
        tag_offset: *tag_offset,
        payload_offset: *payload_offset,
        destination: NextDestination::build(returned, layout, key)?,
    })
}

fn materialize_destroy(
    pack: &nocter_machine::MachinePack,
    state: &crate::Arm64PackStateLayout,
    key: Arm64PackCallbackKey,
    functions: &[(MachineFunctionId, crate::Arm64FunctionId)],
) -> Result<Arm64Code, Arm64MaterializationError> {
    let mut frame_builder = Arm64FrameLayoutBuilder::new();
    for register in [state_register(), cursor_register(), context_register()] {
        frame_builder.preserve(register)?;
    }
    let frame = frame_builder.finish()?;
    let mut code = Arm64CodeBuilder::new();
    Arm64FrameCode::emit_prologue(&frame, &mut code);
    move_register(&mut code, state_register(), argument(0));
    move_register(
        &mut code,
        context_register(),
        Arm64NocterAbi::allocation_context_register(),
    );
    load_register_offset(
        &mut code,
        Arm64LoadStoreSize::Double,
        cursor_register(),
        state_register(),
        state.cursor_offset(),
    );
    crate::frame_access::load_immediate(
        &mut code,
        scratch(0),
        u64::try_from(pack.segments().len())
            .map_err(|_| Arm64MaterializationError::OffsetOverflow)?,
        Arm64DataSize::Bits64,
    );
    store_register_offset(
        &mut code,
        Arm64LoadStoreSize::Double,
        scratch(0),
        state_register(),
        state.cursor_offset(),
    );

    for (index, (segment, layout)) in pack
        .segments()
        .iter()
        .zip(state.segments())
        .enumerate()
        .rev()
    {
        let (destruction, value_offset) = match (segment, layout) {
            (
                nocter_machine::MachinePackSegment::Value { destruction, .. },
                crate::Arm64PackSegmentLayout::Value { value_offset, .. },
            ) => (*destruction, *value_offset),
            (
                nocter_machine::MachinePackSegment::Spread(spread),
                crate::Arm64PackSegmentLayout::Spread {
                    iterator_offset, ..
                },
            ) => (spread.destruction(), *iterator_offset),
            _ => return Err(Arm64MaterializationError::InvalidPackCallback(key)),
        };
        let Some(destruction) = destruction else {
            continue;
        };
        let skip = code.create_label();
        compare_immediate(
            &mut code,
            cursor_register(),
            u64::try_from(index).map_err(|_| Arm64MaterializationError::OffsetOverflow)?,
        );
        code.branch_conditional(skip, Arm64BranchCondition::UnsignedHigher);
        move_register(&mut code, argument(0), state_register());
        crate::frame_access::load_immediate(
            &mut code,
            argument(1),
            value_offset,
            Arm64DataSize::Bits64,
        );
        move_register(
            &mut code,
            Arm64NocterAbi::allocation_context_register(),
            context_register(),
        );
        code.call(function_target(functions, destruction)?);
        code.bind(skip)?;
    }
    Arm64FrameCode::emit_epilogue(&frame, &mut code);
    code.finish().map_err(Arm64MaterializationError::Code)
}

#[derive(Clone, Copy)]
enum NextDestination {
    Direct { size: u64 },
    Indirect,
}

#[derive(Clone, Copy)]
enum ClosedNextDestination {
    Direct { offset: u64, size: u64 },
    Indirect { pointer: Arm64Register },
}

impl NextDestination {
    fn build(
        result: nocter_machine::MachineReturnedValue,
        layout: &nocter_machine::MachineLayout,
        key: Arm64PackCallbackKey,
    ) -> Result<Self, Arm64MaterializationError> {
        match result.location() {
            nocter_machine::MachineResultLocation::Registers(registers)
                if result.class()
                    == (nocter_machine::MachineValueClass::Direct {
                        words: registers.words(),
                    })
                    && registers.first() == 0
                    && u64::from(registers.words())
                        == layout.size().div_ceil(Arm64NocterAbi::word_size()) =>
            {
                Ok(Self::Direct {
                    size: layout.size(),
                })
            }
            nocter_machine::MachineResultLocation::CallerStorage { pointer_register }
                if result.class() == nocter_machine::MachineValueClass::Indirect
                    && Arm64Register::new(pointer_register)
                        == Some(Arm64NocterAbi::indirect_result_register()) =>
            {
                Ok(Self::Indirect)
            }
            nocter_machine::MachineResultLocation::Omitted
            | nocter_machine::MachineResultLocation::Registers(_)
            | nocter_machine::MachineResultLocation::CallerStorage { .. } => {
                Err(Arm64MaterializationError::InvalidPackCallback(key))
            }
        }
    }

    fn close(
        self,
        frame: &Arm64FrameLayout,
        staging: Option<crate::Arm64FrameObjectId>,
        key: Arm64PackCallbackKey,
    ) -> Result<ClosedNextDestination, Arm64MaterializationError> {
        self.close_with_indirect_pointer(
            frame,
            staging,
            Arm64NocterAbi::indirect_result_register(),
            key,
        )
    }

    fn close_with_indirect_pointer(
        self,
        frame: &Arm64FrameLayout,
        staging: Option<crate::Arm64FrameObjectId>,
        pointer: Arm64Register,
        key: Arm64PackCallbackKey,
    ) -> Result<ClosedNextDestination, Arm64MaterializationError> {
        match (self, staging) {
            (Self::Direct { size, .. }, Some(staging)) => frame
                .object(staging)
                .map(|object| ClosedNextDestination::Direct {
                    offset: object.offset(),
                    size,
                })
                .ok_or(Arm64MaterializationError::InvalidPackCallback(key)),
            (Self::Indirect, None) => Ok(ClosedNextDestination::Indirect { pointer }),
            _ => Err(Arm64MaterializationError::InvalidPackCallback(key)),
        }
    }
}

impl ClosedNextDestination {
    fn zero(
        self,
        _frame: &Arm64FrameLayout,
        size: u64,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        crate::frame_access::load_immediate(code, scratch(0), 0, Arm64DataSize::Bits64);
        for (offset, width) in memory_chunks(size) {
            match self {
                Self::Direct { offset: base, .. } => crate::frame_access::store_at_stack_offset(
                    code,
                    width,
                    scratch(0),
                    checked_add(base, offset)?,
                ),
                Self::Indirect { pointer } => {
                    store_register_offset(code, width, scratch(0), pointer, offset);
                }
            }
        }
        Ok(())
    }

    fn store_byte(
        self,
        _frame: &Arm64FrameLayout,
        offset: u64,
        value: u64,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        crate::frame_access::load_immediate(code, scratch(0), value, Arm64DataSize::Bits64);
        match self {
            Self::Direct { offset: base, .. } => crate::frame_access::store_at_stack_offset(
                code,
                Arm64LoadStoreSize::Byte,
                scratch(0),
                checked_add(base, offset)?,
            ),
            Self::Indirect { pointer } => {
                store_register_offset(code, Arm64LoadStoreSize::Byte, scratch(0), pointer, offset);
            }
        }
        Ok(())
    }

    fn copy_from_register(
        self,
        _frame: &Arm64FrameLayout,
        destination_offset: u64,
        source: Arm64Register,
        source_offset: u64,
        size: u64,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        for (offset, width) in memory_chunks(size) {
            load_register_offset(
                code,
                width,
                scratch(0),
                source,
                checked_add(source_offset, offset)?,
            );
            match self {
                Self::Direct { offset: base, .. } => crate::frame_access::store_at_stack_offset(
                    code,
                    width,
                    scratch(0),
                    checked_add(checked_add(base, destination_offset)?, offset)?,
                ),
                Self::Indirect { pointer } => store_register_offset(
                    code,
                    width,
                    scratch(0),
                    pointer,
                    checked_add(destination_offset, offset)?,
                ),
            }
        }
        Ok(())
    }

    fn load_result(
        self,
        _frame: &Arm64FrameLayout,
        result: nocter_machine::MachineResultAbi,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        let (Self::Direct { offset, size }, nocter_machine::MachineResultAbi::Value(result)) =
            (self, result)
        else {
            return Ok(());
        };
        let nocter_machine::MachineResultLocation::Registers(registers) = result.location() else {
            return Err(Arm64MaterializationError::OffsetOverflow);
        };
        for lane in 0..registers.words() {
            let lane_offset = u64::from(lane)
                .checked_mul(Arm64NocterAbi::word_size())
                .ok_or(Arm64MaterializationError::OffsetOverflow)?;
            let remaining = size.saturating_sub(lane_offset);
            let width = u8::try_from(remaining.min(Arm64NocterAbi::word_size()))
                .map_err(|_| Arm64MaterializationError::OffsetOverflow)?;
            let destination = argument(registers.first() + lane);
            if matches!(width, 1 | 2 | 4 | 8) {
                crate::frame_access::load_at_stack_offset(
                    code,
                    load_store_size(width)?,
                    destination,
                    checked_add(offset, lane_offset)?,
                );
            } else {
                crate::memory_code::emit_fragmented_load(
                    code,
                    width,
                    destination,
                    checked_add(offset, lane_offset)?,
                )?;
            }
        }
        Ok(())
    }
}

fn compare_immediate(code: &mut Arm64CodeBuilder, value: Arm64Register, expected: u64) {
    crate::frame_access::load_immediate(code, scratch(0), expected, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(value),
        right: Arm64DataRegister::General(scratch(0)),
    });
}

fn move_register(code: &mut Arm64CodeBuilder, destination: Arm64Register, source: Arm64Register) {
    if destination != source {
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
}

fn load_register_offset(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    destination: Arm64Register,
    base: Arm64Register,
    offset: u64,
) {
    let (base, offset) = register_memory_base(code, size, base, offset, destination);
    code.append(Arm64Instruction::LoadUnsigned {
        size,
        destination: Arm64DataRegister::General(destination),
        base,
        offset,
    });
}

fn store_register_offset(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    source: Arm64Register,
    base: Arm64Register,
    offset: u64,
) {
    let address = if source == scratch(0) {
        scratch(1)
    } else {
        scratch(0)
    };
    let (base, offset) = register_memory_base(code, size, base, offset, address);
    code.append(Arm64Instruction::StoreUnsigned {
        size,
        source: Arm64DataRegister::General(source),
        base,
        offset,
    });
}

fn register_memory_base(
    code: &mut Arm64CodeBuilder,
    size: Arm64LoadStoreSize,
    base: Arm64Register,
    offset: u64,
    address: Arm64Register,
) -> (Arm64BaseRegister, u32) {
    let scale = memory_width(size);
    if offset <= 0x0fff * scale && offset.is_multiple_of(scale) {
        return (
            Arm64BaseRegister::General(base),
            u32::try_from(offset).expect("bounded register offset"),
        );
    }
    crate::frame_access::load_immediate(code, address, offset, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64DataRegister::General(address),
        left: Arm64DataRegister::General(base),
        right: Arm64DataRegister::General(address),
    });
    (Arm64BaseRegister::General(address), 0)
}

fn memory_chunks(size: u64) -> impl Iterator<Item = (u64, Arm64LoadStoreSize)> {
    let mut offset = 0;
    std::iter::from_fn(move || {
        if offset == size {
            return None;
        }
        let remaining = size - offset;
        let width = if remaining >= 8 {
            Arm64LoadStoreSize::Double
        } else if remaining >= 4 {
            Arm64LoadStoreSize::Word
        } else if remaining >= 2 {
            Arm64LoadStoreSize::Half
        } else {
            Arm64LoadStoreSize::Byte
        };
        let current = offset;
        offset += memory_width(width);
        Some((current, width))
    })
}

const fn memory_width(size: Arm64LoadStoreSize) -> u64 {
    match size {
        Arm64LoadStoreSize::Byte => 1,
        Arm64LoadStoreSize::Half => 2,
        Arm64LoadStoreSize::Word => 4,
        Arm64LoadStoreSize::Double => 8,
    }
}

fn load_store_size(bytes: u8) -> Result<Arm64LoadStoreSize, Arm64MaterializationError> {
    match bytes {
        1 => Ok(Arm64LoadStoreSize::Byte),
        2 => Ok(Arm64LoadStoreSize::Half),
        4 => Ok(Arm64LoadStoreSize::Word),
        8 => Ok(Arm64LoadStoreSize::Double),
        _ => Err(Arm64MaterializationError::InvalidMemoryWidth(bytes)),
    }
}

fn function_target(
    functions: &[(MachineFunctionId, crate::Arm64FunctionId)],
    target: MachineFunctionId,
) -> Result<crate::Arm64FunctionId, Arm64MaterializationError> {
    functions
        .get(target.index())
        .and_then(|(actual, selected)| (*actual == target).then_some(*selected))
        .ok_or(Arm64MaterializationError::UnknownFunction(target))
}

fn checked_add(left: u64, right: u64) -> Result<u64, Arm64MaterializationError> {
    left.checked_add(right)
        .ok_or(Arm64MaterializationError::OffsetOverflow)
}

fn argument(index: u8) -> Arm64Register {
    Arm64NocterAbi::argument_register(index).expect("pack callback uses ABI argument registers")
}

fn state_register() -> Arm64Register {
    Arm64Register::new(19).expect("x19 exists")
}

fn cursor_register() -> Arm64Register {
    Arm64Register::new(20).expect("x20 exists")
}

fn context_register() -> Arm64Register {
    Arm64Register::new(21).expect("x21 exists")
}

fn scratch(index: u8) -> Arm64Register {
    crate::frame_access::scratch(index)
}
