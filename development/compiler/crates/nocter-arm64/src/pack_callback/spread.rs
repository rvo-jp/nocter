use nocter_machine::{
    MachineCallableAbi, MachineFunctionId, MachineLayout, MachineLayoutKind, MachineOutcomeKind,
    MachinePack, MachinePackContribution, MachinePackSegment, MachinePackSpread, MachineResultAbi,
    MachineResultLocation, MachineValueClass,
};

use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64Code, Arm64CodeBuilder, Arm64DataSize, Arm64FrameCode, Arm64FrameLayout,
    Arm64FrameLayoutBuilder, Arm64FrameObject, Arm64Instruction, Arm64LoadStoreSize,
    Arm64MaterializationError, Arm64NocterAbi, Arm64PackSegmentLayout, Arm64PackStateLayout,
    Arm64Register,
};

use super::{
    Arm64PackCallbackKey, ClosedNextDestination, FixedNextShape, NextDestination, argument,
    checked_add, compare_immediate, context_register, cursor_register, fixed_next_shape,
    function_target, load_register_offset, load_store_size, move_register, scratch, state_register,
    store_register_offset,
};

pub(super) fn materialize_next(
    machine: &nocter_machine::MachineProgram,
    pack: &MachinePack,
    state: &Arm64PackStateLayout,
    key: Arm64PackCallbackKey,
    functions: &[(MachineFunctionId, crate::Arm64FunctionId)],
) -> Result<Arm64Code, Arm64MaterializationError> {
    let FixedNextShape {
        layout: next_layout,
        tag_offset,
        payload_offset,
        destination: destination_plan,
    } = fixed_next_shape(machine, pack, key)?;
    let (stage_size, stage_alignment) = spread_stage_layout(machine, pack, key)?;
    let mut frame_builder = Arm64FrameLayoutBuilder::new();
    for register in [state_register(), cursor_register(), context_register()] {
        frame_builder.preserve(register)?;
    }
    if matches!(destination_plan, NextDestination::Indirect) {
        frame_builder.preserve(outer_result_pointer())?;
    }
    let destination_staging = match destination_plan {
        NextDestination::Direct { .. } => {
            Some(frame_builder.add_object(next_layout.size(), next_layout.alignment())?)
        }
        NextDestination::Indirect => None,
    };
    let spread_staging = frame_builder.add_object(stage_size, stage_alignment)?;
    let frame = frame_builder.finish()?;
    let destination = destination_plan.close_with_indirect_pointer(
        &frame,
        destination_staging,
        outer_result_pointer(),
        key,
    )?;
    let staging = frame
        .object(spread_staging)
        .ok_or(Arm64MaterializationError::InvalidPackCallback(key))?;
    NextEmitter {
        machine,
        pack,
        state,
        key,
        functions,
        frame,
        destination,
        next_layout,
        next_tag_offset: tag_offset,
        next_payload_offset: payload_offset,
        staging,
    }
    .materialize()
}

struct NextEmitter<'program> {
    machine: &'program nocter_machine::MachineProgram,
    pack: &'program MachinePack,
    state: &'program Arm64PackStateLayout,
    key: Arm64PackCallbackKey,
    functions: &'program [(MachineFunctionId, crate::Arm64FunctionId)],
    frame: Arm64FrameLayout,
    destination: ClosedNextDestination,
    next_layout: &'program MachineLayout,
    next_tag_offset: u64,
    next_payload_offset: u64,
    staging: Arm64FrameObject,
}

