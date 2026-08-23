use std::collections::BTreeSet;
use std::fmt;

use crate::identity::{MachineId, MachineTable};
use crate::{
    MachineAddressId, MachineAddressRoot, MachineAddressStep, MachineAggregateWrite,
    MachineBlockId, MachineBranchTarget, MachineCall, MachineCallAllocation, MachineIndex,
    MachineOperationId, MachineOperationKind, MachinePackId, MachinePackSegment, MachineTerminator,
    MachineValueDefinition, MachineValueId,
};

/// Exact value dependencies and liveness immediately after one machine operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineOperationDataflow {
    inputs: Box<[MachineValueId]>,
    live_after: Box<[MachineValueId]>,
}

impl MachineOperationDataflow {
    #[must_use]
    pub const fn inputs(&self) -> &[MachineValueId] {
        &self.inputs
    }

    #[must_use]
    pub const fn live_after(&self) -> &[MachineValueId] {
        &self.live_after
    }
}

/// Complete block-local and CFG liveness facts. Block parameters are local definitions and never
/// appear in `live_in`; predecessor edge arguments remain terminator inputs instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineBlockDataflow {
    uses: Box<[MachineValueId]>,
    definitions: Box<[MachineValueId]>,
    terminator_inputs: Box<[MachineValueId]>,
    live_in: Box<[MachineValueId]>,
    live_out: Box<[MachineValueId]>,
}

impl MachineBlockDataflow {
    #[must_use]
    pub const fn uses(&self) -> &[MachineValueId] {
        &self.uses
    }

    #[must_use]
    pub const fn definitions(&self) -> &[MachineValueId] {
        &self.definitions
    }

    #[must_use]
    pub const fn terminator_inputs(&self) -> &[MachineValueId] {
        &self.terminator_inputs
    }

    #[must_use]
    pub const fn live_in(&self) -> &[MachineValueId] {
        &self.live_in
    }

    #[must_use]
    pub const fn live_out(&self) -> &[MachineValueId] {
        &self.live_out
    }
}

/// One immutable dataflow authority derived from a closed machine body.
#[derive(Debug)]
pub struct MachineFunctionDataflow {
    operations: MachineTable<MachineOperationId, MachineOperationDataflow>,
    blocks: MachineTable<MachineBlockId, MachineBlockDataflow>,
}

impl MachineFunctionDataflow {
    pub(crate) fn build(body: &crate::MachineBody) -> Result<Self, MachineDataflowError> {
        validate_value_definitions(body)?;
        let mut operations = body
            .operations()
            .map(|(_, operation)| {
                Ok(MachineOperationDataflow {
                    inputs: operation_inputs(body, operation.kind())?,
                    live_after: Box::new([]),
                })
            })
            .collect::<Result<Vec<_>, MachineDataflowError>>()?;
        let ownership = operation_ownership(body)?;
        let mut facts = block_facts(body, &operations, &ownership)?;
        solve_liveness(&mut facts);
        complete_operation_liveness(body, &facts, &mut operations)?;
        let blocks = facts
            .into_iter()
            .map(BlockFacts::finish)
            .collect::<Vec<_>>();
        Ok(Self {
            operations: MachineTable::from_values(operations),
            blocks: MachineTable::from_values(blocks),
        })
    }

    #[must_use]
    pub fn operation(&self, id: MachineOperationId) -> Option<&MachineOperationDataflow> {
        self.operations.get(id)
    }

    #[must_use]
    pub fn block(&self, id: MachineBlockId) -> Option<&MachineBlockDataflow> {
        self.blocks.get(id)
    }
}

#[derive(Debug)]
struct BlockFacts {
    uses: BTreeSet<MachineValueId>,
    definitions: BTreeSet<MachineValueId>,
    terminator_inputs: BTreeSet<MachineValueId>,
    successors: BTreeSet<MachineBlockId>,
    predecessors: BTreeSet<MachineBlockId>,
    live_in: BTreeSet<MachineValueId>,
    live_out: BTreeSet<MachineValueId>,
}

