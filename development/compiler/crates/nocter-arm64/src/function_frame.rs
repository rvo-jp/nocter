use std::fmt;

use nocter_machine::{
    MachineAllocationRequirement, MachineCallTarget, MachineFunctionId, MachineFunctionKind,
    MachineOperationKind, MachinePackId, MachineResultAbi, MachineResultLocation, MachineStackId,
    MachineValueId,
};

use crate::{
    Arm64FrameLayout, Arm64FrameLayoutBuilder, Arm64FrameLayoutError, Arm64FrameObjectId,
    Arm64NocterAbi, Arm64PackDescriptorLayout, Arm64PackLayoutError, Arm64PackStateLayout,
    Arm64SpillSlotId, Arm64ValuePlan, Arm64ValueStorage,
};

/// Frame storage for the compiler-propagated allocation context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64AllocationContextFrame {
    None,
    /// The root owns the two-word context whose address is passed in `x9`.
    ProgramRoot(Arm64FrameObjectId),
    /// A non-root saves its incoming `x9` pointer across ordinary calls.
    IncomingPointer(Arm64FrameObjectId),
}

/// Descriptor and callback-state objects owned by one literal call site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64PackFrame {
    descriptor: Arm64FrameObjectId,
    state: Arm64FrameObjectId,
    state_layout: Arm64PackStateLayout,
}

impl Arm64PackFrame {
    #[must_use]
    pub const fn descriptor(&self) -> Arm64FrameObjectId {
        self.descriptor
    }

    #[must_use]
    pub const fn state(&self) -> Arm64FrameObjectId {
        self.state
    }

    #[must_use]
    pub const fn state_layout(&self) -> &Arm64PackStateLayout {
        &self.state_layout
    }
}

/// One completely placed fixed frame and every body-local identity projection into it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64FunctionFrame {
    layout: Arm64FrameLayout,
    stack_objects: Box<[Arm64FrameObjectId]>,
    drop_flags: Box<[Arm64FrameObjectId]>,
    memory_values: Box<[Option<Arm64FrameObjectId>]>,
    direct_aggregate_staging: Option<Arm64FrameObjectId>,
    memory_edge_staging: Option<Arm64FrameObjectId>,
    packs: Box<[Arm64PackFrame]>,
    spills: Box<[Arm64FrameObjectId]>,
    indirect_result_pointer: Option<Arm64FrameObjectId>,
    pack_input_pointer: Option<Arm64FrameObjectId>,
    allocation_context: Arm64AllocationContextFrame,
    error_report_buffer: Option<Arm64FrameObjectId>,
}

impl Arm64FunctionFrame {
    /// Places every fixed object for one machine function through the common frame planner.
    ///
    /// # Errors
    ///
    /// Rejects a foreign value plan, missing call ABI or allocation facts, malformed dense
    /// identities, pack-layout failures, and every underlying frame-layout failure.
    pub fn build(
        program: &nocter_machine::MachineProgram,
        function_id: MachineFunctionId,
        values: &Arm64ValuePlan,
    ) -> Result<Self, Arm64FunctionFrameError> {
        let function = program
            .function(function_id)
            .ok_or(Arm64FunctionFrameError::UnknownFunction(function_id))?;
        if values.owner() != function.linkage() {
            return Err(Arm64FunctionFrameError::ForeignValuePlan {
                expected: function.linkage(),
                actual: values.owner(),
            });
        }
        let body = function.body();
        let mut builder = Arm64FrameLayoutBuilder::new();
        reserve_outgoing_area(program, body, &mut builder)?;
        let placed = place_body_objects(body, values, &mut builder)?;
        let hidden = place_hidden_objects(program, function_id, function.kind(), &mut builder)?;
        for register in values.registers().preserved_registers() {
            builder.preserve(*register)?;
        }
        let layout = builder.finish()?;
        Ok(Self {
            layout,
            stack_objects: placed.stack_objects,
            drop_flags: placed.drop_flags,
            memory_values: placed.memory_values,
            direct_aggregate_staging: placed.direct_aggregate_staging,
            memory_edge_staging: placed.memory_edge_staging,
            packs: placed.packs,
            spills: placed.spills,
            indirect_result_pointer: hidden.indirect_result_pointer,
            pack_input_pointer: hidden.pack_input_pointer,
            allocation_context: hidden.allocation_context,
            error_report_buffer: hidden.error_report_buffer,
        })
    }

