use super::{Context, Failure};
use crate::identity::MachineId;
use crate::{
    MachineAddress, MachineAddressId, MachineAddressRoot, MachineAddressStep,
    MachineArithmeticCheck, MachineBlock, MachineBlockId, MachineBranchTarget, MachineCall,
    MachineCallAllocation, MachineCallTarget, MachineConstant, MachineDropFlag, MachineDropFlagId,
    MachineFunction, MachineFunctionExecution, MachineFunctionId, MachineFunctionKind,
    MachineLayoutKind, MachineLinkageId, MachineOperation, MachineOperationId,
    MachineOperationKind, MachineStackId, MachineStackObject, MachineStackPurpose,
    MachineSwitchCase, MachineSwitchValue, MachineTerminator, MachineUnaryOperation, MachineValue,
    MachineValueDefinition, MachineValueId, MachineValueRepresentation,
};
use nocter_mir::value_access as mir;
use nocter_model::{AccessInstanceId, MirLocalId, ValueInputOperation};
use nocter_runtime_contract::RuntimeDataId;
use nocter_runtime_contract::RuntimeType;
use std::collections::{BTreeMap, BTreeSet};
mod arrays;
mod callable_choices;
mod calls;
mod destruction;
mod outcomes;
mod process;
pub(super) use destruction::build as cleanup;
pub(super) use process::{build as process, valid_result as valid_process_result};
mod remainder;
mod repacking;
mod repetition;
mod transport;
mod traversal;
pub(super) use transport::build as transport;

