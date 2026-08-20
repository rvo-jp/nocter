use std::fmt;

use nocter_machine::{
    MachineBlockId, MachineFunction, MachineOperationId, MachineOperationKind,
    MachineValueDefinition, MachineValueId, MachineValueRepresentation,
};

use crate::{
    Arm64NocterAbi, Arm64RegisterAllocation, Arm64RegisterAllocationBuilder,
    Arm64RegisterAllocationError, Arm64VirtualRegister,
};

const DIRECT_VALUE_LIMIT: u64 =
    Arm64NocterAbi::DIRECT_VALUE_WORD_LIMIT as u64 * Arm64NocterAbi::WORD_SIZE;

/// Storage selected for one target-independent machine value before frame placement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Arm64ValueStorage {
    /// Completion, divergence, and zero-sized stored values have no runtime lane.
    Omitted,
    /// One or two word lanes participating in the common register allocator.
    Direct(Box<[Arm64VirtualRegister]>),
    /// A stored value whose bytes remain in a fixed-frame object.
    Memory { size: u64, alignment: u64 },
}

impl Arm64ValueStorage {
    #[must_use]
    pub fn direct_registers(&self) -> Option<&[Arm64VirtualRegister]> {
        match self {
            Self::Direct(registers) => Some(registers),
            Self::Omitted | Self::Memory { .. } => None,
        }
    }
}

/// Dense value-storage and physical-register plan for one machine function.
///
/// Machine dataflow is the sole source of dependencies and call-crossing facts. Flattened block
/// order supplies deterministic interval positions only; it cannot make a value call-crossing on
/// an unrelated CFG path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64ValuePlan {
    owner: nocter_machine::MachineLinkageId,
    values: Box<[Arm64ValueStorage]>,
    registers: Arm64RegisterAllocation,
}

impl Arm64ValuePlan {
    /// Selects runtime storage and allocates every direct word lane.
    ///
    /// # Errors
    ///
    /// Rejects inconsistent dense identities, missing dataflow facts, position overflow, or an
    /// invalid live range.
    pub fn build(function: &MachineFunction) -> Result<Self, Arm64ValuePlanError> {
        let body = function.body();
        let schedule = FunctionSchedule::build(function)?;
        let mut register_builder = Arm64RegisterAllocationBuilder::new();
        let mut values = Vec::with_capacity(body.values().len());
        for (value_id, value) in body.values() {
            if value_id.index() != values.len() {
                return Err(Arm64ValuePlanError::NonDenseValue(value_id));
            }
            let storage = match value.representation() {
                MachineValueRepresentation::Completion | MachineValueRepresentation::Diverging => {
                    Arm64ValueStorage::Omitted
                }
                MachineValueRepresentation::Stored { size: 0, .. } => Arm64ValueStorage::Omitted,
                MachineValueRepresentation::Stored { size, alignment: _ }
                    if size <= DIRECT_VALUE_LIMIT =>
                {
                    let definition = schedule.definition_position(value.definition())?;
                    let words = size.div_ceil(Arm64NocterAbi::WORD_SIZE);
                    let registers = (0..words)
                        .map(|_| register_builder.define(definition))
                        .collect::<Vec<_>>();
                    Arm64ValueStorage::Direct(registers.into_boxed_slice())
                }
                MachineValueRepresentation::Stored { size, alignment } => {
                    Arm64ValueStorage::Memory { size, alignment }
                }
            };
            values.push(storage);
        }

        let values = values.into_boxed_slice();
        apply_liveness(function, &schedule, &values, &mut register_builder)?;
        Ok(Self {
            owner: function.linkage(),
            values,
            registers: register_builder.finish(),
        })
    }

    #[must_use]
    pub const fn owner(&self) -> nocter_machine::MachineLinkageId {
        self.owner
    }

    #[must_use]
    pub fn value(&self, id: MachineValueId) -> Option<&Arm64ValueStorage> {
        self.values.get(id.index())
    }

    #[must_use]
    pub const fn registers(&self) -> &Arm64RegisterAllocation {
        &self.registers
    }
}

#[derive(Clone, Copy, Debug)]
struct OperationPoint {
    before: usize,
    after: usize,
}

#[derive(Clone, Copy, Debug)]
struct BlockPoint {
    entry: usize,
    terminator: usize,
}

#[derive(Debug)]
struct FunctionSchedule {
    operations: Box<[Option<OperationPoint>]>,
    blocks: Box<[Option<BlockPoint>]>,
}

impl FunctionSchedule {
    fn build(function: &MachineFunction) -> Result<Self, Arm64ValuePlanError> {
        let body = function.body();
        let mut operations = vec![None; body.operations().len()];
        let mut blocks = vec![None; body.blocks().len()];
        let mut next = 0_usize;
        for (block_id, block) in body.blocks() {
            let entry = take_position(&mut next)?;
            for operation_id in block.operations() {
                let point = OperationPoint {
                    before: take_position(&mut next)?,
                    after: take_position(&mut next)?,
                };
                let slot = operations
                    .get_mut(operation_id.index())
                    .ok_or(Arm64ValuePlanError::UnknownOperation(*operation_id))?;
                if slot.replace(point).is_some() {
                    return Err(Arm64ValuePlanError::DuplicateOperation(*operation_id));
                }
            }
            let terminator = take_position(&mut next)?;
            let slot = blocks
                .get_mut(block_id.index())
                .ok_or(Arm64ValuePlanError::UnknownBlock(block_id))?;
            if slot.replace(BlockPoint { entry, terminator }).is_some() {
                return Err(Arm64ValuePlanError::DuplicateBlock(block_id));
            }
        }
        if let Some((operation, _)) = operations
            .iter()
            .enumerate()
            .find(|(_, point)| point.is_none())
        {
            return Err(Arm64ValuePlanError::UnknownOperationId(operation));
        }
        if let Some((block, _)) = blocks.iter().enumerate().find(|(_, point)| point.is_none()) {
            return Err(Arm64ValuePlanError::UnknownBlockId(block));
        }
        Ok(Self {
            operations: operations.into_boxed_slice(),
            blocks: blocks.into_boxed_slice(),
        })
    }