    #[must_use]
    pub const fn layout(&self) -> &Arm64FrameLayout {
        &self.layout
    }

    #[must_use]
    pub fn stack_object(&self, id: MachineStackId) -> Option<Arm64FrameObjectId> {
        self.stack_objects.get(id.index()).copied()
    }

    #[must_use]
    pub fn drop_flag(&self, id: nocter_machine::MachineDropFlagId) -> Option<Arm64FrameObjectId> {
        self.drop_flags.get(id.index()).copied()
    }

    #[must_use]
    pub fn memory_value(&self, id: MachineValueId) -> Option<Arm64FrameObjectId> {
        self.memory_values.get(id.index()).copied().flatten()
    }

    /// Shared construction storage for aggregates whose completed value lives in registers.
    #[must_use]
    pub const fn direct_aggregate_staging(&self) -> Option<Arm64FrameObjectId> {
        self.direct_aggregate_staging
    }

    /// One value-sized temporary used only to break cycles in block-edge memory assignments.
    #[must_use]
    pub const fn memory_edge_staging(&self) -> Option<Arm64FrameObjectId> {
        self.memory_edge_staging
    }

    #[must_use]
    pub fn pack(&self, id: MachinePackId) -> Option<&Arm64PackFrame> {
        self.packs.get(id.index())
    }

    #[must_use]
    pub fn spill(&self, id: Arm64SpillSlotId) -> Option<Arm64FrameObjectId> {
        self.spills.get(id.index()).copied()
    }

    #[must_use]
    pub const fn indirect_result_pointer(&self) -> Option<Arm64FrameObjectId> {
        self.indirect_result_pointer
    }

    #[must_use]
    pub const fn pack_input_pointer(&self) -> Option<Arm64FrameObjectId> {
        self.pack_input_pointer
    }

    #[must_use]
    pub const fn allocation_context(&self) -> Arm64AllocationContextFrame {
        self.allocation_context
    }

    #[must_use]
    pub const fn error_report_buffer(&self) -> Option<Arm64FrameObjectId> {
        self.error_report_buffer
    }
}

struct PlacedBodyObjects {
    stack_objects: Box<[Arm64FrameObjectId]>,
    drop_flags: Box<[Arm64FrameObjectId]>,
    memory_values: Box<[Option<Arm64FrameObjectId>]>,
    direct_aggregate_staging: Option<Arm64FrameObjectId>,
    memory_edge_staging: Option<Arm64FrameObjectId>,
    packs: Box<[Arm64PackFrame]>,
    spills: Box<[Arm64FrameObjectId]>,
}

struct HiddenObjects {
    indirect_result_pointer: Option<Arm64FrameObjectId>,
    pack_input_pointer: Option<Arm64FrameObjectId>,
    allocation_context: Arm64AllocationContextFrame,
    error_report_buffer: Option<Arm64FrameObjectId>,
}

fn reserve_outgoing_area(
    program: &nocter_machine::MachineProgram,
    body: &nocter_machine::MachineBody,
    builder: &mut Arm64FrameLayoutBuilder,
) -> Result<(), Arm64FunctionFrameError> {
    for (_, operation) in body.operations() {
        let stack_size = match operation.kind() {
            MachineOperationKind::Call(call) => call_stack_size(program, call.target())?,
            MachineOperationKind::InvokeDrop { target, .. } => direct_stack_size(program, *target)?,
            _ => 0,
        };
        builder.require_outgoing_argument_size(stack_size)?;
    }
    Ok(())
}