struct Block {
    parameters: Vec<MachineValueId>,
    operations: Vec<MachineOperationId>,
    terminator: Option<MachineTerminator>,
}
struct Builder<'a> {
    context: &'a Context<'a>,
    owner: MachineFunctionId,
    source: Option<&'a mir::Body>,
    stack: Vec<MachineStackObject>,
    slots: BTreeMap<MirLocalId, MachineStackId>,
    parameters: Vec<MachineStackId>,
    flags: Vec<MachineDropFlag>,
    flag_ids: BTreeMap<(usize, u64), MachineDropFlagId>,
    addresses: Vec<MachineAddress>,
    places: BTreeMap<mir::Place, MachineAddressId>,
    values: Vec<MachineValue>,
    operations: Vec<MachineOperation>,
    blocks: Vec<Block>,
    current: MachineBlockId,
}
impl<'a> Builder<'a> {
    fn new(context: &'a Context<'a>, owner: MachineFunctionId) -> Self {
        Self {
            context,
            owner,
            source: None,
            stack: Vec::new(),
            slots: BTreeMap::new(),
            parameters: Vec::new(),
            flags: Vec::new(),
            flag_ids: BTreeMap::new(),
            addresses: Vec::new(),
            places: BTreeMap::new(),
            values: Vec::new(),
            operations: Vec::new(),
            blocks: vec![Block {
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: None,
            }],
            current: MachineBlockId::new(0),
        }
    }
    fn invalid() -> Failure {
        Failure::InvalidRoot
    }
    fn block(&mut self) -> MachineBlockId {
        let id = MachineBlockId::new(self.blocks.len());
        self.blocks.push(Block {
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: None,
        });
        id
    }
    fn terminate(&mut self, terminator: MachineTerminator) -> Result<(), Failure> {
        if self.blocks[self.current.index()]
            .terminator
            .replace(terminator)
            .is_some()
        {
            return Err(Self::invalid());
        }
        Ok(())
    }
    fn go(&mut self, next: MachineBlockId) -> Result<(), Failure> {
        self.terminate(MachineTerminator::Goto(MachineBranchTarget::new(next, [])))
    }
    fn select_integer_cases(
        &mut self,
        subject: MachineValueId,
        tags: impl IntoIterator<Item = i128>,
        mut action: impl FnMut(&mut Self, usize) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        let mut blocks = Vec::new();
        let mut cases = Vec::new();
        for tag in tags {
            let block = self.block();
            blocks.push(block);
            cases.push(MachineSwitchCase::new(
                MachineSwitchValue::Integer(tag),
                MachineBranchTarget::new(block, []),
            ));
        }
        let fallback = self.block();
        let done = self.block();
        self.terminate(MachineTerminator::SwitchValue {
            subject,
            cases: cases.into(),
            fallback: MachineBranchTarget::new(fallback, []),
        })?;
        for (index, block) in blocks.into_iter().enumerate() {
            self.current = block;
            action(self, index)?;
            self.go(done)?;
        }
        self.current = fallback;
        self.terminate(MachineTerminator::Unreachable)?;
        self.current = done;
        Ok(())
    }
    fn emit(&mut self, kind: MachineOperationKind) {
        self.append(kind, None);
    }
    fn append(&mut self, kind: MachineOperationKind, result: Option<MachineValueId>) {
        let id = MachineOperationId::new(self.operations.len());
        self.operations.push(MachineOperation::new(kind, result));
        self.blocks[self.current.index()].operations.push(id);
    }
    fn value(
        &mut self,
        data: RuntimeDataId,
        address: bool,
        kind: MachineOperationKind,
    ) -> Result<MachineValueId, Failure> {
        let operation = MachineOperationId::new(self.operations.len());
        let definition = MachineValueDefinition::Operation(operation);
        let value = if address {
            MachineValue::address(data, self.context.layouts.target(), definition)
        } else {
            self.stored_value(data, definition)?
        };
        let id = MachineValueId::new(self.values.len());
        self.values.push(value);
        self.append(kind, Some(id));
        Ok(id)
    }
    fn stored_value(
        &self,
        data: RuntimeDataId,
        definition: MachineValueDefinition,
    ) -> Result<MachineValue, Failure> {
        let layout = self.context.layouts.get(data).ok_or_else(Self::invalid)?;
        Ok(MachineValue::new(
            data,
            MachineValueRepresentation::Stored {
                size: layout.size(),
                alignment: layout.alignment(),
                class: self.context.layouts.class(data).ok_or_else(Self::invalid)?,
            },
            definition,
        ))
    }
    fn parameter(
        &mut self,
        block: MachineBlockId,
        data: RuntimeDataId,
    ) -> Result<MachineValueId, Failure> {
        let position = self.blocks[block.index()].parameters.len();
        let value = self.stored_value(
            data,
            MachineValueDefinition::BlockParameter { block, position },
        )?;
        let id = MachineValueId::new(self.values.len());
        self.values.push(value);
        self.blocks[block.index()].parameters.push(id);
        Ok(id)
    }
    fn address(&mut self, address: MachineAddress) -> MachineAddressId {
        let id = MachineAddressId::new(self.addresses.len());
        self.addresses.push(address);
        id
    }
    fn offset_address(
        &mut self,
        base: MachineAddressId,
        data: RuntimeDataId,
        offset: u64,
    ) -> Result<MachineAddressId, Failure> {
        let pointer = self.value(
            self.addresses[base.index()].ty(),
            true,
            MachineOperationKind::AddressOf { source: base },
        )?;
        let layout = self.context.layouts.get(data).ok_or_else(Self::invalid)?;
        Ok(self.address(MachineAddress::new(
            data,
            layout.size(),
            layout.alignment(),
            MachineAddressRoot::Pointer { value: pointer },
            [MachineAddressStep::Offset(offset)],
        )))
    }
    fn slot_address(&mut self, slot: MachineStackId) -> MachineAddressId {
        let object = self.stack[slot.index()];
        self.address(MachineAddress::new(
            object.ty(),
            object.size(),
            object.alignment(),
            MachineAddressRoot::Stack(slot),
            [],
        ))
    }
    fn place(&mut self, place: &mir::Place) -> Result<MachineAddressId, Failure> {
        // A saved scalar is loaded at each use. Caching a projected address would retain an
        // SSA definition from another branch or an earlier execution of this recipe.
        let cacheable = !place
            .projections
            .iter()
            .any(|projection| matches!(projection, mir::Projection::Index(_)));
        if cacheable && let Some(&id) = self.places.get(place) {
            return Ok(id);
        }
        let source = self.source.ok_or_else(Self::invalid)?;
        let slot = source.slots().get(place.slot).ok_or_else(Self::invalid)?;
        if !slot.representations.contains(&place.root) {
            return Err(Self::invalid());
        }
        let stack = *self.slots.get(&place.slot).ok_or_else(Self::invalid)?;
        let mut steps = Vec::new();
        if slot.storage == mir::Storage::Address {
            steps.push(MachineAddressStep::Dereference);
        }
        let mut data = place.root;
        let mut offset = 0u64;
        for projection in &place.projections {
            match projection {
                mir::Projection::Field(field) => {
                    let field = self
                        .context
                        .layouts
                        .value_field(data, *field)
                        .ok_or_else(Self::invalid)?;
                    offset = offset
                        .checked_add(field.offset())
                        .ok_or_else(Self::invalid)?;
                    data = field.ty();
                }
                mir::Projection::Index(snapshot) => {
                    let Some(MachineLayoutKind::FixedArray {
                        element,
                        length,
                        stride,
                    }) = self
                        .context
                        .layouts
                        .get(data)
                        .map(crate::MachineLayout::kind)
                    else {
                        return Err(Self::invalid());
                    };
                    let (element, length, stride) = (*element, *length, *stride);
                    if offset != 0 {
                        steps.push(MachineAddressStep::Offset(offset));
                        offset = 0;
                    }
                    let index = self.retained_index(*snapshot, length)?;
                    steps.push(MachineAddressStep::Index {
                        index: crate::MachineIndex::Value(index),
                        stride,
                        bound: crate::MachineIndexBound::Fixed(length),
                        check: crate::MachineIndexCheck::ProvenInBounds,
                    });
                    data = element;
                }
                mir::Projection::Environment => {
                    let Some(RuntimeType::StaticCallable {
                        environment: Some(environment),
                    }) = self.context.program.runtime().types().get(data)
                    else {
                        return Err(Self::invalid());
                    };
                    data = *environment;
                }
            }
        }
        if !self
            .context
            .program
            .runtime()
            .has_same_value_representation(data, place.data)
        {
            return Err(Self::invalid());
        }
        if offset != 0 {
            steps.push(MachineAddressStep::Offset(offset));
        }
        let layout = self.context.layouts.get(data).ok_or_else(Self::invalid)?;
        let address = self.address(MachineAddress::new(
            data,
            layout.size(),
            layout.alignment(),
            MachineAddressRoot::Stack(stack),
            steps,
        ));
        if cacheable {
            self.places.insert(place.clone(), address);
        }
        Ok(address)
    }
    fn load(&mut self, place: &mir::Place) -> Result<MachineValueId, Failure> {
        let source = self.place(place)?;
        self.value(place.data, false, MachineOperationKind::Load { source })
    }
    fn store(&mut self, place: &mir::Place, value: MachineValueId) -> Result<(), Failure> {
        let destination = self.place(place)?;
        self.emit(MachineOperationKind::Store { destination, value });
        Ok(())
    }
    fn copy(&mut self, source: &mir::Place, destination: &mir::Place) -> Result<(), Failure> {
        let left = self
            .context
            .layouts
            .get(source.data)
            .ok_or_else(Self::invalid)?;
        let right = self
            .context
            .layouts
            .get(destination.data)
            .ok_or_else(Self::invalid)?;
        if left.size() != right.size() || left.alignment() != right.alignment() {
            return Err(Self::invalid());
        }
        if left.size() == 0 {
            return Ok(());
        }
        let value = self.load(source)?;
        self.store(destination, value)
    }
    fn effect(&mut self, effect: &mir::Effect) -> Result<(), Failure> {
        match effect {
            mir::Effect::SetFutureSelector { destination, tag } => {
                self.constant(destination, &mir::Constant::Integer(i128::from(*tag)))?;
            }
            mir::Effect::SelectedFutureMove { selector, cases } => {
                let subject = self.load(selector)?;
                let tags = cases.iter().map(|case| i128::from(case.tag));
                self.select_integer_cases(subject, tags, |builder, index| {
                    let case = cases.get(index).ok_or_else(Self::invalid)?;
                    for field in &case.fields {
                        match field.operation {
                            nocter_model::ValueInputOperation::Take => {
                                builder.copy(&field.source, &field.destination)?;
                            }
                            nocter_model::ValueInputOperation::Read
                            | nocter_model::ValueInputOperation::Edit => {
                                builder.bind_address(field.destination.slot, &field.source)?;
                            }
                        }
                    }
                    Ok(())
                })?;
            }
            mir::Effect::SelectedFutureCleanup { selector, cases } => {
                let subject = self.load(selector)?;
                let tags = cases.iter().map(|case| i128::from(case.tag));
                self.select_integer_cases(subject, tags, |builder, index| {
                    let case = cases.get(index).ok_or_else(Self::invalid)?;
                    for input in &case.fields {
                        builder.dispose_value(&input.source, input.disposal)?;
                    }
                    Ok(())
                })?;
            }
            mir::Effect::VariantTag {
                destination,
                primary,
            } => self.variant_tag(destination, *primary)?,

            mir::Effect::Initialize(_)
            | mir::Effect::End(_)
            | mir::Effect::ConsolidateArrayPrefix { .. } => {}
            mir::Effect::Transfer {
                source,
                destination,
            }
            | mir::Effect::Reinterpret {
                source,
                destination,
            } => self.copy(source, destination)?,
            mir::Effect::Repack {
                source,
                destination,
                preserved,
                missing,
                ..
            } => {
                self.repack(source, destination, preserved, missing)?;
            }
            mir::Effect::Transport {
                source,
                destination,
            } => {
                let from = self.place(source)?;
                let to = self.place(destination)?;
                self.transport_value((source.data, destination.data), from, to)?;
            }
            mir::Effect::BindAddress {
                destination,
                source,
            } => {
                self.bind_address(*destination, source)?;
            }
            mir::Effect::DisposeArrayElements { source, range } => {
                self.dispose_array_elements(source, *range)?;
            }
            mir::Effect::DisposeArrayRemainder { source, missing } => {
                self.dispose_array_remainder(source, missing)?;
            }
            mir::Effect::Dispose { source, disposal } => {
                self.dispose_value(source, *disposal)?;
            }
        }
        Ok(())
    }
    fn dispose_value(
        &mut self,
        source: &mir::Place,
        disposal: mir::Disposal,
    ) -> Result<(), Failure> {
        if disposal != mir::Disposal::Trivial
            && let Some(&target) = self.context.cleanup.get(&source.data)
        {
            let place = self.place(source)?;
            let pointer = self.value(
                source.data,
                true,
                MachineOperationKind::AddressOf { source: place },
            )?;
            self.emit(MachineOperationKind::Call(MachineCall::new(
                MachineCallTarget::Direct(target),
                [pointer],
                MachineCallAllocation::Inherit,
                None,
            )));
        }
        Ok(())
    }
    fn bind_address(
        &mut self,
        destination: nocter_model::MirLocalId,
        source: &mir::Place,
    ) -> Result<(), Failure> {
        let address = self.place(source)?;
        let value = self.value(
            source.data,
            true,
            MachineOperationKind::AddressOf { source: address },
        )?;
        let stack = *self.slots.get(&destination).ok_or_else(Self::invalid)?;
        let destination = self.slot_address(stack);
        self.emit(MachineOperationKind::Store { destination, value });
        Ok(())
    }
    fn effects(&mut self, effects: &[mir::Effect]) -> Result<(), Failure> {
        for effect in effects {
            self.effect(effect)?;
        }
        Ok(())
    }
    fn writes(&mut self, values: &[mir::BitValue]) -> Result<(), Failure> {
        for value in values {
            let flag = *self
                .flag_ids
                .get(&(value.word, value.mask))
                .ok_or_else(Self::invalid)?;
            self.emit(MachineOperationKind::SetDropFlag {
                flag,
                initialized: value.value,
            });
        }
        Ok(())
    }
    fn frozen(
        &mut self,
        destination: &mir::Place,
        value: &nocter_model::FrozenData,
    ) -> Result<(), Failure> {
        let layout = self
            .context
            .layouts
            .get(destination.data)
            .ok_or_else(Self::invalid)?;
        if layout.size() == 0 {
            return Ok(());
        }
        let data = self
            .context
            .data
            .get(destination.data, value)
            .ok_or_else(Self::invalid)?;
        let source = self.address(MachineAddress::new(
            destination.data,
            layout.size(),
            layout.alignment(),
            MachineAddressRoot::Data(data),
            [],
        ));
        let value = self.value(
            destination.data,
            false,
            MachineOperationKind::Load { source },
        )?;
        self.store(destination, value)?;
        Ok(())
    }
    fn prepare_flags(&mut self, source: &mir::Body) -> Result<(), Failure> {
        let mut flags = BTreeSet::new();
        for alternative in source.actions().values().flat_map(|a| &a.alternatives) {
            for bit in alternative
                .tests
                .iter()
                .chain(&alternative.before_writes)
                .chain(&alternative.after_writes)
            {
                flags.insert((bit.word, bit.mask));
            }
        }
        for (word, mask) in flags {
            let id = MachineDropFlagId::new(self.flags.len());
            let initial = *source.flags().get(word).ok_or(Self::invalid())? & mask != 0;
            self.flags.push(MachineDropFlag::new(initial));
            self.flag_ids.insert((word, mask), id);
        }
        Ok(())
    }
    fn bind_static(
        &mut self,
        slot: MirLocalId,
        id: nocter_model::ValueStatic,
    ) -> Result<(), Failure> {
        let global = self
            .context
            .program
            .statics()
            .get(&id)
            .ok_or_else(Self::invalid)?;
        let layout = self
            .context
            .layouts
            .get(global.data)
            .ok_or_else(Self::invalid)?;
        let data = self.context.data.global(id).ok_or_else(Self::invalid)?;
        let source = self.address(MachineAddress::new(
            global.data,
            layout.size(),
            layout.alignment(),
            MachineAddressRoot::Data(data),
            [],
        ));
        let value = self.value(
            global.data,
            true,
            MachineOperationKind::AddressOf { source },
        )?;
        let destination = self.slot_address(*self.slots.get(&slot).ok_or_else(Self::invalid)?);
        self.emit(MachineOperationKind::Store { destination, value });
        Ok(())
    }
    fn constant(&mut self, destination: &mir::Place, value: &mir::Constant) -> Result<(), Failure> {
        let value = match value {
            mir::Constant::Integer(value) => MachineConstant::Integer(*value),
            mir::Constant::Boolean(value) => MachineConstant::Bool(*value),
            mir::Constant::Zero => return Ok(()),
        };
        let value = self.value(
            destination.data,
            false,
            MachineOperationKind::Constant(value),
        )?;
        self.store(destination, value)?;
        Ok(())
    }
    fn operation(&mut self, operation: &mir::Operation) -> Result<(), Failure> {
        match operation {
            mir::Operation::BoundsTrap => return Err(Self::invalid()),
            mir::Operation::RequireVariant { source, primary } => {
                self.require_variant(source, *primary)?;
            }
            mir::Operation::PrimaryCondition {
                source,
                destination,
            } => {
                let address = self.place(source)?;
                let value = self.primary_condition(address, source.data)?;
                let destination = self.place(destination)?;
                self.emit(MachineOperationKind::Store { destination, value });
            }
            mir::Operation::Frozen { destination, value } => self.frozen(destination, value)?,
            mir::Operation::Constant { destination, value } => self.constant(destination, value)?,
            mir::Operation::Copy {
                source,
                destination,
            } => self.copy(source, destination)?,
            mir::Operation::Index {
                base,
                index,
                destination,
                copy,
                bounds,
            } => {
                self.index(base, index, destination, *copy, *bounds)?;
            }
            mir::Operation::Repeat {
                source,
                destination,
            } => self.repeat(source, destination)?,
            mir::Operation::Storage(effect) => self.effect(effect)?,
            mir::Operation::Unary {
                operation,
                operand,
                destination,
                check,
            } => {
                let operand = self.load(operand)?;
                let value = self.value(
                    destination.data,
                    false,
                    MachineOperationKind::Unary {
                        operation: (*operation).into(),
                        operand,
                        check: (*check).into(),
                    },
                )?;
                self.store(destination, value)?;
            }
            mir::Operation::Binary {
                operation,
                left,
                right,
                destination,
                invert,
                check,
            } => {
                let left = self.load(left)?;
                let right = self.load(right)?;
                let value = self.value(
                    destination.data,
                    false,
                    MachineOperationKind::Binary {
                        operation: (*operation).into(),
                        left,
                        right,
                        check: (*check).into(),
                    },
                )?;
                let value = if *invert {
                    self.value(
                        destination.data,
                        false,
                        MachineOperationKind::Unary {
                            operation: MachineUnaryOperation::LogicalNot,
                            operand: value,
                            check: MachineArithmeticCheck::NotRequired,
                        },
                    )?
                } else {
                    value
                };
                self.store(destination, value)?;
            }
            mir::Operation::Call(call) => self.call(call)?,
            mir::Operation::SelectedCall(call) => self.selected_call(call)?,
            mir::Operation::SelectedDrive(drive) => self.selected_drive(drive)?,
        }
        Ok(())
    }
    fn action(&mut self, action: &mir::Action) -> Result<(), Failure> {
        if matches!(action.operations.as_ref(), [mir::Operation::BoundsTrap]) {
            return self.terminate(MachineTerminator::Trap(crate::MachineTrap::Bounds));
        }
        if action.alternatives.len() == 1 && action.alternatives[0].tests.is_empty() {
            return self.alternative(&action.alternatives[0], action);
        }
        let join = self.block();
        for alternative in &action.alternatives {
            let failed = self.block();
            for bit in &alternative.tests {
                let passed = self.block();
                let flag = *self
                    .flag_ids
                    .get(&(bit.word, bit.mask))
                    .ok_or_else(Self::invalid)?;
                let (initialized, uninitialized) = if bit.value {
                    (passed, failed)
                } else {
                    (failed, passed)
                };
                self.terminate(MachineTerminator::BranchDropFlag {
                    flag,
                    initialized: MachineBranchTarget::new(initialized, []),
                    uninitialized: MachineBranchTarget::new(uninitialized, []),
                })?;
                self.current = passed;
            }
            self.alternative(alternative, action)?;
            self.go(join)?;
            self.current = failed;
        }
        self.terminate(MachineTerminator::Trap(crate::MachineTrap::General))?;
        self.current = join;
        Ok(())
    }
    fn alternative(
        &mut self,
        alternative: &mir::Alternative,
        action: &mir::Action,
    ) -> Result<(), Failure> {
        self.effects(&alternative.before)?;
        self.writes(&alternative.before_writes)?;
        self.effects(&action.call_bindings)?;
        for operation in &action.operations {
            self.operation(operation)?;
        }
        self.effects(&alternative.after)?;
        self.writes(&alternative.after_writes)
    }
    fn finish(
        self,
        kind: MachineFunctionKind,
        entry: MachineBlockId,
    ) -> Result<MachineFunction, Failure> {
        let blocks = self
            .blocks
            .into_iter()
            .map(|block| {
                Ok(MachineBlock::new(
                    block.parameters,
                    block.operations,
                    block.terminator.ok_or(Failure::InvalidRoot)?,
                ))
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        let mut execution = MachineFunctionExecution::Immediate;
        let body = crate::MachineBody::freeze(
            crate::program::MachineBodyDraft {
                parameters: self.parameters,
                stack: self.stack,
                drop_flags: self.flags,
                addresses: self.addresses,
                values: self.values,
                operations: self.operations,
                packs: Vec::new(),
                blocks,
                entry,
            },
            &mut execution,
        )
        .map_err(Failure::Optimization)?;
        MachineFunction::new(
            MachineLinkageId::new(self.owner.index()),
            kind,
            execution,
            body,
        )
        .map_err(Failure::Dataflow)
    }
}

pub(super) fn source<'a>(
    context: &'a Context<'a>,
    instance: AccessInstanceId,
    source: &'a mir::Body,
) -> Result<MachineFunction, Failure> {
    let owner = *context
        .functions
        .get(&instance)
        .ok_or(Failure::InvalidBody(instance))?;
    let mut builder = Builder::new(context, owner);
    builder.source = Some(source);
    let positions = source
        .inputs()
        .iter()
        .enumerate()
        .map(|(index, &id)| (id, index))
        .collect::<BTreeMap<_, _>>();
    for (id, slot) in source.slots().iter() {
        let data = *slot
            .representations
            .first()
            .ok_or(Failure::InvalidBody(instance))?;
        let purpose = positions
            .get(&id)
            .map_or(MachineStackPurpose::Temporary, |&position| {
                MachineStackPurpose::Parameter { position }
            });
        let object = if slot.storage == mir::Storage::Address {
            MachineStackObject::address(data, context.layouts.target(), purpose)
        } else {
            let mut size = 0;
            let mut alignment = 1;
            for &data in &slot.representations {
                let layout = context.layouts.get(data).ok_or(Failure::InvalidRoot)?;
                size = size.max(layout.size());
                alignment = alignment.max(layout.alignment());
            }
            MachineStackObject::new(data, size, alignment, purpose)
        };
        let stack = MachineStackId::new(builder.stack.len());
        builder.stack.push(object);
        builder.slots.insert(id, stack);
    }
    builder.parameters = source
        .inputs()
        .iter()
        .map(|id| builder.slots.get(id).copied().ok_or(Failure::InvalidRoot))
        .collect::<Result<_, _>>()?;
    builder.prepare_flags(source)?;
    let mut points = BTreeMap::new();
    for &point in source.terminators().keys() {
        let block = if points.is_empty() {
            builder.current
        } else {
            builder.block()
        };
        points.insert(point, block);
    }
    for (&point, terminator) in source.terminators() {
        builder.current = points[&point];
        if point == source.entry() {
            for &(slot, id) in source.statics() {
                builder.bind_static(slot, id)?;
            }
        }
        for (_, action) in source.actions().range(
            mir::Location { point, action: 0 }..=mir::Location {
                point,
                action: usize::MAX,
            },
        ) {
            builder.action(action)?;
        }
        let terminator = match terminator {
            mir::Terminator::Jump(next) => {
                MachineTerminator::Goto(MachineBranchTarget::new(points[next], []))
            }
            mir::Terminator::Branch { condition, yes, no } => MachineTerminator::Branch {
                condition: builder.load(condition)?,
                then_target: MachineBranchTarget::new(points[yes], []),
                else_target: MachineBranchTarget::new(points[no], []),
            },
            mir::Terminator::Return(place) => MachineTerminator::Return(Some(builder.load(place)?)),
            mir::Terminator::Unreachable => MachineTerminator::Unreachable,
        };
        if builder.blocks[builder.current.index()].terminator.is_some() {
            if !matches!(terminator, MachineTerminator::Unreachable) {
                return Err(Failure::InvalidRoot);
            }
            continue;
        }
        builder.terminate(terminator)?;
    }
    builder.finish(
        MachineFunctionKind::Callable(context.abis[&instance].clone()),
        points[&source.entry()],
    )
}