    fn definition_position(
        &self,
        definition: MachineValueDefinition,
    ) -> Result<usize, Arm64ValuePlanError> {
        match definition {
            MachineValueDefinition::BlockParameter { block, .. } => self
                .block(block)
                .map(|point| point.entry)
                .ok_or(Arm64ValuePlanError::UnknownBlock(block)),
            MachineValueDefinition::Operation(operation) => self
                .operation(operation)
                .map(|point| point.after)
                .ok_or(Arm64ValuePlanError::UnknownOperation(operation)),
        }
    }

    fn operation(&self, id: MachineOperationId) -> Option<OperationPoint> {
        self.operations.get(id.index()).copied().flatten()
    }

    fn block(&self, id: MachineBlockId) -> Option<BlockPoint> {
        self.blocks.get(id.index()).copied().flatten()
    }
}

fn take_position(next: &mut usize) -> Result<usize, Arm64ValuePlanError> {
    let position = *next;
    *next = next
        .checked_add(1)
        .ok_or(Arm64ValuePlanError::PositionOverflow)?;
    Ok(position)
}

fn apply_liveness(
    function: &MachineFunction,
    schedule: &FunctionSchedule,
    values: &[Arm64ValueStorage],
    registers: &mut Arm64RegisterAllocationBuilder,
) -> Result<(), Arm64ValuePlanError> {
    let body = function.body();
    for (block_id, block) in body.blocks() {
        let point = schedule
            .block(block_id)
            .ok_or(Arm64ValuePlanError::UnknownBlock(block_id))?;
        let flow = function
            .dataflow()
            .block(block_id)
            .ok_or(Arm64ValuePlanError::MissingBlockDataflow(block_id))?;
        use_values(values, flow.live_in(), point.entry, registers)?;
        for operation_id in block.operations() {
            let operation = body
                .operation(*operation_id)
                .ok_or(Arm64ValuePlanError::UnknownOperation(*operation_id))?;
            let operation_point = schedule
                .operation(*operation_id)
                .ok_or(Arm64ValuePlanError::UnknownOperation(*operation_id))?;
            let operation_flow = function
                .dataflow()
                .operation(*operation_id)
                .ok_or(Arm64ValuePlanError::MissingOperationDataflow(*operation_id))?;
            use_values(
                values,
                operation_flow.inputs(),
                operation_point.before,
                registers,
            )?;
            use_values(
                values,
                operation_flow.live_after(),
                operation_point.after,
                registers,
            )?;
            if matches!(operation.kind(), MachineOperationKind::Call(_)) {
                for value in operation_flow
                    .live_after()
                    .iter()
                    .copied()
                    .filter(|value| Some(*value) != operation.result())
                {
                    for register in direct_registers(values, value)? {
                        registers.mark_call_crossing(*register)?;
                    }
                }
            }
        }
        use_values(
            values,
            flow.terminator_inputs(),
            point.terminator,
            registers,
        )?;
        use_values(values, flow.live_out(), point.terminator, registers)?;
    }
    Ok(())
}

fn use_values(
    values: &[Arm64ValueStorage],
    used: &[MachineValueId],
    position: usize,
    registers: &mut Arm64RegisterAllocationBuilder,
) -> Result<(), Arm64ValuePlanError> {
    for value in used {
        for register in direct_registers(values, *value)? {
            registers.use_at(*register, position)?;
        }
    }
    Ok(())
}

fn direct_registers(
    values: &[Arm64ValueStorage],
    value: MachineValueId,
) -> Result<&[Arm64VirtualRegister], Arm64ValuePlanError> {
    values
        .get(value.index())
        .map(|storage| storage.direct_registers().unwrap_or(&[]))
        .ok_or(Arm64ValuePlanError::UnknownValue(value))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64ValuePlanError {
    NonDenseValue(MachineValueId),
    UnknownValue(MachineValueId),
    UnknownOperation(MachineOperationId),
    UnknownOperationId(usize),
    DuplicateOperation(MachineOperationId),
    MissingOperationDataflow(MachineOperationId),
    UnknownBlock(MachineBlockId),
    UnknownBlockId(usize),
    DuplicateBlock(MachineBlockId),
    MissingBlockDataflow(MachineBlockId),
    PositionOverflow,
    RegisterAllocation(Arm64RegisterAllocationError),
}

impl fmt::Display for Arm64ValuePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 value planning failed: {self:?}")
    }
}

impl std::error::Error for Arm64ValuePlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RegisterAllocation(error) => Some(error),
            Self::NonDenseValue(_)
            | Self::UnknownValue(_)
            | Self::UnknownOperation(_)
            | Self::UnknownOperationId(_)
            | Self::DuplicateOperation(_)
            | Self::MissingOperationDataflow(_)
            | Self::UnknownBlock(_)
            | Self::UnknownBlockId(_)
            | Self::DuplicateBlock(_)
            | Self::MissingBlockDataflow(_)
            | Self::PositionOverflow => None,
        }
    }
}

impl From<Arm64RegisterAllocationError> for Arm64ValuePlanError {
    fn from(error: Arm64RegisterAllocationError) -> Self {
        Self::RegisterAllocation(error)
    }
}