impl BlockFacts {
    fn finish(self) -> MachineBlockDataflow {
        MachineBlockDataflow {
            uses: boxed(self.uses),
            definitions: boxed(self.definitions),
            terminator_inputs: boxed(self.terminator_inputs),
            live_in: boxed(self.live_in),
            live_out: boxed(self.live_out),
        }
    }
}

fn validate_value_definitions(body: &crate::MachineBody) -> Result<(), MachineDataflowError> {
    for (value_id, value) in body.values() {
        let valid = match value.definition() {
            MachineValueDefinition::BlockParameter { block, position } => body
                .block(block)
                .and_then(|block| block.parameters().get(position))
                .is_some_and(|actual| *actual == value_id),
            MachineValueDefinition::Operation(operation) => body
                .operation(operation)
                .is_some_and(|operation| operation.result() == Some(value_id)),
        };
        if !valid {
            return Err(MachineDataflowError::InvalidValueDefinition(value_id));
        }
    }
    for (block_id, block) in body.blocks() {
        for (position, value_id) in block.parameters().iter().copied().enumerate() {
            let valid = body.value(value_id).is_some_and(|value| {
                value.definition()
                    == (MachineValueDefinition::BlockParameter {
                        block: block_id,
                        position,
                    })
            });
            if !valid {
                return Err(MachineDataflowError::InvalidValueDefinition(value_id));
            }
        }
    }
    for (operation_id, operation) in body.operations() {
        if let Some(value_id) = operation.result() {
            let valid = body.value(value_id).is_some_and(|value| {
                value.definition() == MachineValueDefinition::Operation(operation_id)
            });
            if !valid {
                return Err(MachineDataflowError::InvalidValueDefinition(value_id));
            }
        }
    }
    Ok(())
}

fn operation_ownership(
    body: &crate::MachineBody,
) -> Result<Vec<MachineBlockId>, MachineDataflowError> {
    let mut owners = vec![None; body.operations().len()];
    for (block_id, block) in body.blocks() {
        for operation in block.operations() {
            let owner = owners
                .get_mut(operation.index())
                .ok_or(MachineDataflowError::UnknownOperation(*operation))?;
            if owner.replace(block_id).is_some() {
                return Err(MachineDataflowError::DuplicateOperation(*operation));
            }
        }
    }
    owners
        .into_iter()
        .enumerate()
        .map(|(index, owner)| {
            owner.ok_or_else(|| {
                MachineDataflowError::UnownedOperation(MachineOperationId::new(index))
            })
        })
        .collect()
}

fn block_facts(
    body: &crate::MachineBody,
    operations: &[MachineOperationDataflow],
    ownership: &[MachineBlockId],
) -> Result<Vec<BlockFacts>, MachineDataflowError> {
    if body.block(body.entry()).is_none() {
        return Err(MachineDataflowError::UnknownBlock(body.entry()));
    }
    let mut facts = Vec::with_capacity(body.blocks().len());
    for (block_id, block) in body.blocks() {
        let mut definitions = block.parameters().iter().copied().collect::<BTreeSet<_>>();
        for parameter in block.parameters() {
            require_value(body, *parameter)?;
        }
        let mut uses = BTreeSet::new();
        for operation_id in block.operations() {
            if ownership.get(operation_id.index()) != Some(&block_id) {
                return Err(MachineDataflowError::UnknownOperation(*operation_id));
            }
            let flow = operations
                .get(operation_id.index())
                .ok_or(MachineDataflowError::UnknownOperation(*operation_id))?;
            add_uses(&mut uses, &definitions, flow.inputs().iter().copied());
            if let Some(result) = body
                .operation(*operation_id)
                .ok_or(MachineDataflowError::UnknownOperation(*operation_id))?
                .result()
            {
                require_value(body, result)?;
                if !definitions.insert(result) {
                    return Err(MachineDataflowError::DuplicateValueDefinition(result));
                }
            }
        }
        let (terminator_inputs, successors) = terminator_flow(body, block.terminator())?;
        add_uses(&mut uses, &definitions, terminator_inputs.iter().copied());
        facts.push(BlockFacts {
            uses,
            definitions,
            terminator_inputs,
            successors,
            predecessors: BTreeSet::new(),
            live_in: BTreeSet::new(),
            live_out: BTreeSet::new(),
        });
    }
    let successor_snapshot = facts
        .iter()
        .map(|fact| fact.successors.clone())
        .collect::<Vec<_>>();
    for (predecessor_index, successors) in successor_snapshot.into_iter().enumerate() {
        let predecessor = MachineBlockId::new(predecessor_index);
        for successor in successors {
            facts
                .get_mut(successor.index())
                .ok_or(MachineDataflowError::UnknownBlock(successor))?
                .predecessors
                .insert(predecessor);
        }
    }
    Ok(facts)
}