impl NextEmitter<'_> {
    fn materialize(&self) -> Result<Arm64Code, Arm64MaterializationError> {
        let mut code = Arm64CodeBuilder::new();
        Arm64FrameCode::emit_prologue(&self.frame, &mut code);
        move_register(&mut code, state_register(), argument(0));
        move_register(
            &mut code,
            context_register(),
            Arm64NocterAbi::allocation_context_register(),
        );
        if matches!(self.destination, ClosedNextDestination::Indirect { .. }) {
            move_register(
                &mut code,
                outer_result_pointer(),
                Arm64NocterAbi::indirect_result_register(),
            );
        }
        load_register_offset(
            &mut code,
            Arm64LoadStoreSize::Double,
            cursor_register(),
            state_register(),
            self.state.cursor_offset(),
        );

        let dispatch = code.create_label();
        let finish = code.create_label();
        let none = code.create_label();
        let cases = (0..self.pack.segments().len())
            .map(|_| code.create_label())
            .collect::<Vec<_>>();
        code.bind(dispatch)?;
        Self::emit_dispatch(&cases, none, &mut code)?;
        for (index, ((segment, layout), label)) in self
            .pack
            .segments()
            .iter()
            .zip(self.state.segments())
            .zip(cases)
            .enumerate()
        {
            code.bind(label)?;
            match (segment, layout) {
                (MachinePackSegment::Value { .. }, Arm64PackSegmentLayout::Value { .. }) => {
                    self.emit_fixed(index, layout, finish, &mut code)?;
                }
                (MachinePackSegment::Spread(spread), Arm64PackSegmentLayout::Spread { .. }) => {
                    self.emit_spread(index, spread, layout, dispatch, finish, &mut code)?;
                }
                _ => return Err(Arm64MaterializationError::InvalidPackCallback(self.key)),
            }
        }
        code.bind(none)?;
        self.emit_none(&mut code)?;
        code.bind(finish)?;
        Arm64FrameCode::emit_epilogue(&self.frame, &mut code);
        code.finish().map_err(Arm64MaterializationError::Code)
    }

    fn emit_dispatch(
        cases: &[crate::Arm64LabelId],
        none: crate::Arm64LabelId,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        for (index, label) in cases.iter().copied().enumerate() {
            compare_immediate(
                code,
                cursor_register(),
                u64::try_from(index).map_err(|_| Arm64MaterializationError::OffsetOverflow)?,
            );
            code.branch_conditional(label, Arm64BranchCondition::Equal);
        }
        code.branch(none, false);
        Ok(())
    }

    fn emit_fixed(
        &self,
        index: usize,
        layout: &Arm64PackSegmentLayout,
        finish: crate::Arm64LabelId,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        let Arm64PackSegmentLayout::Value {
            value_offset, size, ..
        } = *layout
        else {
            return Err(Arm64MaterializationError::InvalidPackCallback(self.key));
        };
        self.destination
            .zero(&self.frame, self.next_layout.size(), code)?;
        self.destination
            .store_byte(&self.frame, self.next_tag_offset, 0, code)?;
        self.destination.copy_from_register(
            &self.frame,
            self.next_payload_offset,
            state_register(),
            value_offset,
            size,
            code,
        )?;
        self.advance(index, code)?;
        self.return_next(finish, code)
    }

    fn emit_spread(
        &self,
        index: usize,
        spread: &MachinePackSpread,
        layout: &Arm64PackSegmentLayout,
        dispatch: crate::Arm64LabelId,
        finish: crate::Arm64LabelId,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        let Arm64PackSegmentLayout::Spread {
            remaining_offset,
            iterator_offset,
            iterator_size: _,
            ..
        } = *layout
        else {
            return Err(Arm64MaterializationError::InvalidPackCallback(self.key));
        };
        let exhausted = code.create_label();
        load_register_offset(
            code,
            Arm64LoadStoreSize::Double,
            scratch(1),
            state_register(),
            remaining_offset,
        );
        compare_immediate(code, scratch(1), 0);
        code.branch_conditional(exhausted, Arm64BranchCondition::Equal);
        self.emit_spread_item(spread, remaining_offset, iterator_offset, finish, code)?;
        code.bind(exhausted)?;
        self.advance(index, code)?;
        self.destroy_spread(spread, iterator_offset, code)?;
        code.branch(dispatch, false);
        Ok(())
    }

    fn emit_spread_item(
        &self,
        spread: &MachinePackSpread,
        remaining_offset: u64,
        iterator_offset: u64,
        finish: crate::Arm64LabelId,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        let result = spread_result_shape(self.machine, spread, self.key)?;
        let receiver_offset = checked_add(iterator_offset, spread.next().receiver_offset())?;
        self.call_next(spread, receiver_offset, result.layout, code)?;
        let some = code.create_label();
        crate::frame_access::load_at_stack_offset(
            code,
            Arm64LoadStoreSize::Byte,
            scratch(1),
            checked_add(self.staging.offset(), result.tag_offset)?,
        );
        compare_immediate(code, scratch(1), 0);
        code.branch_conditional(some, Arm64BranchCondition::Equal);
        code.append(Arm64Instruction::Break {
            immediate: crate::runtime_trap::Arm64RuntimeTrap::ExactSizeIteratorViolation
                .immediate(),
        });
        code.bind(some)?;
        decrement_remaining(remaining_offset, code);
        self.emit_contribution(spread, result, code)?;
        self.return_next(finish, code)
    }

    fn call_next(
        &self,
        spread: &MachinePackSpread,
        receiver_offset: u64,
        result_layout: &MachineLayout,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        let abi = next_abi(self.machine, spread, self.key)?;
        move_register(code, argument(0), state_register());
        add_offset(code, argument(0), receiver_offset);
        prepare_result_storage(abi, result_layout, self.staging, code, self.key)?;
        move_register(
            code,
            Arm64NocterAbi::allocation_context_register(),
            context_register(),
        );
        code.call(function_target(self.functions, spread.next().target())?);
        capture_result(abi, result_layout, self.staging, code, self.key)
    }

    fn emit_contribution(
        &self,
        spread: &MachinePackSpread,
        result: SpreadResultShape<'_>,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        self.destination
            .zero(&self.frame, self.next_layout.size(), code)?;
        self.destination
            .store_byte(&self.frame, self.next_tag_offset, 0, code)?;
        let element = self
            .machine
            .layouts()
            .get(self.pack.element())
            .ok_or(Arm64MaterializationError::InvalidPackCallback(self.key))?;
        match spread.contribution() {
            MachinePackContribution::Direct if spread.next().item() == self.pack.element() => {
                crate::frame_access::form_stack_address(
                    code,
                    scratch(1),
                    checked_add(self.staging.offset(), result.payload_offset)?,
                );
            }
            MachinePackContribution::CopyBorrowed
                if matches!(
                    self.machine
                        .layouts()
                        .get(spread.next().item())
                        .map(MachineLayout::kind),
                    Some(MachineLayoutKind::Pointer)
                ) =>
            {
                crate::frame_access::load_at_stack_offset(
                    code,
                    Arm64LoadStoreSize::Double,
                    scratch(1),
                    checked_add(self.staging.offset(), result.payload_offset)?,
                );
            }
            MachinePackContribution::Direct | MachinePackContribution::CopyBorrowed => {
                return Err(Arm64MaterializationError::InvalidPackCallback(self.key));
            }
        }
        self.destination.copy_from_register(
            &self.frame,
            self.next_payload_offset,
            scratch(1),
            0,
            element.size(),
            code,
        )
    }

    fn destroy_spread(
        &self,
        spread: &MachinePackSpread,
        iterator_offset: u64,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        let Some(destruction) = spread.destruction() else {
            return Ok(());
        };
        move_register(code, argument(0), state_register());
        crate::frame_access::load_immediate(
            code,
            argument(1),
            iterator_offset,
            Arm64DataSize::Bits64,
        );
        move_register(
            code,
            Arm64NocterAbi::allocation_context_register(),
            context_register(),
        );
        code.call(function_target(self.functions, destruction)?);
        Ok(())
    }

    fn advance(
        &self,
        index: usize,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        let next = u64::try_from(index)
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or(Arm64MaterializationError::OffsetOverflow)?;
        crate::frame_access::load_immediate(code, cursor_register(), next, Arm64DataSize::Bits64);
        store_register_offset(
            code,
            Arm64LoadStoreSize::Double,
            cursor_register(),
            state_register(),
            self.state.cursor_offset(),
        );
        Ok(())
    }

    fn emit_none(&self, code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
        self.destination
            .zero(&self.frame, self.next_layout.size(), code)?;
        self.destination.store_byte(
            &self.frame,
            self.next_tag_offset,
            u64::from(MachineOutcomeKind::Optional.alternate_tag()),
            code,
        )?;
        self.destination
            .load_result(&self.frame, self.pack.next_result(), code)
    }

    fn return_next(
        &self,
        finish: crate::Arm64LabelId,
        code: &mut Arm64CodeBuilder,
    ) -> Result<(), Arm64MaterializationError> {
        self.destination
            .load_result(&self.frame, self.pack.next_result(), code)?;
        code.branch(finish, false);
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct SpreadResultShape<'layout> {
    layout: &'layout MachineLayout,
    tag_offset: u64,
    payload_offset: u64,
}

