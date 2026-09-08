use std::collections::BTreeSet;
use std::fmt;

use nocter_machine::{
    MachineContextRequirement, MachineFrameField, MachineFunctionExecution, MachineFunctionId,
    MachineStackPurpose, MachineValueRepresentation,
};

use crate::Arm64NocterAbi;
use crate::object_layout::{Arm64ObjectLayoutError, Arm64ObjectRange, Arm64ObjectSequence};

const MAXIMUM_ALIGNMENT: u64 = Arm64NocterAbi::stack_alignment();
const WORD_SIZE: u64 = Arm64NocterAbi::word_size();

/// One stable byte range in an allocation-backed asynchronous computation frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64AsyncFrameField {
    offset: u64,
    size: u64,
    alignment: u64,
}

impl Arm64AsyncFrameField {
    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }
}

impl From<Arm64ObjectRange> for Arm64AsyncFrameField {
    fn from(range: Arm64ObjectRange) -> Self {
        Self {
            offset: range.offset(),
            size: range.size(),
            alignment: range.alignment(),
        }
    }
}

/// The stored tag assigned to one exact Machine suspension state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64AsyncSuspensionTag {
    suspend: nocter_machine::MachineBlockId,
    tag: u64,
}

impl Arm64AsyncSuspensionTag {
    #[must_use]
    pub const fn suspend(self) -> nocter_machine::MachineBlockId {
        self.suspend
    }

    #[must_use]
    pub const fn tag(self) -> u64 {
        self.tag
    }
}

/// Complete heap-frame placement for one deferred Machine function.
///
/// Machine owns which identities survive suspension. This target plan assigns each such identity
/// exactly one stable ARM64 byte range; instruction selection must consume this projection rather
/// than repeat liveness or choose state-specific storage independently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64AsyncFrameLayout {
    size: u64,
    alignment: u64,
    resume_function: Arm64AsyncFrameField,
    cancel_function: Arm64AsyncFrameField,
    consume_function: Arm64AsyncFrameField,
    state_tag: Arm64AsyncFrameField,
    allocation_context: Arm64AsyncFrameField,
    process_context: Option<Arm64AsyncFrameField>,
    pack_input: Option<Arm64AsyncFrameField>,
    output: Option<Arm64AsyncFrameField>,
    stack_objects: Box<[Option<Arm64AsyncFrameField>]>,
    drop_flags: Box<[Option<Arm64AsyncFrameField>]>,
    values: Box<[Option<Arm64AsyncFrameField>]>,
    suspension_tags: Box<[Arm64AsyncSuspensionTag]>,
    initial_tag: u64,
    completed_tag: u64,
}

