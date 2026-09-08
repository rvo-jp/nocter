use std::fmt;

use crate::async_cancellation::{
    Arm64AsyncAddress, Arm64AsyncAddressBound, Arm64AsyncAddressIndex, Arm64AsyncAddressRoot,
    Arm64AsyncAddressStep, Arm64AsyncCancellationAction,
};
use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64AsyncFrameField, Arm64AsyncFunctionPlan,
    Arm64BaseRegister, Arm64BranchCondition, Arm64Code, Arm64CodeBuilder, Arm64CodeError,
    Arm64DataRegister, Arm64DataSize, Arm64FrameCode, Arm64FunctionTarget, Arm64FunctionTargets,
    Arm64Instruction, Arm64LoadStoreSize, Arm64NocterAbi, Arm64Register,
};

pub(crate) fn materialize(
    plan: &Arm64AsyncFunctionPlan,
    target: Arm64FunctionTarget,
    functions: &Arm64FunctionTargets,
) -> Result<Arm64Code, Arm64AsyncCancelError> {
    validate_target(plan, target)?;
    let activation = plan.cancellation().activation();
    let mut code = Arm64CodeBuilder::new();
    Arm64FrameCode::emit_prologue(activation.layout(), &mut code);
    store_mapping(plan, argument(0)?, &mut code)?;

    let actual = scratch(0)?;
    let mapping = load_mapping(plan, scratch(1)?, &mut code)?;
    crate::address_code::load_native(
        &mut code,
        Arm64LoadStoreSize::Double,
        None,
        actual,
        mapping,
        plan.frame().state_tag().offset(),
    );
    let state_labels = plan
        .cancellation()
        .states()
        .iter()
        .map(|_| code.create_label())
        .collect::<Vec<_>>();
    let completed = code.create_label();
    let release = code.create_label();
    for (state, label) in plan.cancellation().states().iter().zip(&state_labels) {
        emit_tag_branch(actual, state.tag(), *label, &mut code)?;
    }
    emit_tag_branch(actual, plan.frame().completed_tag(), completed, &mut code)?;
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameStateCorruption.immediate(),
    });

    for (state, label) in plan.cancellation().states().iter().zip(state_labels) {
        code.bind(label)?;
        for action in state.actions() {
            emit_action(plan, action, functions, &mut code)?;
        }
        code.branch(release, false);
    }
    code.bind(completed)?;
    if let Some(destruction) = plan.cancellation().completed_destruction() {
        let output = plan
            .frame()
            .output()
            .ok_or(Arm64AsyncCancelError::MissingCompletedOutput)?;
        emit_destruction(
            plan,
            &Arm64AsyncAddress::frame(output),
            None,
            destruction,
            functions,
            &mut code,
        )?;
    }
    code.branch(release, false);

    code.bind(release)?;
    let mapping = load_mapping(plan, argument(0)?, &mut code)?;
    crate::address_code::move_register(&mut code, mapping, argument(0)?);
    crate::frame_access::load_immediate(
        &mut code,
        argument(1)?,
        plan.frame().size(),
        Arm64DataSize::Bits64,
    );
    crate::darwin_memory_code::emit_unmap(
        &mut code,
        crate::runtime_trap::Arm64RuntimeTrap::AsyncFrameReleaseFailure,
    )?;
    Arm64FrameCode::emit_epilogue(activation.layout(), &mut code);
    code.finish().map_err(Arm64AsyncCancelError::Code)
}

fn validate_target(
    plan: &Arm64AsyncFunctionPlan,
    target: Arm64FunctionTarget,
) -> Result<(), Arm64AsyncCancelError> {
    if target.owner() != plan.owner() {
        return Err(Arm64AsyncCancelError::ForeignTarget {
            expected: plan.owner(),
            actual: target.owner(),
        });
    }
    target
        .asynchronous()
        .ok_or(Arm64AsyncCancelError::ImmediateTarget(plan.owner()))?;
    Ok(())
}

fn emit_tag_branch(
    actual: Arm64Register,
    tag: u64,
    target: crate::Arm64LabelId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncCancelError> {
    let expected = scratch(1)?;
    crate::frame_access::load_immediate(code, expected, tag, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(actual),
        right: Arm64DataRegister::General(expected),
    });
    code.branch_conditional(target, Arm64BranchCondition::Equal);
    Ok(())
}

