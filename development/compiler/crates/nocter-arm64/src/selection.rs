use nocter_machine::{
    MachineArgumentLocation, MachineBinaryOperation, MachineBlockId, MachineBranchTarget,
    MachineCall, MachineCallAllocation, MachineCallTarget, MachineComparisonOperation,
    MachineComparisonRepresentation, MachineConstant, MachineDataId, MachineFunctionId,
    MachineFunctionKind, MachineLayoutKind, MachineOperationId, MachineOperationKind,
    MachineResultAbi, MachineResultLocation, MachineScalar, MachineTerminator,
    MachineUnaryOperation, MachineValueClass, MachineValueId,
};

use crate::{
    Arm64DataSize, Arm64FunctionFrame, Arm64LoadStoreSize, Arm64NocterAbi, Arm64Register,
    Arm64SelectionError, Arm64ValuePlan, Arm64ValueStorage, Arm64VirtualRegister,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedRegister {
    Virtual(Arm64VirtualRegister),
    Fixed(Arm64Register),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedStackAddress {
    FrameObject {
        object: crate::Arm64FrameObjectId,
        offset: u64,
    },
    Outgoing(u64),
    Incoming(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedInstruction {
    LoadImmediate {
        size: Arm64DataSize,
        destination: Arm64SelectedRegister,
        value: u64,
    },
    LoadDataAddress {
        destination: Arm64SelectedRegister,
        source: MachineDataId,
    },
    Move {
        size: Arm64DataSize,
        destination: Arm64SelectedRegister,
        source: Arm64SelectedRegister,
    },
    LoadStack {
        bytes: u8,
        extension: Arm64SelectedLoadExtension,
        destination: Arm64SelectedRegister,
        source: Arm64SelectedStackAddress,
    },
    StoreStack {
        bytes: u8,
        destination: Arm64SelectedStackAddress,
        source: Arm64SelectedRegister,
    },
    StackAddress {
        destination: Arm64SelectedRegister,
        source: Arm64SelectedStackAddress,
    },
    Unary {
        size: Arm64DataSize,
        operation: Arm64SelectedUnaryOperation,
        destination: Arm64SelectedRegister,
        operand: Arm64SelectedRegister,
    },
    Binary {
        size: Arm64DataSize,
        operation: Arm64SelectedBinaryOperation,
        destination: Arm64SelectedRegister,
        left: Arm64SelectedRegister,
        right: Arm64SelectedRegister,
    },
    CompareBorrowed {
        size: Arm64LoadStoreSize,
        extension: Arm64SelectedLoadExtension,
        operation: Arm64SelectedComparisonOperation,
        offset: u64,
        destination: Arm64SelectedRegister,
        left: Arm64SelectedRegister,
        right: Arm64SelectedRegister,
    },
    Call(MachineFunctionId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedLoadExtension {
    Zero,
    Sign(Arm64DataSize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedUnaryOperation {
    LogicalNot,
    Negate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide { signed: bool },
    Remainder { signed: bool },
    ShiftLeft,
    ShiftRight { signed: bool },
    Equal,
    Less { signed: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedComparisonOperation {
    Equal,
    Less { signed: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64SelectedCopy {
    destination: Arm64SelectedRegister,
    source: Arm64SelectedRegister,
}

impl Arm64SelectedCopy {
    #[must_use]
    pub const fn destination(self) -> Arm64SelectedRegister {
        self.destination
    }

    #[must_use]
    pub const fn source(self) -> Arm64SelectedRegister {
        self.source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64SelectedEdge {
    target: MachineBlockId,
    copies: Box<[Arm64SelectedCopy]>,
}

impl Arm64SelectedEdge {
    #[must_use]
    pub const fn target(&self) -> MachineBlockId {
        self.target
    }

    #[must_use]
    pub const fn copies(&self) -> &[Arm64SelectedCopy] {
        &self.copies
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Arm64SelectedTerminator {
    Goto(Arm64SelectedEdge),
    Branch {
        condition: Arm64SelectedRegister,
        then_edge: Arm64SelectedEdge,
        else_edge: Arm64SelectedEdge,
    },
    Return,
    Exit(Option<Arm64SelectedRegister>),
    Trap,
    Unreachable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64SelectedBlock {
    instructions: Box<[Arm64SelectedInstruction]>,
    terminator: Arm64SelectedTerminator,
}

impl Arm64SelectedBlock {
    #[must_use]
    pub const fn instructions(&self) -> &[Arm64SelectedInstruction] {
        &self.instructions
    }

    #[must_use]
    pub const fn terminator(&self) -> &Arm64SelectedTerminator {
        &self.terminator
    }
}

/// Target-selected operations, value allocation, and fixed frame for one machine function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64SelectedFunction {
    owner: MachineFunctionId,
    values: Arm64ValuePlan,
    frame: Arm64FunctionFrame,
    entry_instructions: Box<[Arm64SelectedInstruction]>,
    blocks: Box<[(MachineBlockId, Arm64SelectedBlock)]>,
    entry: MachineBlockId,
}

impl Arm64SelectedFunction {
    /// Selects the currently executable scalar/call/control slice.
    ///
    /// Unsupported machine operations fail explicitly and never survive as passthrough nodes.
    ///
    /// # Errors
    ///
    /// Rejects malformed machine identities, value/frame planning failures, and operations or
    /// ABI transports outside the currently closed selection slice.
    pub fn build(
        program: &nocter_machine::MachineProgram,
        owner: MachineFunctionId,
    ) -> Result<Self, Arm64SelectionError> {
        let function = program
            .function(owner)
            .ok_or(Arm64SelectionError::UnknownFunction(owner))?;
        let values = Arm64ValuePlan::build(function)?;
        let frame = Arm64FunctionFrame::build(program, owner, &values)?;
        let entry_instructions = select_parameters(function, &frame)?;
        let mut blocks = Vec::with_capacity(function.body().blocks().len());
        for (block_id, block) in function.body().blocks() {
            if block_id.index() != blocks.len() {
                return Err(Arm64SelectionError::NonDenseBlock(block_id));
            }
            let mut instructions = Vec::new();
            for operation_id in block.operations() {
                select_operation(
                    program,
                    owner,
                    *operation_id,
                    &values,
                    &frame,
                    &mut instructions,
                )?;
            }
            let terminator = select_terminator(
                function,
                block_id,
                block.terminator(),
                &values,
                &mut instructions,
            )?;
            blocks.push((
                block_id,
                Arm64SelectedBlock {
                    instructions: instructions.into_boxed_slice(),
                    terminator,
                },
            ));
        }
        Ok(Self {
            owner,
            values,
            frame,
            entry_instructions,
            blocks: blocks.into_boxed_slice(),
            entry: function.body().entry(),
        })
    }

    #[must_use]
    pub const fn owner(&self) -> MachineFunctionId {
        self.owner
    }

    #[must_use]
    pub const fn values(&self) -> &Arm64ValuePlan {
        &self.values
    }

    #[must_use]
    pub const fn frame(&self) -> &Arm64FunctionFrame {
        &self.frame
    }

    #[must_use]
    pub const fn entry_instructions(&self) -> &[Arm64SelectedInstruction] {
        &self.entry_instructions
    }

    #[must_use]
    pub fn block(&self, id: MachineBlockId) -> Option<&Arm64SelectedBlock> {
        self.blocks
            .get(id.index())
            .and_then(|(actual, block)| (*actual == id).then_some(block))
    }

    #[must_use]
    pub fn blocks(&self) -> impl ExactSizeIterator<Item = (MachineBlockId, &Arm64SelectedBlock)> {
        self.blocks.iter().map(|(id, block)| (*id, block))
    }

    #[must_use]
    pub const fn entry(&self) -> MachineBlockId {
        self.entry
    }
}

fn select_parameters(
    function: &nocter_machine::MachineFunction,
    frame: &Arm64FunctionFrame,
) -> Result<Box<[Arm64SelectedInstruction]>, Arm64SelectionError> {
    let MachineFunctionKind::Callable(abi) = function.kind() else {
        return if function.body().parameters().is_empty() {
            Ok(Vec::new().into_boxed_slice())
        } else {
            Err(Arm64SelectionError::Parameters(function.linkage()))
        };
    };
    if function.body().parameters().len() != abi.arguments().len() {
        return Err(Arm64SelectionError::Parameters(function.linkage()));
    }
    let mut selected = Vec::new();
    for (stack, argument) in function
        .body()
        .parameters()
        .iter()
        .copied()
        .zip(abi.arguments())
    {
        match (argument.class(), argument.location()) {
            (MachineValueClass::Zero, None) => {}
            (
                MachineValueClass::Direct { words },
                Some(MachineArgumentLocation::Registers(registers)),
            ) if words == registers.words() => {
                let sizes = crate::memory_selection::parameter_lane_sizes(function, stack, words)?;
                for (lane, bytes) in sizes.into_iter().enumerate() {
                    selected.push(Arm64SelectedInstruction::StoreStack {
                        bytes,
                        destination: crate::memory_selection::frame_stack(
                            frame,
                            stack,
                            crate::memory_selection::lane_offset(lane)?,
                        )?,
                        source: Arm64SelectedRegister::Fixed(argument_register(
                            registers.first(),
                            lane,
                        )?),
                    });
                }
            }
            (MachineValueClass::Direct { words }, Some(MachineArgumentLocation::Stack(slot))) => {
                let sizes = crate::memory_selection::parameter_lane_sizes(function, stack, words)?;
                let transport_size = u64::from(words)
                    .checked_mul(Arm64NocterAbi::WORD_SIZE)
                    .ok_or(Arm64SelectionError::AddressOverflow)?;
                if slot.size() < transport_size {
                    return Err(Arm64SelectionError::ParameterTransport(function.linkage()));
                }
                for (lane, bytes) in sizes.into_iter().enumerate() {
                    let offset = crate::memory_selection::lane_offset(lane)?;
                    selected.push(Arm64SelectedInstruction::LoadStack {
                        bytes,
                        extension: Arm64SelectedLoadExtension::Zero,
                        destination: Arm64SelectedRegister::Fixed(scratch_boundary()),
                        source: Arm64SelectedStackAddress::Incoming(
                            slot.offset()
                                .checked_add(offset)
                                .ok_or(Arm64SelectionError::AddressOverflow)?,
                        ),
                    });
                    selected.push(Arm64SelectedInstruction::StoreStack {
                        bytes,
                        destination: crate::memory_selection::frame_stack(frame, stack, offset)?,
                        source: Arm64SelectedRegister::Fixed(scratch_boundary()),
                    });
                }
            }
            _ => return Err(Arm64SelectionError::ParameterTransport(function.linkage())),
        }
    }
    Ok(selected.into_boxed_slice())
}

fn select_operation(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    operation_id: MachineOperationId,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let operation = program
        .function(owner)
        .and_then(|function| function.body().operation(operation_id))
        .ok_or(Arm64SelectionError::UnknownOperation {
            function: owner,
            operation: operation_id,
        })?;
    match operation.kind() {
        MachineOperationKind::Constant(constant) => {
            let result = operation
                .result()
                .ok_or(Arm64SelectionError::MissingResult(operation_id))?;
            select_constant(program, owner, *constant, result, values, selected)
        }
        MachineOperationKind::Load { source } => {
            let result = operation
                .result()
                .ok_or(Arm64SelectionError::MissingResult(operation_id))?;
            crate::memory_selection::select_direct_load(
                program, owner, *source, result, values, frame, selected,
            )
        }
        MachineOperationKind::Store { destination, value } => {
            crate::memory_selection::select_direct_store(
                program,
                owner,
                *destination,
                *value,
                values,
                frame,
                selected,
            )
        }
        MachineOperationKind::AddressOf { source } => {
            let result = operation
                .result()
                .ok_or(Arm64SelectionError::MissingResult(operation_id))?;
            selected.push(Arm64SelectedInstruction::StackAddress {
                destination: one_word(values, result)?,
                source: crate::memory_selection::select_stack_address(
                    program, owner, *source, frame,
                )?,
            });
            Ok(())
        }
        MachineOperationKind::Unary {
            operation: unary,
            operand,
        } => select_unary(
            (program, owner),
            operation_id,
            *unary,
            *operand,
            operation.result(),
            values,
            selected,
        ),
        MachineOperationKind::Binary {
            operation: binary,
            left,
            right,
        } => select_binary(
            (program, owner),
            operation_id,
            *binary,
            (*left, *right),
            operation.result(),
            values,
            selected,
        ),
        MachineOperationKind::Comparison(comparison) => select_comparison(
            operation_id,
            *comparison,
            operation.result(),
            values,
            selected,
        ),
        MachineOperationKind::Call(call) => select_call(
            program,
            operation_id,
            call,
            operation.result(),
            values,
            frame,
            selected,
        ),
        unsupported => Err(Arm64SelectionError::UnsupportedOperation {
            operation: operation_id,
            kind: operation_name(unsupported),
        }),
    }
}

fn select_constant(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    constant: MachineConstant,
    result: MachineValueId,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if let MachineConstant::Text(data) = constant {
        return crate::memory_selection::select_text_constant(
            program, owner, data, result, values, selected,
        );
    }
    let bits = match constant {
        MachineConstant::Bool(value) => u128::from(value),
        MachineConstant::Integer(value) => value.cast_unsigned(),
        MachineConstant::Text(_) => unreachable!("text constants return before scalar selection"),
    };
    let registers = direct_value(values, result)?;
    for (lane, register) in registers.iter().copied().enumerate() {
        selected.push(Arm64SelectedInstruction::LoadImmediate {
            size: Arm64DataSize::Bits64,
            destination: Arm64SelectedRegister::Virtual(register),
            value: u64::try_from((bits >> (lane * 64)) & u128::from(u64::MAX))
                .expect("one selected constant lane is exactly 64 bits"),
        });
    }
    Ok(())
}

fn select_call(
    program: &nocter_machine::MachineProgram,
    operation: MachineOperationId,
    call: &MachineCall,
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if call.pack().is_some() {
        return Err(Arm64SelectionError::CallPack(operation));
    }
    if program.allocation().call_requires_context(call)? {
        return Err(Arm64SelectionError::CallAllocation(operation));
    }
    if call.allocation() != MachineCallAllocation::Inherit {
        return Err(Arm64SelectionError::CallAllocation(operation));
    }
    let MachineCallTarget::Direct(target) = call.target() else {
        return Err(Arm64SelectionError::PrimitiveCall(operation));
    };
    let target_function = program
        .function(*target)
        .ok_or(Arm64SelectionError::UnknownFunction(*target))?;
    let MachineFunctionKind::Callable(abi) = target_function.kind() else {
        return Err(Arm64SelectionError::NonCallableTarget(*target));
    };
    select_call_arguments(operation, call, abi, values, frame, selected)?;
    selected.push(Arm64SelectedInstruction::Call(*target));
    match (abi.result(), result) {
        (MachineResultAbi::Completion | MachineResultAbi::Diverging, _) => Ok(()),
        (MachineResultAbi::Value(returned), Some(result)) => match returned.location() {
            MachineResultLocation::Omitted => Ok(()),
            MachineResultLocation::Registers(span) => {
                let destinations = direct_value(values, result)?;
                if usize::from(span.words()) != destinations.len() {
                    return Err(Arm64SelectionError::ResultTransport(operation));
                }
                for (lane, destination) in destinations.iter().copied().enumerate() {
                    selected.push(Arm64SelectedInstruction::Move {
                        size: Arm64DataSize::Bits64,
                        destination: Arm64SelectedRegister::Virtual(destination),
                        source: Arm64SelectedRegister::Fixed(argument_register(
                            span.first(),
                            lane,
                        )?),
                    });
                }
                Ok(())
            }
            MachineResultLocation::CallerStorage { .. } => {
                Err(Arm64SelectionError::IndirectResult(operation))
            }
        },
        (MachineResultAbi::Value(_), None) => Err(Arm64SelectionError::MissingResult(operation)),
    }
}

fn select_call_arguments(
    operation: MachineOperationId,
    call: &MachineCall,
    abi: &nocter_machine::MachineCallableAbi,
    values: &Arm64ValuePlan,
    _frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if call.arguments().len() != abi.arguments().len() {
        return Err(Arm64SelectionError::CallArguments(operation));
    }
    for (value, argument) in call.arguments().iter().copied().zip(abi.arguments()) {
        match (argument.class(), argument.location()) {
            (MachineValueClass::Zero, None) => {}
            (
                MachineValueClass::Direct { words },
                Some(MachineArgumentLocation::Registers(registers)),
            ) if words == registers.words() => {
                let sources = direct_value(values, value)?;
                if sources.len() != usize::from(words) {
                    return Err(Arm64SelectionError::CallArguments(operation));
                }
                for (lane, source) in sources.iter().copied().enumerate() {
                    selected.push(Arm64SelectedInstruction::Move {
                        size: Arm64DataSize::Bits64,
                        destination: Arm64SelectedRegister::Fixed(argument_register(
                            registers.first(),
                            lane,
                        )?),
                        source: Arm64SelectedRegister::Virtual(source),
                    });
                }
            }
            (MachineValueClass::Direct { words }, Some(MachineArgumentLocation::Stack(slot))) => {
                let sources = direct_value(values, value)?;
                let transport_size = u64::from(words)
                    .checked_mul(Arm64NocterAbi::WORD_SIZE)
                    .ok_or(Arm64SelectionError::AddressOverflow)?;
                if sources.len() != usize::from(words) || slot.size() < transport_size {
                    return Err(Arm64SelectionError::CallArguments(operation));
                }
                for (lane, source) in sources.iter().copied().enumerate() {
                    selected.push(Arm64SelectedInstruction::StoreStack {
                        bytes: u8::try_from(Arm64NocterAbi::WORD_SIZE)
                            .expect("one ABI word fits the selected byte width"),
                        destination: Arm64SelectedStackAddress::Outgoing(
                            slot.offset()
                                .checked_add(crate::memory_selection::lane_offset(lane)?)
                                .ok_or(Arm64SelectionError::AddressOverflow)?,
                        ),
                        source: Arm64SelectedRegister::Virtual(source),
                    });
                }
            }
            _ => return Err(Arm64SelectionError::CallArguments(operation)),
        }
    }
    Ok(())
}

fn select_unary(
    scope: (&nocter_machine::MachineProgram, MachineFunctionId),
    operation_id: MachineOperationId,
    operation: MachineUnaryOperation,
    operand: MachineValueId,
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let result = result.ok_or(Arm64SelectionError::MissingResult(operation_id))?;
    let (size, _) = scalar_value(scope.0, scope.1, operand)?;
    selected.push(Arm64SelectedInstruction::Unary {
        size,
        operation: match operation {
            MachineUnaryOperation::LogicalNot => Arm64SelectedUnaryOperation::LogicalNot,
            MachineUnaryOperation::Negate => Arm64SelectedUnaryOperation::Negate,
        },
        destination: one_word(values, result)?,
        operand: one_word(values, operand)?,
    });
    Ok(())
}

fn select_binary(
    scope: (&nocter_machine::MachineProgram, MachineFunctionId),
    operation_id: MachineOperationId,
    operation: MachineBinaryOperation,
    operands: (MachineValueId, MachineValueId),
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let (left, right) = operands;
    let result = result.ok_or(Arm64SelectionError::MissingResult(operation_id))?;
    let (size, signed) = scalar_value(scope.0, scope.1, left)?;
    let selected_operation = match operation {
        MachineBinaryOperation::Add => Arm64SelectedBinaryOperation::Add,
        MachineBinaryOperation::Subtract => Arm64SelectedBinaryOperation::Subtract,
        MachineBinaryOperation::Multiply => Arm64SelectedBinaryOperation::Multiply,
        MachineBinaryOperation::Divide => Arm64SelectedBinaryOperation::Divide { signed },
        MachineBinaryOperation::Remainder => Arm64SelectedBinaryOperation::Remainder { signed },
        MachineBinaryOperation::ShiftLeft => Arm64SelectedBinaryOperation::ShiftLeft,
        MachineBinaryOperation::ShiftRightSigned => {
            Arm64SelectedBinaryOperation::ShiftRight { signed: true }
        }
        MachineBinaryOperation::ShiftRightUnsigned => {
            Arm64SelectedBinaryOperation::ShiftRight { signed: false }
        }
        MachineBinaryOperation::Equal => Arm64SelectedBinaryOperation::Equal,
        MachineBinaryOperation::Less => Arm64SelectedBinaryOperation::Less { signed },
    };
    selected.push(Arm64SelectedInstruction::Binary {
        size,
        operation: selected_operation,
        destination: one_word(values, result)?,
        left: one_word(values, left)?,
        right: one_word(values, right)?,
    });
    Ok(())
}

fn select_comparison(
    operation_id: MachineOperationId,
    comparison: nocter_machine::MachineComparison,
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let result = result.ok_or(Arm64SelectionError::MissingResult(operation_id))?;
    let (size, extension, offset, signed) = match comparison.representation() {
        MachineComparisonRepresentation::Scalar(scalar) => {
            let (size, extension, signed) =
                crate::memory_selection::scalar_memory_representation(scalar)?;
            (size, extension, 0, signed)
        }
        MachineComparisonRepresentation::Tag { offset } => (
            Arm64LoadStoreSize::Byte,
            Arm64SelectedLoadExtension::Zero,
            offset,
            false,
        ),
    };
    let operation = match comparison.operation() {
        MachineComparisonOperation::Equal => Arm64SelectedComparisonOperation::Equal,
        MachineComparisonOperation::Less => Arm64SelectedComparisonOperation::Less { signed },
    };
    selected.push(Arm64SelectedInstruction::CompareBorrowed {
        size,
        extension,
        operation,
        offset,
        destination: one_word(values, result)?,
        left: one_word(values, comparison.left())?,
        right: one_word(values, comparison.right())?,
    });
    Ok(())
}

fn select_terminator(
    function: &nocter_machine::MachineFunction,
    block: MachineBlockId,
    terminator: &MachineTerminator,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<Arm64SelectedTerminator, Arm64SelectionError> {
    match terminator {
        MachineTerminator::Goto(target) => Ok(Arm64SelectedTerminator::Goto(select_edge(
            function, target, values,
        )?)),
        MachineTerminator::Branch {
            condition,
            then_target,
            else_target,
        } => Ok(Arm64SelectedTerminator::Branch {
            condition: one_word(values, *condition)?,
            then_edge: select_edge(function, then_target, values)?,
            else_edge: select_edge(function, else_target, values)?,
        }),
        MachineTerminator::Return(value) => {
            select_return(function, block, *value, values, selected)?;
            Ok(Arm64SelectedTerminator::Return)
        }
        MachineTerminator::Exit(value) => Ok(Arm64SelectedTerminator::Exit(
            value.map(|value| one_word(values, value)).transpose()?,
        )),
        MachineTerminator::Trap => Ok(Arm64SelectedTerminator::Trap),
        MachineTerminator::Unreachable => Ok(Arm64SelectedTerminator::Unreachable),
        _ => Err(Arm64SelectionError::UnsupportedTerminator(block)),
    }
}

fn select_edge(
    function: &nocter_machine::MachineFunction,
    target: &MachineBranchTarget,
    values: &Arm64ValuePlan,
) -> Result<Arm64SelectedEdge, Arm64SelectionError> {
    let parameters = function
        .body()
        .block(target.block())
        .map(nocter_machine::MachineBlock::parameters)
        .ok_or(Arm64SelectionError::UnknownBlock(target.block()))?;
    if parameters.len() != target.arguments().len() {
        return Err(Arm64SelectionError::EdgeArity(target.block()));
    }
    let mut copies = Vec::new();
    for (argument, parameter) in target
        .arguments()
        .iter()
        .copied()
        .zip(parameters.iter().copied())
    {
        match (values.value(argument), values.value(parameter)) {
            (Some(Arm64ValueStorage::Omitted), Some(Arm64ValueStorage::Omitted)) => {}
            (
                Some(Arm64ValueStorage::Direct(sources)),
                Some(Arm64ValueStorage::Direct(destinations)),
            ) if sources.len() == destinations.len() => {
                copies.extend(
                    sources
                        .iter()
                        .zip(destinations)
                        .map(|(source, destination)| Arm64SelectedCopy {
                            destination: Arm64SelectedRegister::Virtual(*destination),
                            source: Arm64SelectedRegister::Virtual(*source),
                        }),
                );
            }
            (Some(_), Some(_)) => return Err(Arm64SelectionError::EdgeTransport(target.block())),
            (None, _) => return Err(Arm64SelectionError::UnknownValue(argument)),
            (_, None) => return Err(Arm64SelectionError::UnknownValue(parameter)),
        }
    }
    Ok(Arm64SelectedEdge {
        target: target.block(),
        copies: copies.into_boxed_slice(),
    })
}

fn select_return(
    function: &nocter_machine::MachineFunction,
    block: MachineBlockId,
    value: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let MachineFunctionKind::Callable(abi) = function.kind() else {
        return Err(Arm64SelectionError::RootReturn(block));
    };
    match (abi.result(), value) {
        (MachineResultAbi::Completion, None) | (MachineResultAbi::Diverging, _) => Ok(()),
        (MachineResultAbi::Value(returned), Some(value)) => match returned.location() {
            MachineResultLocation::Omitted => Ok(()),
            MachineResultLocation::Registers(span) => {
                let sources = direct_value(values, value)?;
                if usize::from(span.words()) != sources.len() {
                    return Err(Arm64SelectionError::ReturnTransport(block));
                }
                for (lane, source) in sources.iter().copied().enumerate() {
                    selected.push(Arm64SelectedInstruction::Move {
                        size: Arm64DataSize::Bits64,
                        destination: Arm64SelectedRegister::Fixed(argument_register(
                            span.first(),
                            lane,
                        )?),
                        source: Arm64SelectedRegister::Virtual(source),
                    });
                }
                Ok(())
            }
            MachineResultLocation::CallerStorage { .. } => {
                Err(Arm64SelectionError::IndirectReturn(block))
            }
        },
        _ => Err(Arm64SelectionError::ReturnTransport(block)),
    }
}

pub(crate) fn direct_value(
    values: &Arm64ValuePlan,
    value: MachineValueId,
) -> Result<&[Arm64VirtualRegister], Arm64SelectionError> {
    match values
        .value(value)
        .ok_or(Arm64SelectionError::UnknownValue(value))?
    {
        Arm64ValueStorage::Direct(registers) => Ok(registers),
        Arm64ValueStorage::Omitted => Ok(&[]),
        Arm64ValueStorage::Memory { .. } => Err(Arm64SelectionError::MemoryValue(value)),
    }
}

fn scalar_value(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    value: MachineValueId,
) -> Result<(Arm64DataSize, bool), Arm64SelectionError> {
    let ty = program
        .function(owner)
        .and_then(|function| function.body().value(value))
        .map(nocter_machine::MachineValue::ty)
        .ok_or(Arm64SelectionError::UnknownValue(value))?;
    match program
        .layouts()
        .get(ty)
        .map(nocter_machine::MachineLayout::kind)
    {
        Some(MachineLayoutKind::Scalar(MachineScalar::Bool)) => Ok((Arm64DataSize::Bits32, false)),
        Some(MachineLayoutKind::Scalar(MachineScalar::Integer { bits, signed })) => {
            let size = match bits {
                1..=32 => Arm64DataSize::Bits32,
                33..=64 => Arm64DataSize::Bits64,
                _ => return Err(Arm64SelectionError::UnsupportedScalar(value)),
            };
            Ok((size, *signed))
        }
        _ => Err(Arm64SelectionError::UnsupportedScalar(value)),
    }
}

fn scratch_boundary() -> Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(1)
        .expect("the ABI reserves x17 for selection boundary staging")
}

fn one_word(
    values: &Arm64ValuePlan,
    value: MachineValueId,
) -> Result<Arm64SelectedRegister, Arm64SelectionError> {
    match direct_value(values, value)? {
        [register] => Ok(Arm64SelectedRegister::Virtual(*register)),
        _ => Err(Arm64SelectionError::ExpectedOneWord(value)),
    }
}

fn argument_register(first: u8, lane: usize) -> Result<Arm64Register, Arm64SelectionError> {
    let lane = u8::try_from(lane).map_err(|_| Arm64SelectionError::RegisterOverflow)?;
    first
        .checked_add(lane)
        .and_then(Arm64NocterAbi::argument_register)
        .ok_or(Arm64SelectionError::RegisterOverflow)
}

const fn operation_name(operation: &MachineOperationKind) -> &'static str {
    match operation {
        MachineOperationKind::Constant(_) => "constant",
        MachineOperationKind::Load { .. } => "load",
        MachineOperationKind::AddressOf { .. } => "address-of",
        MachineOperationKind::Store { .. } => "store",
        MachineOperationKind::Unary { .. } => "unary",
        MachineOperationKind::Binary { .. } => "binary",
        MachineOperationKind::IntegerConversion { .. } => "integer-conversion",
        MachineOperationKind::Comparison(_) => "comparison",
        MachineOperationKind::IndexBorrow(_) => "index-borrow",
        MachineOperationKind::BorrowWeakening { .. } => "borrow-weakening",
        MachineOperationKind::Aggregate(_) => "aggregate",
        MachineOperationKind::InvokeDrop { .. } => "invoke-drop",
        MachineOperationKind::ReportError { .. } => "report-error",
        MachineOperationKind::CreateRegion { .. } => "create-region",
        MachineOperationKind::ReleaseRegion { .. } => "release-region",
        MachineOperationKind::SetDropFlag { .. } => "set-drop-flag",
        MachineOperationKind::Call(_) => "call",
        MachineOperationKind::PackLength => "pack-length",
        MachineOperationKind::PackNext => "pack-next",
        MachineOperationKind::DestroyPack => "destroy-pack",
    }
}