fn solve_liveness(facts: &mut [BlockFacts]) {
    let mut pending = (0..facts.len())
        .map(MachineBlockId::new)
        .collect::<BTreeSet<_>>();
    while let Some(block) = pending.pop_last() {
        let live_out = facts[block.index()]
            .successors
            .iter()
            .flat_map(|successor| facts[successor.index()].live_in.iter().copied())
            .collect::<BTreeSet<_>>();
        let mut live_in = facts[block.index()].uses.clone();
        live_in.extend(
            live_out
                .iter()
                .filter(|value| !facts[block.index()].definitions.contains(value))
                .copied(),
        );
        if live_in != facts[block.index()].live_in {
            pending.extend(facts[block.index()].predecessors.iter().copied());
        }
        facts[block.index()].live_in = live_in;
        facts[block.index()].live_out = live_out;
    }
}

fn complete_operation_liveness(
    body: &crate::MachineBody,
    facts: &[BlockFacts],
    operations: &mut [MachineOperationDataflow],
) -> Result<(), MachineDataflowError> {
    for (block_id, block) in body.blocks() {
        let fact = facts
            .get(block_id.index())
            .ok_or(MachineDataflowError::UnknownBlock(block_id))?;
        let mut live = fact.live_out.clone();
        live.extend(fact.terminator_inputs.iter().copied());
        for operation_id in block.operations().iter().rev() {
            let operation = body
                .operation(*operation_id)
                .ok_or(MachineDataflowError::UnknownOperation(*operation_id))?;
            let flow = operations
                .get_mut(operation_id.index())
                .ok_or(MachineDataflowError::UnknownOperation(*operation_id))?;
            flow.live_after = boxed(live.clone());
            if let Some(result) = operation.result() {
                live.remove(&result);
            }
            live.extend(flow.inputs.iter().copied());
        }
        for parameter in block.parameters() {
            live.remove(parameter);
        }
        if live != fact.live_in {
            return Err(MachineDataflowError::InconsistentLiveness(block_id));
        }
    }
    Ok(())
}

fn operation_inputs(
    body: &crate::MachineBody,
    operation: &MachineOperationKind,
) -> Result<Box<[MachineValueId]>, MachineDataflowError> {
    let mut inputs = BTreeSet::new();
    match operation {
        MachineOperationKind::Constant(_)
        | MachineOperationKind::ReleaseRegion { .. }
        | MachineOperationKind::SetDropFlag { .. }
        | MachineOperationKind::PackLength
        | MachineOperationKind::PackNext
        | MachineOperationKind::DestroyPack => {}
        MachineOperationKind::Load { source } | MachineOperationKind::AddressOf { source } => {
            add_address_inputs(body, *source, &mut inputs)?;
        }
        MachineOperationKind::Store { destination, value } => {
            add_address_inputs(body, *destination, &mut inputs)?;
            insert_value(body, *value, &mut inputs)?;
        }
        MachineOperationKind::Unary { operand, .. }
        | MachineOperationKind::IntegerConversion { operand }
        | MachineOperationKind::BorrowWeakening { source: operand } => {
            insert_value(body, *operand, &mut inputs)?;
        }
        MachineOperationKind::Binary { left, right, .. } => {
            insert_value(body, *left, &mut inputs)?;
            insert_value(body, *right, &mut inputs)?;
        }
        MachineOperationKind::Comparison(comparison) => {
            insert_value(body, comparison.left(), &mut inputs)?;
            insert_value(body, comparison.right(), &mut inputs)?;
        }
        MachineOperationKind::IndexBorrow(index) => {
            insert_value(body, index.receiver(), &mut inputs)?;
            insert_value(body, index.index(), &mut inputs)?;
        }
        MachineOperationKind::Aggregate(aggregate) => {
            for write in aggregate.writes() {
                if let MachineAggregateWrite::Value { value, .. } = write {
                    insert_value(body, *value, &mut inputs)?;
                }
            }
        }
        MachineOperationKind::InvokeDrop { place, .. } => {
            add_address_inputs(body, *place, &mut inputs)?;
        }
        MachineOperationKind::ReportError { error } => {
            insert_value(body, *error, &mut inputs)?;
        }
        MachineOperationKind::CreateRegion { parent, .. } => {
            insert_value(body, *parent, &mut inputs)?;
        }
        MachineOperationKind::Call(call) => add_call_inputs(body, call, &mut inputs)?,
    }
    Ok(boxed(inputs))
}