fn emit_action(
    plan: &Arm64AsyncFunctionPlan,
    action: &Arm64AsyncCancellationAction,
    functions: &Arm64FunctionTargets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncCancelError> {
    match action {
        Arm64AsyncCancellationAction::ReleaseAwaited(field) => {
            let mapping = load_mapping(plan, scratch(1)?, code)?;
            let child = argument(0)?;
            crate::address_code::load_native(
                code,
                Arm64LoadStoreSize::Double,
                None,
                child,
                mapping,
                field.offset(),
            );
            let target = scratch(0)?;
            crate::address_code::load_native(
                code,
                Arm64LoadStoreSize::Double,
                None,
                target,
                child,
                Arm64NocterAbi::asynchronous().cancel_function_offset(),
            );
            code.append(Arm64Instruction::BranchRegister { target, link: true });
            Ok(())
        }
        Arm64AsyncCancellationAction::Destroy {
            address,
            initialized,
            destruction,
        } => emit_destruction(plan, address, *initialized, *destruction, functions, code),
        Arm64AsyncCancellationAction::ReleaseRegion(field) => {
            emit_region_release(plan, *field, code)
        }
        Arm64AsyncCancellationAction::DestroyPack => emit_destroy_pack(plan, code),
    }
}

fn emit_destroy_pack(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncCancelError> {
    let field = plan
        .frame()
        .pack_input()
        .ok_or(Arm64AsyncCancelError::MissingPackInput)?;
    restore_ambient_context(plan, code)?;
    let descriptor = argument(2)?;
    load_frame_word(plan, field, 0, descriptor, code)?;
    let callback = scratch(0)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        callback,
        descriptor,
        crate::Arm64PackDescriptorLayout::DESTROY_CALLBACK_OFFSET,
    );
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        argument(0)?,
        descriptor,
        crate::Arm64PackDescriptorLayout::STATE_POINTER_OFFSET,
    );
    code.append(Arm64Instruction::BranchRegister {
        target: callback,
        link: true,
    });
    load_frame_word(plan, field, 0, descriptor, code)?;
    crate::pack_allocation_code::emit_release(descriptor, code)?;
    Ok(())
}

