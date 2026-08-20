use nocter_machine::{
    MachineAddressId, MachineBinaryOperation, MachineBlockId, MachineBranchTarget,
    MachineComparisonOperation, MachineComparisonRepresentation, MachineConstant, MachineDataId,
    MachineFunctionId, MachineLayoutKind, MachineOperationId, MachineOperationKind, MachineScalar,
    MachineTerminator, MachineUnaryOperation, MachineValueId,
};

use crate::{
    Arm64DataSize, Arm64FunctionFrame, Arm64LoadStoreSize, Arm64Register, Arm64SelectionError,
    Arm64ValuePlan, Arm64ValueStorage, Arm64VirtualRegister,
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
pub enum Arm64SelectedMemoryAddress {
    Stack(Arm64SelectedStackAddress),
    Register {
        base: Arm64SelectedRegister,
        offset: u64,
    },
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
    LoadMemory {
        bytes: u8,
        extension: Arm64SelectedLoadExtension,
        destination: Arm64SelectedRegister,
        source: Arm64SelectedMemoryAddress,
    },
    StoreMemory {
        bytes: u8,
        destination: Arm64SelectedMemoryAddress,
        source: Arm64SelectedRegister,
    },
    ZeroStack {
        destination: Arm64SelectedStackAddress,
        bytes: u64,
    },
    /// Exact copy between storage domains proven distinct during selection.
    CopyMemoryNonOverlapping {
        destination: Arm64SelectedMemoryAddress,
        source: Arm64SelectedMemoryAddress,
        bytes: u64,
    },
    ResolveAddress(MachineAddressId),
    IndexAddress {
        destination: Arm64SelectedRegister,
        index: Arm64SelectedRegister,
        domain: Arm64SelectedIndexAddressDomain,
    },
    MemoryAddress {
        destination: Arm64SelectedRegister,
        source: Arm64SelectedMemoryAddress,
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
pub enum Arm64SelectedIndexAddressDomain {
    Fixed {
        pointer: Arm64SelectedRegister,
        length: u64,
        stride: u64,
    },
    View {
        pointer: Arm64SelectedRegister,
        length: Arm64SelectedRegister,
        stride: u64,
    },
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
    addresses: crate::Arm64SelectedAddressPlan,
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
        let addresses = crate::Arm64SelectedAddressPlan::build(function, &values, &frame)?;
        let entry_instructions = crate::call_selection::select_parameters(function, &frame)?;
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
                    &addresses,
                    &mut instructions,
                )?;
            }
            let terminator = select_terminator(
                function,
                block_id,
                block.terminator(),
                &values,
                &frame,
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
            addresses,
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
    pub const fn addresses(&self) -> &crate::Arm64SelectedAddressPlan {
        &self.addresses
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

fn select_operation(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    operation_id: MachineOperationId,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    addresses: &crate::Arm64SelectedAddressPlan,
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
        MachineOperationKind::Load { .. }
        | MachineOperationKind::Store { .. }
        | MachineOperationKind::AddressOf { .. } => crate::memory_selection::select_operation(
            (program, owner),
            operation_id,
            operation,
            values,
            frame,
            addresses,
            selected,
        ),
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
        MachineOperationKind::IndexBorrow(_) | MachineOperationKind::BorrowWeakening { .. } => {
            crate::structural_selection::select_operation(operation_id, operation, values, selected)
        }
        MachineOperationKind::Aggregate(aggregate) => select_aggregate_operation(
            (program, owner),
            operation_id,
            aggregate,
            operation.result(),
            values,
            frame,
            selected,
        ),
        MachineOperationKind::Call(call) => crate::call_selection::select_call(
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
            kind: crate::selection_error::operation_name(unsupported),
        }),
    }
}

fn select_aggregate_operation(
    scope: (&nocter_machine::MachineProgram, MachineFunctionId),
    operation: MachineOperationId,
    aggregate: &nocter_machine::MachineAggregate,
    result: Option<MachineValueId>,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    crate::aggregate_selection::select_aggregate(
        scope.0,
        scope.1,
        aggregate,
        result.ok_or(Arm64SelectionError::MissingResult(operation))?,
        values,
        frame,
        selected,
    )
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
    frame: &Arm64FunctionFrame,
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
            crate::call_selection::select_return(function, block, *value, values, frame, selected)?;
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

fn one_word(
    values: &Arm64ValuePlan,
    value: MachineValueId,
) -> Result<Arm64SelectedRegister, Arm64SelectionError> {
    match direct_value(values, value)? {
        [register] => Ok(Arm64SelectedRegister::Virtual(*register)),
        _ => Err(Arm64SelectionError::ExpectedOneWord(value)),
    }
}