fn place_body_objects(
    body: &nocter_machine::MachineBody,
    values: &Arm64ValuePlan,
    builder: &mut Arm64FrameLayoutBuilder,
) -> Result<PlacedBodyObjects, Arm64FunctionFrameError> {
    let mut stack_objects = Vec::with_capacity(body.stack_objects().len());
    for (stack_id, object) in body.stack_objects() {
        if stack_id.index() != stack_objects.len() {
            return Err(Arm64FunctionFrameError::NonDenseStack(stack_id));
        }
        let (size, alignment) = match object.purpose() {
            nocter_machine::MachineStackPurpose::Region => (
                crate::region_layout::Arm64RegionLayout::SIZE,
                crate::region_layout::Arm64RegionLayout::ALIGNMENT,
            ),
            _ => (object.size(), object.alignment()),
        };
        stack_objects.push(builder.add_object(size, alignment)?);
    }
    let mut drop_flags = Vec::with_capacity(body.drop_flags().len());
    for (flag_id, _) in body.drop_flags() {
        if flag_id.index() != drop_flags.len() {
            return Err(Arm64FunctionFrameError::NonDenseDropFlag(flag_id));
        }
        drop_flags.push(builder.add_object(1, 1)?);
    }
    let memory_values = place_memory_values(body, values, builder)?;
    let direct_aggregate_staging = place_direct_aggregate_staging(body, values, builder)?;
    let memory_edge_staging = place_memory_edge_staging(body, values, builder)?;
    let packs = place_packs(body, builder)?;
    let mut spills = Vec::with_capacity(values.registers().spill_count());
    for _ in 0..values.registers().spill_count() {
        spills.push(builder.add_object(Arm64NocterAbi::WORD_SIZE, Arm64NocterAbi::WORD_SIZE)?);
    }
    Ok(PlacedBodyObjects {
        stack_objects: stack_objects.into_boxed_slice(),
        drop_flags: drop_flags.into_boxed_slice(),
        memory_values,
        direct_aggregate_staging,
        memory_edge_staging,
        packs,
        spills: spills.into_boxed_slice(),
    })
}

fn place_memory_edge_staging(
    body: &nocter_machine::MachineBody,
    values: &Arm64ValuePlan,
    builder: &mut Arm64FrameLayoutBuilder,
) -> Result<Option<Arm64FrameObjectId>, Arm64FunctionFrameError> {
    let mut requirement: Option<(u64, u64)> = None;
    for parameter in body
        .blocks()
        .flat_map(|(_, block)| block.parameters().iter().copied())
    {
        match values
            .value(parameter)
            .ok_or(Arm64FunctionFrameError::MissingValue(parameter))?
        {
            Arm64ValueStorage::Memory { size, alignment } => {
                let (required_size, required_alignment) = requirement.unwrap_or((0, 1));
                requirement = Some((required_size.max(*size), required_alignment.max(*alignment)));
            }
            Arm64ValueStorage::Omitted | Arm64ValueStorage::Direct(_) => {}
        }
    }
    requirement
        .map(|(size, alignment)| builder.add_object(size, alignment))
        .transpose()
        .map_err(Arm64FunctionFrameError::from)
}

fn place_direct_aggregate_staging(
    body: &nocter_machine::MachineBody,
    values: &Arm64ValuePlan,
    builder: &mut Arm64FrameLayoutBuilder,
) -> Result<Option<Arm64FrameObjectId>, Arm64FunctionFrameError> {
    let mut requirement: Option<(u64, u64)> = None;
    for (operation_id, operation) in body.operations() {
        let MachineOperationKind::Aggregate(aggregate) = operation.kind() else {
            continue;
        };
        let result = operation
            .result()
            .ok_or(Arm64FunctionFrameError::MissingOperationResult(
                operation_id,
            ))?;
        match values
            .value(result)
            .ok_or(Arm64FunctionFrameError::MissingValue(result))?
        {
            Arm64ValueStorage::Direct(_) => {
                let (size, alignment) = requirement.unwrap_or((0, 1));
                requirement = Some((
                    size.max(aggregate.size()),
                    alignment.max(aggregate.alignment()),
                ));
            }
            Arm64ValueStorage::Omitted | Arm64ValueStorage::Memory { .. } => {}
        }
    }
    requirement
        .map(|(size, alignment)| builder.add_object(size, alignment))
        .transpose()
        .map_err(Arm64FunctionFrameError::from)
}