impl Arm64AsyncFrameLayout {
    /// Places the union of the already-derived Machine frame fields and the closed runtime header.
    ///
    /// # Errors
    ///
    /// Rejects an immediate function, missing identities or layouts, invalid ambient-context
    /// requirements, non-stored live values, state-tag exhaustion, and target layout overflow.
    pub fn build(
        program: &nocter_machine::MachineProgram,
        owner: MachineFunctionId,
    ) -> Result<Self, Arm64AsyncFrameLayoutError> {
        let function = program
            .function(owner)
            .ok_or(Arm64AsyncFrameLayoutError::UnknownFunction(owner))?;
        let MachineFunctionExecution::Deferred(frame) = function.execution() else {
            return Err(Arm64AsyncFrameLayoutError::ImmediateFunction(owner));
        };
        if program.contexts().allocation().get(owner) != Some(MachineContextRequirement::Incoming) {
            return Err(Arm64AsyncFrameLayoutError::InvalidAllocationContext(owner));
        }

        let body = function.body();
        let fields = collect_frame_fields(frame);
        let asynchronous = Arm64NocterAbi::asynchronous();
        let mut sequence = Arm64ObjectSequence::new(
            asynchronous.fixed_header_size(),
            asynchronous.fixed_header_alignment(),
            MAXIMUM_ALIGNMENT,
        );
        let header = place_header(
            program,
            owner,
            fields.contains(&MachineFrameField::Pack),
            &mut sequence,
        )?;
        let output = match frame.output_representation() {
            MachineValueRepresentation::Completion | MachineValueRepresentation::Diverging => None,
            MachineValueRepresentation::Stored {
                size, alignment, ..
            } => add_stored(&mut sequence, size, alignment)?,
        };
        let placed = place_body_fields(body, fields, &mut sequence)?;
        let suspension_count = u64::try_from(frame.states().len())
            .ok()
            .ok_or(Arm64AsyncFrameLayoutError::StateTagExhausted(owner))?;
        let state_tags = asynchronous
            .state_tags(suspension_count)
            .ok_or(Arm64AsyncFrameLayoutError::StateTagExhausted(owner))?;
        let suspension_tags = build_suspension_tags(owner, frame, state_tags)?;
        let (size, alignment) = sequence.finish(WORD_SIZE)?;
        Ok(Self {
            size,
            alignment,
            resume_function: header.resume_function,
            cancel_function: header.cancel_function,
            consume_function: header.consume_function,
            state_tag: header.state_tag,
            allocation_context: header.allocation_context,
            process_context: header.process_context,
            pack_input: header.pack_input,
            output,
            stack_objects: placed.stack_objects,
            drop_flags: placed.drop_flags,
            values: placed.values,
            suspension_tags: suspension_tags.into_boxed_slice(),
            initial_tag: state_tags.initial(),
            completed_tag: state_tags.completed(),
        })
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    #[must_use]
    pub const fn resume_function(&self) -> Arm64AsyncFrameField {
        self.resume_function
    }

    #[must_use]
    pub const fn cancel_function(&self) -> Arm64AsyncFrameField {
        self.cancel_function
    }

    #[must_use]
    pub const fn consume_function(&self) -> Arm64AsyncFrameField {
        self.consume_function
    }

    #[must_use]
    pub const fn state_tag(&self) -> Arm64AsyncFrameField {
        self.state_tag
    }

    #[must_use]
    pub const fn allocation_context(&self) -> Arm64AsyncFrameField {
        self.allocation_context
    }

    #[must_use]
    pub const fn process_context(&self) -> Option<Arm64AsyncFrameField> {
        self.process_context
    }

    #[must_use]
    pub const fn pack_input(&self) -> Option<Arm64AsyncFrameField> {
        self.pack_input
    }

    #[must_use]
    pub const fn output(&self) -> Option<Arm64AsyncFrameField> {
        self.output
    }

    #[must_use]
    pub fn stack_object(&self, id: nocter_machine::MachineStackId) -> Option<Arm64AsyncFrameField> {
        self.stack_objects.get(id.index()).copied().flatten()
    }

    #[must_use]
    pub fn drop_flag(&self, id: nocter_machine::MachineDropFlagId) -> Option<Arm64AsyncFrameField> {
        self.drop_flags.get(id.index()).copied().flatten()
    }

    #[must_use]
    pub fn value(&self, id: nocter_machine::MachineValueId) -> Option<Arm64AsyncFrameField> {
        self.values.get(id.index()).copied().flatten()
    }

    #[must_use]
    pub const fn suspension_tags(&self) -> &[Arm64AsyncSuspensionTag] {
        &self.suspension_tags
    }

    #[must_use]
    pub const fn initial_tag(&self) -> u64 {
        self.initial_tag
    }

    #[must_use]
    pub const fn completed_tag(&self) -> u64 {
        self.completed_tag
    }
}

struct RuntimeHeader {
    resume_function: Arm64AsyncFrameField,
    cancel_function: Arm64AsyncFrameField,
    consume_function: Arm64AsyncFrameField,
    state_tag: Arm64AsyncFrameField,
    allocation_context: Arm64AsyncFrameField,
    process_context: Option<Arm64AsyncFrameField>,
    pack_input: Option<Arm64AsyncFrameField>,
}

struct PlacedBodyFields {
    stack_objects: Box<[Option<Arm64AsyncFrameField>]>,
    drop_flags: Box<[Option<Arm64AsyncFrameField>]>,
    values: Box<[Option<Arm64AsyncFrameField>]>,
}

fn collect_frame_fields(frame: &nocter_machine::MachineAsyncFrame) -> BTreeSet<MachineFrameField> {
    let mut fields: BTreeSet<MachineFrameField> =
        frame.initial().fields().iter().copied().collect();
    for state in frame.states() {
        fields.extend(state.fields().iter().copied());
    }
    fields
}

fn place_header(
    program: &nocter_machine::MachineProgram,
    owner: MachineFunctionId,
    retains_pack: bool,
    sequence: &mut Arm64ObjectSequence,
) -> Result<RuntimeHeader, Arm64AsyncFrameLayoutError> {
    let asynchronous = Arm64NocterAbi::asynchronous();
    let resume_function = fixed_header_field(asynchronous.resume_function_offset());
    let cancel_function = fixed_header_field(asynchronous.cancel_function_offset());
    let consume_function = fixed_header_field(asynchronous.consume_function_offset());
    let state_tag = fixed_header_field(asynchronous.state_tag_offset());
    let allocation_context = fixed_header_field(asynchronous.allocation_context_offset());
    let process_context = match program.contexts().process().get(owner) {
        Some(MachineContextRequirement::None) => None,
        Some(MachineContextRequirement::Incoming) => Some(add_word(sequence)?),
        Some(MachineContextRequirement::ProgramRoot) | None => {
            return Err(Arm64AsyncFrameLayoutError::InvalidProcessContext(owner));
        }
    };
    let pack_input = retains_pack.then(|| add_word(sequence)).transpose()?;
    Ok(RuntimeHeader {
        resume_function,
        cancel_function,
        consume_function,
        state_tag,
        allocation_context,
        process_context,
        pack_input,
    })
}

fn place_body_fields(
    body: &nocter_machine::MachineBody,
    fields: BTreeSet<MachineFrameField>,
    sequence: &mut Arm64ObjectSequence,
) -> Result<PlacedBodyFields, Arm64AsyncFrameLayoutError> {
    let mut stack_objects = vec![None; body.stack_objects().len()];
    let mut drop_flags = vec![None; body.drop_flags().len()];
    let mut values = vec![None; body.values().len()];
    for field in fields {
        match field {
            MachineFrameField::Pack => {}
            MachineFrameField::Stack(id) => {
                let object = body
                    .stack(id)
                    .ok_or(Arm64AsyncFrameLayoutError::UnknownStack(id))?;
                let (size, alignment) = stack_layout(object);
                stack_objects[id.index()] = add_stored(sequence, size, alignment)?;
            }
            MachineFrameField::DropFlag(id) => {
                body.drop_flag(id)
                    .ok_or(Arm64AsyncFrameLayoutError::UnknownDropFlag(id))?;
                drop_flags[id.index()] = Some(sequence.add(1, 1)?.into());
            }
            MachineFrameField::Value(id) => {
                let value = body
                    .value(id)
                    .ok_or(Arm64AsyncFrameLayoutError::UnknownValue(id))?;
                let MachineValueRepresentation::Stored {
                    size, alignment, ..
                } = value.representation()
                else {
                    return Err(Arm64AsyncFrameLayoutError::NonStoredValue(id));
                };
                values[id.index()] = add_stored(sequence, size, alignment)?;
            }
        }
    }
    Ok(PlacedBodyFields {
        stack_objects: stack_objects.into_boxed_slice(),
        drop_flags: drop_flags.into_boxed_slice(),
        values: values.into_boxed_slice(),
    })
}

const fn stack_layout(object: nocter_machine::MachineStackObject) -> (u64, u64) {
    match object.purpose() {
        MachineStackPurpose::Region => (
            crate::region_layout::Arm64RegionLayout::SIZE,
            crate::region_layout::Arm64RegionLayout::ALIGNMENT,
        ),
        MachineStackPurpose::Parameter { .. }
        | MachineStackPurpose::User
        | MachineStackPurpose::Temporary => (object.size(), object.alignment()),
    }
}

fn build_suspension_tags(
    owner: MachineFunctionId,
    frame: &nocter_machine::MachineAsyncFrame,
    state_tags: nocter_runtime_contract::RuntimeAsyncStateTags,
) -> Result<Vec<Arm64AsyncSuspensionTag>, Arm64AsyncFrameLayoutError> {
    frame
        .states()
        .iter()
        .enumerate()
        .map(|(index, state)| {
            u64::try_from(index)
                .ok()
                .and_then(|index| state_tags.suspension(index))
                .map(|tag| Arm64AsyncSuspensionTag {
                    suspend: state.suspend(),
                    tag,
                })
                .ok_or(Arm64AsyncFrameLayoutError::StateTagExhausted(owner))
        })
        .collect()
}

const fn fixed_header_field(offset: u64) -> Arm64AsyncFrameField {
    Arm64AsyncFrameField {
        offset,
        size: WORD_SIZE,
        alignment: WORD_SIZE,
    }
}

fn add_word(
    sequence: &mut Arm64ObjectSequence,
) -> Result<Arm64AsyncFrameField, Arm64AsyncFrameLayoutError> {
    sequence
        .add(WORD_SIZE, WORD_SIZE)
        .map(Arm64AsyncFrameField::from)
        .map_err(Arm64AsyncFrameLayoutError::from)
}

fn add_stored(
    sequence: &mut Arm64ObjectSequence,
    size: u64,
    alignment: u64,
) -> Result<Option<Arm64AsyncFrameField>, Arm64AsyncFrameLayoutError> {
    sequence
        .add(size, alignment)
        .map(Arm64AsyncFrameField::from)
        .map(Some)
        .map_err(Arm64AsyncFrameLayoutError::from)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64AsyncFrameLayoutError {
    UnknownFunction(MachineFunctionId),
    ImmediateFunction(MachineFunctionId),
    InvalidAllocationContext(MachineFunctionId),
    InvalidProcessContext(MachineFunctionId),
    UnknownStack(nocter_machine::MachineStackId),
    UnknownDropFlag(nocter_machine::MachineDropFlagId),
    UnknownValue(nocter_machine::MachineValueId),
    NonStoredValue(nocter_machine::MachineValueId),
    StateTagExhausted(MachineFunctionId),
    InvalidAlignment(u64),
    SizeOverflow,
}

impl fmt::Display for Arm64AsyncFrameLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 async frame layout failed: {self:?}")
    }
}

impl std::error::Error for Arm64AsyncFrameLayoutError {}

impl From<Arm64ObjectLayoutError> for Arm64AsyncFrameLayoutError {
    fn from(error: Arm64ObjectLayoutError) -> Self {
        match error {
            Arm64ObjectLayoutError::InvalidAlignment(alignment) => {
                Self::InvalidAlignment(alignment)
            }
            Arm64ObjectLayoutError::SizeOverflow => Self::SizeOverflow,
        }
    }
}
