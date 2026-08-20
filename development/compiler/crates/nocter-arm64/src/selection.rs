use std::fmt;

use nocter_machine::{
    MachineBlockId, MachineCall, MachineCallAllocation, MachineCallTarget, MachineConstant,
    MachineFunctionId, MachineFunctionKind, MachineOperationId, MachineOperationKind,
    MachineResultAbi, MachineResultLocation, MachineTerminator, MachineValueId,
};

use crate::{
    Arm64DataSize, Arm64FunctionFrame, Arm64FunctionFrameError, Arm64LoadStoreSize, Arm64NocterAbi,
    Arm64Register, Arm64ValuePlan, Arm64ValuePlanError, Arm64ValueStorage, Arm64VirtualRegister,
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
    Move {
        size: Arm64DataSize,
        destination: Arm64SelectedRegister,
        source: Arm64SelectedRegister,
    },
    LoadStack {
        size: Arm64LoadStoreSize,
        destination: Arm64SelectedRegister,
        source: Arm64SelectedStackAddress,
    },
    StoreStack {
        size: Arm64LoadStoreSize,
        destination: Arm64SelectedStackAddress,
        source: Arm64SelectedRegister,
    },
    StackAddress {
        destination: Arm64SelectedRegister,
        source: Arm64SelectedStackAddress,
    },
    Call(MachineFunctionId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Arm64SelectedTerminator {
    Goto(MachineBlockId),
    Branch {
        condition: Arm64SelectedRegister,
        then_block: MachineBlockId,
        else_block: MachineBlockId,
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
        let entry_instructions = select_parameters(function, &values, &frame)?;
        let mut blocks = Vec::with_capacity(function.body().blocks().len());
        for (block_id, block) in function.body().blocks() {
            if block_id.index() != blocks.len() {
                return Err(Arm64SelectionError::NonDenseBlock(block_id));
            }
            if !block.parameters().is_empty() {
                return Err(Arm64SelectionError::BlockParameters(block_id));
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
    _values: &Arm64ValuePlan,
    _frame: &Arm64FunctionFrame,
) -> Result<Box<[Arm64SelectedInstruction]>, Arm64SelectionError> {
    if function.body().parameters().is_empty() {
        return Ok(Box::new([]));
    }
    Err(Arm64SelectionError::Parameters(function.linkage()))
}

fn select_operation(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    operation_id: MachineOperationId,
    values: &Arm64ValuePlan,
    _frame: &Arm64FunctionFrame,
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
            select_constant(*constant, result, values, selected)
        }
        MachineOperationKind::Call(call) => select_call(
            program,
            operation_id,
            call,
            operation.result(),
            values,
            selected,
        ),
        unsupported => Err(Arm64SelectionError::UnsupportedOperation {
            operation: operation_id,
            kind: operation_name(unsupported),
        }),
    }
}

fn select_constant(
    constant: MachineConstant,
    result: MachineValueId,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    let bits = match constant {
        MachineConstant::Bool(value) => u128::from(value),
        MachineConstant::Integer(value) => value.cast_unsigned(),
        MachineConstant::Text(_) => return Err(Arm64SelectionError::TextConstant),
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
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<(), Arm64SelectionError> {
    if !call.arguments().is_empty() {
        return Err(Arm64SelectionError::CallArguments(operation));
    }
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

fn select_terminator(
    function: &nocter_machine::MachineFunction,
    block: MachineBlockId,
    terminator: &MachineTerminator,
    values: &Arm64ValuePlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<Arm64SelectedTerminator, Arm64SelectionError> {
    match terminator {
        MachineTerminator::Goto(target) if target.arguments().is_empty() => {
            Ok(Arm64SelectedTerminator::Goto(target.block()))
        }
        MachineTerminator::Branch {
            condition,
            then_target,
            else_target,
        } if then_target.arguments().is_empty() && else_target.arguments().is_empty() => {
            Ok(Arm64SelectedTerminator::Branch {
                condition: one_word(values, *condition)?,
                then_block: then_target.block(),
                else_block: else_target.block(),
            })
        }
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

fn direct_value(
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

#[derive(Debug)]
pub enum Arm64SelectionError {
    UnknownFunction(MachineFunctionId),
    NonCallableTarget(MachineFunctionId),
    UnknownOperation {
        function: MachineFunctionId,
        operation: MachineOperationId,
    },
    UnknownValue(MachineValueId),
    NonDenseBlock(MachineBlockId),
    BlockParameters(MachineBlockId),
    Parameters(nocter_machine::MachineLinkageId),
    MissingResult(MachineOperationId),
    UnsupportedOperation {
        operation: MachineOperationId,
        kind: &'static str,
    },
    UnsupportedTerminator(MachineBlockId),
    TextConstant,
    MemoryValue(MachineValueId),
    ExpectedOneWord(MachineValueId),
    CallArguments(MachineOperationId),
    CallPack(MachineOperationId),
    CallAllocation(MachineOperationId),
    PrimitiveCall(MachineOperationId),
    IndirectResult(MachineOperationId),
    ResultTransport(MachineOperationId),
    RootReturn(MachineBlockId),
    IndirectReturn(MachineBlockId),
    ReturnTransport(MachineBlockId),
    RegisterOverflow,
    Allocation(nocter_machine::MachineAllocationError),
    ValuePlan(Arm64ValuePlanError),
    Frame(Arm64FunctionFrameError),
}

impl fmt::Display for Arm64SelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 instruction selection failed: {self:?}")
    }
}

impl std::error::Error for Arm64SelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation(error) => Some(error),
            Self::ValuePlan(error) => Some(error),
            Self::Frame(error) => Some(error),
            _ => None,
        }
    }
}

impl From<nocter_machine::MachineAllocationError> for Arm64SelectionError {
    fn from(error: nocter_machine::MachineAllocationError) -> Self {
        Self::Allocation(error)
    }
}

impl From<Arm64ValuePlanError> for Arm64SelectionError {
    fn from(error: Arm64ValuePlanError) -> Self {
        Self::ValuePlan(error)
    }
}

impl From<Arm64FunctionFrameError> for Arm64SelectionError {
    fn from(error: Arm64FunctionFrameError) -> Self {
        Self::Frame(error)
    }
}