fn place_memory_values(
    body: &nocter_machine::MachineBody,
    values: &Arm64ValuePlan,
    builder: &mut Arm64FrameLayoutBuilder,
) -> Result<Box<[Option<Arm64FrameObjectId>]>, Arm64FunctionFrameError> {
    let mut memory_values = Vec::with_capacity(body.values().len());
    for (value_id, _) in body.values() {
        if value_id.index() != memory_values.len() {
            return Err(Arm64FunctionFrameError::NonDenseValue(value_id));
        }
        let object = match values
            .value(value_id)
            .ok_or(Arm64FunctionFrameError::MissingValue(value_id))?
        {
            Arm64ValueStorage::Memory { size, alignment } => {
                Some(builder.add_object(*size, *alignment)?)
            }
            Arm64ValueStorage::Omitted | Arm64ValueStorage::Direct(_) => None,
        };
        memory_values.push(object);
    }
    Ok(memory_values.into_boxed_slice())
}

fn place_packs(
    body: &nocter_machine::MachineBody,
    builder: &mut Arm64FrameLayoutBuilder,
) -> Result<Box<[Arm64PackFrame]>, Arm64FunctionFrameError> {
    let mut packs = Vec::with_capacity(body.packs().len());
    for (pack_id, pack) in body.packs() {
        if pack_id.index() != packs.len() {
            return Err(Arm64FunctionFrameError::NonDensePack(pack_id));
        }
        let state_layout = Arm64PackStateLayout::build(body, pack).map_err(|error| {
            Arm64FunctionFrameError::PackLayout {
                pack: pack_id,
                error,
            }
        })?;
        let descriptor = builder.add_object(
            Arm64PackDescriptorLayout::SIZE,
            Arm64PackDescriptorLayout::ALIGNMENT,
        )?;
        let state = builder.add_object(state_layout.size(), state_layout.alignment())?;
        packs.push(Arm64PackFrame {
            descriptor,
            state,
            state_layout,
        });
    }
    Ok(packs.into_boxed_slice())
}