fn emit_destruction(
    plan: &Arm64AsyncFunctionPlan,
    address: &Arm64AsyncAddress,
    initialized: Option<Arm64AsyncFrameField>,
    destruction: nocter_machine::MachineFunctionId,
    functions: &Arm64FunctionTargets,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncCancelError> {
    let skip = initialized.map(|_| code.create_label());
    if let (Some(flag), Some(skip)) = (initialized, skip) {
        let mapping = load_mapping(plan, scratch(1)?, code)?;
        let value = scratch(0)?;
        crate::address_code::load_native(
            code,
            Arm64LoadStoreSize::Byte,
            None,
            value,
            mapping,
            flag.offset(),
        );
        compare_zero(value, code);
        code.branch_conditional(skip, Arm64BranchCondition::Equal);
    }

    restore_ambient_context(plan, code)?;
    resolve_address(plan, address, code)?;
    crate::frame_access::load_immediate(code, argument(1)?, 0, Arm64DataSize::Bits64);
    let target = functions
        .get(destruction)
        .map(Arm64FunctionTarget::callable)
        .ok_or(Arm64AsyncCancelError::UnknownDestruction(destruction))?;
    code.call(target);
    if let Some(skip) = skip {
        code.bind(skip)?;
    }
    Ok(())
}

fn restore_ambient_context(
    plan: &Arm64AsyncFunctionPlan,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncCancelError> {
    let mapping = load_mapping(plan, scratch(1)?, code)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        Arm64NocterAbi::allocation_context_register(),
        mapping,
        plan.frame().allocation_context().offset(),
    );
    if let Some(process) = plan.frame().process_context() {
        crate::address_code::load_native(
            code,
            Arm64LoadStoreSize::Double,
            None,
            Arm64NocterAbi::process_context_register(),
            mapping,
            process.offset(),
        );
    }
    Ok(())
}

fn resolve_address(
    plan: &Arm64AsyncFunctionPlan,
    address: &Arm64AsyncAddress,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncCancelError> {
    let destination = scratch(0)?;
    let view_length = scratch(1)?;
    match address.root() {
        Arm64AsyncAddressRoot::Frame(field) => {
            load_mapping(plan, destination, code)?;
            crate::address_code::add_offset(code, destination, field.offset());
        }
        Arm64AsyncAddressRoot::Pointer(field) => {
            load_frame_word(plan, field, 0, destination, code)?;
        }
        Arm64AsyncAddressRoot::View {
            field,
            pointer_offset,
            length_offset,
        } => {
            load_frame_word(plan, field, length_offset, view_length, code)?;
            load_frame_word(plan, field, pointer_offset, destination, code)?;
        }
    }
    for step in address.steps() {
        match *step {
            Arm64AsyncAddressStep::Offset(offset) => {
                crate::address_code::add_offset(code, destination, offset);
            }
            Arm64AsyncAddressStep::OffsetFrame(field) => {
                let offset = argument(1)?;
                load_frame_word(plan, field, 0, offset, code)?;
                add_register(destination, offset, code);
            }
            Arm64AsyncAddressStep::Dereference => crate::address_code::load_native(
                code,
                Arm64LoadStoreSize::Double,
                None,
                destination,
                destination,
                0,
            ),
            Arm64AsyncAddressStep::ViewDereference {
                pointer_offset,
                length_offset,
            } => {
                crate::address_code::load_native(
                    code,
                    Arm64LoadStoreSize::Double,
                    None,
                    view_length,
                    destination,
                    length_offset,
                );
                crate::address_code::load_native(
                    code,
                    Arm64LoadStoreSize::Double,
                    None,
                    destination,
                    destination,
                    pointer_offset,
                );
            }
            Arm64AsyncAddressStep::Index {
                index,
                stride,
                bound,
            } => {
                let index_register = argument(0)?;
                load_index(plan, index, index_register, code)?;
                let bound = match bound {
                    Arm64AsyncAddressBound::Fixed(length) => {
                        let register = argument(1)?;
                        crate::frame_access::load_immediate(
                            code,
                            register,
                            length,
                            Arm64DataSize::Bits64,
                        );
                        register
                    }
                    Arm64AsyncAddressBound::CurrentView => view_length,
                };
                crate::address_code::emit_bounds_check(index_register, bound, code)?;
                let stride_register = argument(1)?;
                crate::frame_access::load_immediate(
                    code,
                    stride_register,
                    stride,
                    Arm64DataSize::Bits64,
                );
                code.append(Arm64Instruction::MultiplyAdd {
                    size: Arm64DataSize::Bits64,
                    destination,
                    left: index_register,
                    right: stride_register,
                    addend: Arm64DataRegister::General(destination),
                    subtract_product: false,
                });
            }
        }
    }
    crate::address_code::move_register(code, destination, argument(0)?);
    Ok(())
}

fn load_index(
    plan: &Arm64AsyncFunctionPlan,
    source: Arm64AsyncAddressIndex,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncCancelError> {
    match source {
        Arm64AsyncAddressIndex::Constant(value) => {
            crate::frame_access::load_immediate(code, destination, value, Arm64DataSize::Bits64);
        }
        Arm64AsyncAddressIndex::Frame(field) => {
            load_frame_word(plan, field, 0, destination, code)?;
        }
    }
    Ok(())
}

fn emit_region_release(
    plan: &Arm64AsyncFunctionPlan,
    field: Arm64AsyncFrameField,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncCancelError> {
    let loop_ = code.create_label();
    let complete = code.create_label();
    code.bind(loop_)?;
    let mapping = load_mapping(plan, scratch(1)?, code)?;
    let page = argument(0)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        page,
        mapping,
        checked_add(
            field.offset(),
            crate::region_layout::Arm64RegionLayout::HEAD_OFFSET,
        )?,
    );
    compare_zero(page, code);
    code.branch_conditional(complete, Arm64BranchCondition::Equal);
    let next = scratch(0)?;
    crate::address_code::load_native(code, Arm64LoadStoreSize::Double, None, next, page, 0);
    crate::address_code::store_native(
        code,
        Arm64LoadStoreSize::Double,
        next,
        mapping,
        checked_add(
            field.offset(),
            crate::region_layout::Arm64RegionLayout::HEAD_OFFSET,
        )?,
    );
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        argument(1)?,
        page,
        Arm64NocterAbi::word_size(),
    );
    crate::darwin_memory_code::emit_unmap(
        code,
        crate::runtime_trap::Arm64RuntimeTrap::RegionReleaseFailure,
    )?;
    code.branch(loop_, false);
    code.bind(complete)?;
    Ok(())
}

fn load_frame_word(
    plan: &Arm64AsyncFunctionPlan,
    field: Arm64AsyncFrameField,
    offset: u64,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncCancelError> {
    let mapping = load_mapping(plan, argument(1)?, code)?;
    crate::address_code::load_native(
        code,
        Arm64LoadStoreSize::Double,
        None,
        destination,
        mapping,
        checked_add(field.offset(), offset)?,
    );
    Ok(())
}

fn store_mapping(
    plan: &Arm64AsyncFunctionPlan,
    source: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64AsyncCancelError> {
    crate::frame_access::store_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        source,
        mapping_offset(plan)?,
    );
    Ok(())
}

fn load_mapping(
    plan: &Arm64AsyncFunctionPlan,
    destination: Arm64Register,
    code: &mut Arm64CodeBuilder,
) -> Result<Arm64Register, Arm64AsyncCancelError> {
    crate::frame_access::load_at_stack_offset(
        code,
        Arm64LoadStoreSize::Double,
        destination,
        mapping_offset(plan)?,
    );
    Ok(destination)
}

fn mapping_offset(plan: &Arm64AsyncFunctionPlan) -> Result<u64, Arm64AsyncCancelError> {
    plan.cancellation()
        .activation()
        .layout()
        .object(plan.cancellation().activation().mapping())
        .map(crate::Arm64FrameObject::offset)
        .ok_or(Arm64AsyncCancelError::InvalidActivation)
}

fn add_register(destination: Arm64Register, source: Arm64Register, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64DataRegister::General(destination),
        left: Arm64DataRegister::General(destination),
        right: Arm64DataRegister::General(source),
    });
}

fn compare_zero(value: Arm64Register, code: &mut Arm64CodeBuilder) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: 0,
        shift_12: false,
    });
}