fn add_call_inputs(
    body: &crate::MachineBody,
    call: &MachineCall,
    inputs: &mut BTreeSet<MachineValueId>,
) -> Result<(), MachineDataflowError> {
    for argument in call.arguments() {
        insert_value(body, *argument, inputs)?;
    }
    if let MachineCallAllocation::Explicit(address) = call.allocation() {
        add_address_inputs(body, address, inputs)?;
    }
    if let Some(crate::MachineCallPack::Prepared(pack)) = call.pack() {
        add_pack_inputs(body, pack, inputs)?;
    }
    Ok(())
}

fn add_pack_inputs(
    body: &crate::MachineBody,
    pack_id: MachinePackId,
    inputs: &mut BTreeSet<MachineValueId>,
) -> Result<(), MachineDataflowError> {
    let pack = body
        .pack(pack_id)
        .ok_or(MachineDataflowError::UnknownPack(pack_id))?;
    insert_value(body, pack.length(), inputs)?;
    for segment in pack.segments() {
        match segment {
            MachinePackSegment::Value { value, .. } => insert_value(body, *value, inputs)?,
            MachinePackSegment::Spread(spread) => {
                add_address_inputs(body, spread.iterator(), inputs)?;
                insert_value(body, spread.remaining(), inputs)?;
            }
        }
    }
    Ok(())
}

fn add_address_inputs(
    body: &crate::MachineBody,
    address_id: MachineAddressId,
    inputs: &mut BTreeSet<MachineValueId>,
) -> Result<(), MachineDataflowError> {
    let address = body
        .address(address_id)
        .ok_or(MachineDataflowError::UnknownAddress(address_id))?;
    match address.root() {
        MachineAddressRoot::Stack(_) => {}
        MachineAddressRoot::Pointer { value } | MachineAddressRoot::View { value, .. } => {
            insert_value(body, value, inputs)?;
        }
    }
    for step in address.steps() {
        match step {
            MachineAddressStep::OffsetValue(value)
            | MachineAddressStep::Index {
                index: MachineIndex::Value(value),
                ..
            } => insert_value(body, *value, inputs)?,
            MachineAddressStep::Offset(_)
            | MachineAddressStep::Dereference
            | MachineAddressStep::ViewDereference { .. }
            | MachineAddressStep::Index {
                index: MachineIndex::Constant(_),
                ..
            } => {}
        }
    }
    Ok(())
}