fn place_hidden_objects(
    program: &nocter_machine::MachineProgram,
    function_id: MachineFunctionId,
    kind: &MachineFunctionKind,
    builder: &mut Arm64FrameLayoutBuilder,
) -> Result<HiddenObjects, Arm64FunctionFrameError> {
    let callable = match kind {
        MachineFunctionKind::Callable(abi) => Some(abi),
        MachineFunctionKind::ProcessRoot | MachineFunctionKind::TestRoot { .. } => None,
    };
    let indirect_result_pointer = callable
        .is_some_and(|abi| {
            matches!(
                abi.result(),
                MachineResultAbi::Value(result)
                    if matches!(result.location(), MachineResultLocation::CallerStorage { .. })
            )
        })
        .then(|| builder.add_object(Arm64NocterAbi::WORD_SIZE, Arm64NocterAbi::WORD_SIZE))
        .transpose()?;
    let pack_input_pointer = callable
        .is_some_and(|abi| abi.pack().is_some())
        .then(|| builder.add_object(Arm64NocterAbi::WORD_SIZE, Arm64NocterAbi::WORD_SIZE))
        .transpose()?;
    let allocation_context = match program
        .allocation()
        .get(function_id)
        .ok_or(Arm64FunctionFrameError::MissingAllocation(function_id))?
    {
        MachineAllocationRequirement::None => Arm64AllocationContextFrame::None,
        MachineAllocationRequirement::ProgramRoot => Arm64AllocationContextFrame::ProgramRoot(
            builder.add_object(2 * Arm64NocterAbi::WORD_SIZE, Arm64NocterAbi::WORD_SIZE)?,
        ),
        MachineAllocationRequirement::Incoming => Arm64AllocationContextFrame::IncomingPointer(
            builder.add_object(Arm64NocterAbi::WORD_SIZE, Arm64NocterAbi::WORD_SIZE)?,
        ),
    };
    let error_report_buffer = program
        .function(function_id)
        .is_some_and(|function| {
            function.body().operations().any(|(_, operation)| {
                matches!(operation.kind(), MachineOperationKind::ReportError { .. })
            })
        })
        .then(|| {
            builder.add_object(
                crate::error_layout::Arm64ErrorLayout::REPORT_BUFFER_SIZE,
                crate::error_layout::Arm64ErrorLayout::REPORT_BUFFER_ALIGNMENT,
            )
        })
        .transpose()?;
    Ok(HiddenObjects {
        indirect_result_pointer,
        pack_input_pointer,
        allocation_context,
        error_report_buffer,
    })
}

fn call_stack_size(
    program: &nocter_machine::MachineProgram,
    target: &MachineCallTarget,
) -> Result<u64, Arm64FunctionFrameError> {
    match target {
        MachineCallTarget::Direct(target) => direct_stack_size(program, *target),
        MachineCallTarget::Primitive(target) => Ok(target.abi().stack_argument_size()),
    }
}

fn direct_stack_size(
    program: &nocter_machine::MachineProgram,
    target: MachineFunctionId,
) -> Result<u64, Arm64FunctionFrameError> {
    let function = program
        .function(target)
        .ok_or(Arm64FunctionFrameError::UnknownFunction(target))?;
    match function.kind() {
        MachineFunctionKind::Callable(abi) => Ok(abi.stack_argument_size()),
        MachineFunctionKind::ProcessRoot | MachineFunctionKind::TestRoot { .. } => {
            Err(Arm64FunctionFrameError::NonCallableTarget(target))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64FunctionFrameError {
    UnknownFunction(MachineFunctionId),
    NonCallableTarget(MachineFunctionId),
    ForeignValuePlan {
        expected: nocter_machine::MachineLinkageId,
        actual: nocter_machine::MachineLinkageId,
    },
    MissingAllocation(MachineFunctionId),
    NonDenseStack(MachineStackId),
    NonDenseDropFlag(nocter_machine::MachineDropFlagId),
    NonDenseValue(MachineValueId),
    MissingValue(MachineValueId),
    MissingOperationResult(nocter_machine::MachineOperationId),
    NonDensePack(MachinePackId),
    PackLayout {
        pack: MachinePackId,
        error: Arm64PackLayoutError,
    },
    FrameLayout(Arm64FrameLayoutError),
}

impl fmt::Display for Arm64FunctionFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 function frame planning failed: {self:?}")
    }
}

impl std::error::Error for Arm64FunctionFrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PackLayout { error, .. } => Some(error),
            Self::FrameLayout(error) => Some(error),
            Self::UnknownFunction(_)
            | Self::NonCallableTarget(_)
            | Self::ForeignValuePlan { .. }
            | Self::MissingAllocation(_)
            | Self::NonDenseStack(_)
            | Self::NonDenseDropFlag(_)
            | Self::NonDenseValue(_)
            | Self::MissingValue(_)
            | Self::MissingOperationResult(_)
            | Self::NonDensePack(_) => None,
        }
    }
}

impl From<Arm64FrameLayoutError> for Arm64FunctionFrameError {
    fn from(error: Arm64FrameLayoutError) -> Self {
        Self::FrameLayout(error)
    }
}