fn checked_add(left: u64, right: u64) -> Result<u64, Arm64AsyncCancelError> {
    left.checked_add(right)
        .ok_or(Arm64AsyncCancelError::OffsetOverflow)
}

fn argument(index: u8) -> Result<Arm64Register, Arm64AsyncCancelError> {
    Arm64NocterAbi::argument_register(index).ok_or(Arm64AsyncCancelError::RegisterOverflow)
}

fn scratch(index: u8) -> Result<Arm64Register, Arm64AsyncCancelError> {
    Arm64NocterAbi::compiler_scratch_register(index).ok_or(Arm64AsyncCancelError::RegisterOverflow)
}

#[derive(Debug)]
pub enum Arm64AsyncCancelError {
    ForeignTarget {
        expected: nocter_machine::MachineFunctionId,
        actual: nocter_machine::MachineFunctionId,
    },
    ImmediateTarget(nocter_machine::MachineFunctionId),
    UnknownDestruction(nocter_machine::MachineFunctionId),
    MissingPackInput,
    MissingCompletedOutput,
    InvalidActivation,
    RegisterOverflow,
    OffsetOverflow,
    Materialization(crate::Arm64MaterializationError),
    Code(Arm64CodeError),
}

impl fmt::Display for Arm64AsyncCancelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 async cancellation emission failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64AsyncCancelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Materialization(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::ForeignTarget { .. }
            | Self::ImmediateTarget(_)
            | Self::UnknownDestruction(_)
            | Self::MissingPackInput
            | Self::MissingCompletedOutput
            | Self::InvalidActivation
            | Self::RegisterOverflow
            | Self::OffsetOverflow => None,
        }
    }
}

impl From<crate::Arm64MaterializationError> for Arm64AsyncCancelError {
    fn from(error: crate::Arm64MaterializationError) -> Self {
        Self::Materialization(error)
    }
}

impl From<Arm64CodeError> for Arm64AsyncCancelError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}