fn terminator_flow(
    body: &crate::MachineBody,
    terminator: &MachineTerminator,
) -> Result<(BTreeSet<MachineValueId>, BTreeSet<MachineBlockId>), MachineDataflowError> {
    let mut inputs = BTreeSet::new();
    let mut successors = BTreeSet::new();
    match terminator {
        MachineTerminator::Goto(target) => add_target(body, target, &mut inputs, &mut successors)?,
        MachineTerminator::Branch {
            condition,
            then_target,
            else_target,
        } => {
            insert_value(body, *condition, &mut inputs)?;
            add_target(body, then_target, &mut inputs, &mut successors)?;
            add_target(body, else_target, &mut inputs, &mut successors)?;
        }
        MachineTerminator::BranchDropFlag {
            initialized,
            uninitialized,
            ..
        } => {
            add_target(body, initialized, &mut inputs, &mut successors)?;
            add_target(body, uninitialized, &mut inputs, &mut successors)?;
        }
        MachineTerminator::SwitchValue {
            subject,
            cases,
            fallback,
        } => {
            insert_value(body, *subject, &mut inputs)?;
            for case in cases {
                add_target(body, case.target(), &mut inputs, &mut successors)?;
            }
            add_target(body, fallback, &mut inputs, &mut successors)?;
        }
        MachineTerminator::SwitchTag {
            subject,
            cases,
            fallback,
            ..
        } => {
            add_address_inputs(body, *subject, &mut inputs)?;
            for case in cases {
                add_target(body, case.target(), &mut inputs, &mut successors)?;
            }
            add_target(body, fallback, &mut inputs, &mut successors)?;
        }
        MachineTerminator::Return(value) | MachineTerminator::Exit(value) => {
            if let Some(value) = value {
                insert_value(body, *value, &mut inputs)?;
            }
        }
        MachineTerminator::Trap | MachineTerminator::Unreachable => {}
    }
    Ok((inputs, successors))
}

fn add_target(
    body: &crate::MachineBody,
    target: &MachineBranchTarget,
    inputs: &mut BTreeSet<MachineValueId>,
    successors: &mut BTreeSet<MachineBlockId>,
) -> Result<(), MachineDataflowError> {
    let block = body
        .block(target.block())
        .ok_or(MachineDataflowError::UnknownBlock(target.block()))?;
    if block.parameters().len() != target.arguments().len() {
        return Err(MachineDataflowError::BranchArity(target.block()));
    }
    for (argument, parameter) in target.arguments().iter().zip(block.parameters()) {
        let argument_ty = require_value(body, *argument)?;
        let parameter_ty = require_value(body, *parameter)?;
        if argument_ty != parameter_ty {
            return Err(MachineDataflowError::BranchType(target.block()));
        }
        inputs.insert(*argument);
    }
    successors.insert(target.block());
    Ok(())
}

fn add_uses(
    uses: &mut BTreeSet<MachineValueId>,
    definitions: &BTreeSet<MachineValueId>,
    inputs: impl IntoIterator<Item = MachineValueId>,
) {
    uses.extend(
        inputs
            .into_iter()
            .filter(|input| !definitions.contains(input)),
    );
}

fn insert_value(
    body: &crate::MachineBody,
    value: MachineValueId,
    inputs: &mut BTreeSet<MachineValueId>,
) -> Result<(), MachineDataflowError> {
    require_value(body, value)?;
    inputs.insert(value);
    Ok(())
}

fn require_value(
    body: &crate::MachineBody,
    value: MachineValueId,
) -> Result<nocter_model::TypeId, MachineDataflowError> {
    body.value(value)
        .map(crate::MachineValue::ty)
        .ok_or(MachineDataflowError::UnknownValue(value))
}

fn boxed<T: Ord>(values: BTreeSet<T>) -> Box<[T]> {
    values.into_iter().collect::<Vec<_>>().into_boxed_slice()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineDataflowError {
    UnknownValue(MachineValueId),
    InvalidValueDefinition(MachineValueId),
    DuplicateValueDefinition(MachineValueId),
    UnknownOperation(MachineOperationId),
    DuplicateOperation(MachineOperationId),
    UnownedOperation(MachineOperationId),
    UnknownAddress(MachineAddressId),
    UnknownPack(MachinePackId),
    UnknownBlock(MachineBlockId),
    BranchArity(MachineBlockId),
    BranchType(MachineBlockId),
    InconsistentLiveness(MachineBlockId),
}

impl fmt::Display for MachineDataflowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "machine dataflow construction failed: {self:?}")
    }
}

impl std::error::Error for MachineDataflowError {}