fn spread_result_shape<'layout>(
    machine: &'layout nocter_machine::MachineProgram,
    spread: &MachinePackSpread,
    key: Arm64PackCallbackKey,
) -> Result<SpreadResultShape<'layout>, Arm64MaterializationError> {
    let layout = machine
        .layouts()
        .get(spread.next().result())
        .ok_or(Arm64MaterializationError::InvalidPackCallback(key))?;
    let MachineLayoutKind::Outcome {
        kind: MachineOutcomeKind::Optional,
        tag_offset,
        payload_offset,
        primary: Some(item),
        alternate: None,
    } = layout.kind()
    else {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    };
    let abi = next_abi(machine, spread, key)?;
    let [receiver] = abi.arguments() else {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    };
    let Some(nocter_machine::MachineArgumentLocation::Registers(registers)) = receiver.location()
    else {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    };
    if *item != spread.next().item()
        || receiver.class() != (MachineValueClass::Direct { words: 1 })
        || registers.first() != 0
        || registers.words() != 1
        || abi.pack().is_some()
        || abi.stack_argument_size() != 0
        || !matches!(
            abi.result(),
            MachineResultAbi::Value(result) if result.ty() == spread.next().result()
        )
    {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    }
    Ok(SpreadResultShape {
        layout,
        tag_offset: *tag_offset,
        payload_offset: *payload_offset,
    })
}

fn next_abi<'program>(
    machine: &'program nocter_machine::MachineProgram,
    spread: &MachinePackSpread,
    key: Arm64PackCallbackKey,
) -> Result<&'program MachineCallableAbi, Arm64MaterializationError> {
    let function = machine.function(spread.next().target()).ok_or(
        Arm64MaterializationError::UnknownFunction(spread.next().target()),
    )?;
    let nocter_machine::MachineFunctionKind::Callable(abi) = function.kind() else {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    };
    Ok(abi)
}

fn spread_stage_layout(
    machine: &nocter_machine::MachineProgram,
    pack: &MachinePack,
    key: Arm64PackCallbackKey,
) -> Result<(u64, u64), Arm64MaterializationError> {
    let mut size = 0;
    let mut alignment = 1;
    for segment in pack.segments() {
        let MachinePackSegment::Spread(spread) = segment else {
            continue;
        };
        let layout = spread_result_shape(machine, spread, key)?.layout;
        size = size.max(layout.size());
        alignment = alignment.max(layout.alignment());
    }
    if size == 0 {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    }
    Ok((size, alignment))
}

fn prepare_result_storage(
    abi: &MachineCallableAbi,
    layout: &MachineLayout,
    staging: Arm64FrameObject,
    code: &mut Arm64CodeBuilder,
    key: Arm64PackCallbackKey,
) -> Result<(), Arm64MaterializationError> {
    let MachineResultAbi::Value(result) = abi.result() else {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    };
    match result.location() {
        MachineResultLocation::Registers(registers)
            if result.class()
                == (MachineValueClass::Direct {
                    words: registers.words(),
                })
                && registers.first() == 0
                && u64::from(registers.words())
                    == layout.size().div_ceil(Arm64NocterAbi::word_size()) =>
        {
            Ok(())
        }
        MachineResultLocation::CallerStorage { pointer_register }
            if result.class() == MachineValueClass::Indirect
                && Arm64Register::new(pointer_register)
                    == Some(Arm64NocterAbi::indirect_result_register()) =>
        {
            crate::frame_access::form_stack_address(
                code,
                Arm64NocterAbi::indirect_result_register(),
                staging.offset(),
            );
            Ok(())
        }
        MachineResultLocation::Omitted
        | MachineResultLocation::Registers(_)
        | MachineResultLocation::CallerStorage { .. } => {
            Err(Arm64MaterializationError::InvalidPackCallback(key))
        }
    }
}

fn capture_result(
    abi: &MachineCallableAbi,
    layout: &MachineLayout,
    staging: Arm64FrameObject,
    code: &mut Arm64CodeBuilder,
    key: Arm64PackCallbackKey,
) -> Result<(), Arm64MaterializationError> {
    let MachineResultAbi::Value(result) = abi.result() else {
        return Err(Arm64MaterializationError::InvalidPackCallback(key));
    };
    let MachineResultLocation::Registers(registers) = result.location() else {
        return Ok(());
    };
    for lane in 0..registers.words() {
        let lane_offset = u64::from(lane)
            .checked_mul(Arm64NocterAbi::word_size())
            .ok_or(Arm64MaterializationError::OffsetOverflow)?;
        let width = u8::try_from(
            layout
                .size()
                .saturating_sub(lane_offset)
                .min(Arm64NocterAbi::word_size()),
        )
        .map_err(|_| Arm64MaterializationError::OffsetOverflow)?;
        let source = argument(registers.first() + lane);
        let offset = checked_add(staging.offset(), lane_offset)?;
        if matches!(width, 1 | 2 | 4 | 8) {
            crate::frame_access::store_at_stack_offset(
                code,
                load_store_size(width)?,
                source,
                offset,
            );
        } else {
            move_register(code, scratch(0), source);
            crate::memory_code::emit_fragmented_store(code, width, scratch(0), offset)?;
        }
    }
    Ok(())
}

fn decrement_remaining(offset: u64, code: &mut Arm64CodeBuilder) {
    load_register_offset(
        code,
        Arm64LoadStoreSize::Double,
        scratch(1),
        state_register(),
        offset,
    );
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(scratch(1)),
        source: Arm64BaseRegister::General(scratch(1)),
        immediate: 1,
        shift_12: false,
    });
    store_register_offset(
        code,
        Arm64LoadStoreSize::Double,
        scratch(1),
        state_register(),
        offset,
    );
}

fn add_offset(code: &mut Arm64CodeBuilder, destination: Arm64Register, offset: u64) {
    if offset == 0 {
        return;
    }
    crate::frame_access::load_immediate(code, scratch(0), offset, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: crate::Arm64DataRegister::General(destination),
        left: crate::Arm64DataRegister::General(destination),
        right: crate::Arm64DataRegister::General(scratch(0)),
    });
}

fn outer_result_pointer() -> Arm64Register {
    Arm64Register::new(22).expect("x22 exists")
}
