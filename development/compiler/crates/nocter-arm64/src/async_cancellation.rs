use std::fmt;

use nocter_machine::{
    MachineAddressExtent, MachineAddressRoot, MachineAddressStep, MachineCancellationAction,
    MachineFrameField, MachineFunctionExecution, MachineFunctionId, MachineIndex,
    MachineIndexBound, MachineValueClass, MachineValueRepresentation,
};

use crate::{
    Arm64AsyncFrameField, Arm64AsyncFrameLayout, Arm64FrameLayout, Arm64FrameLayoutBuilder,
    Arm64FrameLayoutError, Arm64FrameObjectId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Arm64AsyncAddressRoot {
    Frame(Arm64AsyncFrameField),
    Pointer(Arm64AsyncFrameField),
    View {
        field: Arm64AsyncFrameField,
        pointer_offset: u64,
        length_offset: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Arm64AsyncAddressIndex {
    Constant(u64),
    Frame(Arm64AsyncFrameField),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Arm64AsyncAddressBound {
    Fixed(u64),
    CurrentView,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Arm64AsyncAddressStep {
    Offset(u64),
    OffsetFrame(Arm64AsyncFrameField),
    Dereference,
    ViewDereference {
        pointer_offset: u64,
        length_offset: u64,
    },
    Index {
        index: Arm64AsyncAddressIndex,
        stride: u64,
        bound: Arm64AsyncAddressBound,
    },
}

/// One stored cancellation address projected entirely onto stable async-frame byte ranges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Arm64AsyncAddress {
    root: Arm64AsyncAddressRoot,
    steps: Box<[Arm64AsyncAddressStep]>,
}

impl Arm64AsyncAddress {
    pub(crate) fn frame(field: Arm64AsyncFrameField) -> Self {
        Self {
            root: Arm64AsyncAddressRoot::Frame(field),
            steps: Box::new([]),
        }
    }

    pub(crate) const fn root(&self) -> Arm64AsyncAddressRoot {
        self.root
    }

    pub(crate) const fn steps(&self) -> &[Arm64AsyncAddressStep] {
        &self.steps
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Arm64AsyncCancellationAction {
    ReleaseAwaited(Arm64AsyncFrameField),
    Destroy {
        address: Arm64AsyncAddress,
        initialized: Option<Arm64AsyncFrameField>,
        destruction: MachineFunctionId,
    },
    ReleaseRegion(Arm64AsyncFrameField),
    DestroyPack,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Arm64AsyncCancellationState {
    tag: u64,
    actions: Box<[Arm64AsyncCancellationAction]>,
}

impl Arm64AsyncCancellationState {
    pub(crate) const fn tag(&self) -> u64 {
        self.tag
    }

    pub(crate) const fn actions(&self) -> &[Arm64AsyncCancellationAction] {
        &self.actions
    }
}

/// Closed target plan for cancellation dispatch and its one transient native activation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Arm64AsyncCancellationPlan {
    states: Box<[Arm64AsyncCancellationState]>,
    completed_destruction: Option<MachineFunctionId>,
    activation: Arm64AsyncCancellationFrame,
}

impl Arm64AsyncCancellationPlan {
    pub(crate) fn build(
        owner: MachineFunctionId,
        function: &nocter_machine::MachineFunction,
        frame: &Arm64AsyncFrameLayout,
    ) -> Result<Self, Arm64AsyncCancellationPlanError> {
        let MachineFunctionExecution::Deferred(machine) = function.execution() else {
            return Err(Arm64AsyncCancellationPlanError::ImmediateFunction(owner));
        };
        let mut states = Vec::with_capacity(machine.states().len() + 1);
        states.push(select_state(
            function,
            frame,
            frame.initial_tag(),
            machine.initial().fields(),
            machine.initial().cancellation(),
        )?);
        for state in machine.states() {
            let tag = frame
                .suspension_tags()
                .iter()
                .find(|tag| tag.suspend() == state.suspend())
                .map(|tag| tag.tag())
                .ok_or(Arm64AsyncCancellationPlanError::MissingStateTag(
                    state.suspend(),
                ))?;
            states.push(select_state(
                function,
                frame,
                tag,
                state.fields(),
                state.cancellation(),
            )?);
        }
        let mut builder = Arm64FrameLayoutBuilder::new();
        let mapping = builder.add_object(8, 8)?;
        Ok(Self {
            states: states.into_boxed_slice(),
            completed_destruction: machine.completed_destruction(),
            activation: Arm64AsyncCancellationFrame {
                layout: builder.finish()?,
                mapping,
            },
        })
    }

    pub(crate) const fn states(&self) -> &[Arm64AsyncCancellationState] {
        &self.states
    }

    pub(crate) const fn completed_destruction(&self) -> Option<MachineFunctionId> {
        self.completed_destruction
    }

    pub(crate) const fn activation(&self) -> &Arm64AsyncCancellationFrame {
        &self.activation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Arm64AsyncCancellationFrame {
    layout: Arm64FrameLayout,
    mapping: Arm64FrameObjectId,
}

impl Arm64AsyncCancellationFrame {
    pub(crate) const fn layout(&self) -> &Arm64FrameLayout {
        &self.layout
    }

    pub(crate) const fn mapping(&self) -> Arm64FrameObjectId {
        self.mapping
    }
}

fn select_state(
    function: &nocter_machine::MachineFunction,
    frame: &Arm64AsyncFrameLayout,
    tag: u64,
    fields: &[MachineFrameField],
    actions: &[MachineCancellationAction],
) -> Result<Arm64AsyncCancellationState, Arm64AsyncCancellationPlanError> {
    let actions = actions
        .iter()
        .map(|action| select_action(function, frame, fields, action))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Arm64AsyncCancellationState {
        tag,
        actions: actions.into_boxed_slice(),
    })
}

fn select_action(
    function: &nocter_machine::MachineFunction,
    frame: &Arm64AsyncFrameLayout,
    fields: &[MachineFrameField],
    action: &MachineCancellationAction,
) -> Result<Arm64AsyncCancellationAction, Arm64AsyncCancellationPlanError> {
    match action {
        MachineCancellationAction::ReleaseAwaited(value) => {
            Ok(Arm64AsyncCancellationAction::ReleaseAwaited(require_value(
                function, frame, fields, *value,
            )?))
        }
        MachineCancellationAction::Destroy {
            address,
            initialized,
            destruction,
        } => Ok(Arm64AsyncCancellationAction::Destroy {
            address: select_address(function, frame, fields, *address)?,
            initialized: initialized
                .map(|flag| require_flag(frame, fields, flag))
                .transpose()?,
            destruction: *destruction,
        }),
        MachineCancellationAction::ReleaseRegion(region) => {
            if !fields.contains(&MachineFrameField::Stack(*region)) {
                return Err(Arm64AsyncCancellationPlanError::UnavailableStack(*region));
            }
            frame
                .stack_object(*region)
                .map(Arm64AsyncCancellationAction::ReleaseRegion)
                .ok_or(Arm64AsyncCancellationPlanError::UnavailableStack(*region))
        }
        MachineCancellationAction::DestroyPack => Ok(Arm64AsyncCancellationAction::DestroyPack),
    }
}

fn select_address(
    function: &nocter_machine::MachineFunction,
    frame: &Arm64AsyncFrameLayout,
    fields: &[MachineFrameField],
    id: nocter_machine::MachineAddressId,
) -> Result<Arm64AsyncAddress, Arm64AsyncCancellationPlanError> {
    let address = function
        .body()
        .address(id)
        .ok_or(Arm64AsyncCancellationPlanError::UnknownAddress(id))?;
    if !matches!(address.extent(), MachineAddressExtent::Stored { .. }) {
        return Err(Arm64AsyncCancellationPlanError::ViewDestruction(id));
    }
    let (root, current_view) = select_address_root(function, frame, fields, id, address.root())?;
    let steps = select_address_steps(function, frame, fields, id, address.steps(), current_view)?;
    Ok(Arm64AsyncAddress {
        root,
        steps: steps.into_boxed_slice(),
    })
}

fn select_address_root(
    function: &nocter_machine::MachineFunction,
    frame: &Arm64AsyncFrameLayout,
    fields: &[MachineFrameField],
    id: nocter_machine::MachineAddressId,
    root: MachineAddressRoot,
) -> Result<(Arm64AsyncAddressRoot, bool), Arm64AsyncCancellationPlanError> {
    match root {
        MachineAddressRoot::Stack(stack) => {
            if !fields.contains(&MachineFrameField::Stack(stack)) {
                return Err(Arm64AsyncCancellationPlanError::UnavailableStack(stack));
            }
            Ok((
                Arm64AsyncAddressRoot::Frame(
                    frame
                        .stack_object(stack)
                        .ok_or(Arm64AsyncCancellationPlanError::UnavailableStack(stack))?,
                ),
                false,
            ))
        }
        MachineAddressRoot::Pointer { value } => Ok((
            Arm64AsyncAddressRoot::Pointer(require_word_value(function, frame, fields, value)?),
            false,
        )),
        MachineAddressRoot::View {
            value,
            pointer_offset,
            length_offset,
        } => {
            let field = require_value(function, frame, fields, value)?;
            require_word_range(field, pointer_offset)?;
            require_word_range(field, length_offset)?;
            Ok((
                Arm64AsyncAddressRoot::View {
                    field,
                    pointer_offset,
                    length_offset,
                },
                true,
            ))
        }
        MachineAddressRoot::Data(_) => Err(Arm64AsyncCancellationPlanError::NonOwnedAddress(id)),
    }
}

fn select_address_steps(
    function: &nocter_machine::MachineFunction,
    frame: &Arm64AsyncFrameLayout,
    fields: &[MachineFrameField],
    id: nocter_machine::MachineAddressId,
    source: &[MachineAddressStep],
    mut current_view: bool,
) -> Result<Vec<Arm64AsyncAddressStep>, Arm64AsyncCancellationPlanError> {
    let mut steps = Vec::with_capacity(source.len());
    for step in source {
        steps.push(match *step {
            MachineAddressStep::Offset(offset) => Arm64AsyncAddressStep::Offset(offset),
            MachineAddressStep::OffsetValue(value) => Arm64AsyncAddressStep::OffsetFrame(
                require_word_value(function, frame, fields, value)?,
            ),
            MachineAddressStep::Dereference => {
                current_view = false;
                Arm64AsyncAddressStep::Dereference
            }
            MachineAddressStep::ViewDereference {
                pointer_offset,
                length_offset,
            } => {
                current_view = true;
                Arm64AsyncAddressStep::ViewDereference {
                    pointer_offset,
                    length_offset,
                }
            }
            MachineAddressStep::Index {
                index,
                stride,
                bound,
            } => {
                let bound = match bound {
                    MachineIndexBound::Fixed(length) => Arm64AsyncAddressBound::Fixed(length),
                    MachineIndexBound::CurrentView if current_view => {
                        Arm64AsyncAddressBound::CurrentView
                    }
                    MachineIndexBound::CurrentView => {
                        return Err(Arm64AsyncCancellationPlanError::MissingViewBound(id));
                    }
                };
                current_view = false;
                Arm64AsyncAddressStep::Index {
                    index: match index {
                        MachineIndex::Constant(index) => Arm64AsyncAddressIndex::Constant(index),
                        MachineIndex::Value(value) => Arm64AsyncAddressIndex::Frame(
                            require_word_value(function, frame, fields, value)?,
                        ),
                    },
                    stride,
                    bound,
                }
            }
        });
    }
    if current_view {
        return Err(Arm64AsyncCancellationPlanError::ViewDestruction(id));
    }
    Ok(steps)
}

fn require_value(
    function: &nocter_machine::MachineFunction,
    frame: &Arm64AsyncFrameLayout,
    fields: &[MachineFrameField],
    value: nocter_machine::MachineValueId,
) -> Result<Arm64AsyncFrameField, Arm64AsyncCancellationPlanError> {
    if !fields.contains(&MachineFrameField::Value(value)) || function.body().value(value).is_none()
    {
        return Err(Arm64AsyncCancellationPlanError::UnavailableValue(value));
    }
    frame
        .value(value)
        .ok_or(Arm64AsyncCancellationPlanError::UnavailableValue(value))
}

fn require_word_value(
    function: &nocter_machine::MachineFunction,
    frame: &Arm64AsyncFrameLayout,
    fields: &[MachineFrameField],
    value: nocter_machine::MachineValueId,
) -> Result<Arm64AsyncFrameField, Arm64AsyncCancellationPlanError> {
    let field = require_value(function, frame, fields, value)?;
    let representation = function
        .body()
        .value(value)
        .map(nocter_machine::MachineValue::representation)
        .ok_or(Arm64AsyncCancellationPlanError::UnavailableValue(value))?;
    if !matches!(
        representation,
        MachineValueRepresentation::Stored {
            size: 8,
            class: MachineValueClass::Direct { words: 1 },
            ..
        }
    ) {
        return Err(Arm64AsyncCancellationPlanError::NonWordValue(value));
    }
    Ok(field)
}

fn require_flag(
    frame: &Arm64AsyncFrameLayout,
    fields: &[MachineFrameField],
    flag: nocter_machine::MachineDropFlagId,
) -> Result<Arm64AsyncFrameField, Arm64AsyncCancellationPlanError> {
    if !fields.contains(&MachineFrameField::DropFlag(flag)) {
        return Err(Arm64AsyncCancellationPlanError::UnavailableFlag(flag));
    }
    frame
        .drop_flag(flag)
        .ok_or(Arm64AsyncCancellationPlanError::UnavailableFlag(flag))
}

fn require_word_range(
    field: Arm64AsyncFrameField,
    offset: u64,
) -> Result<(), Arm64AsyncCancellationPlanError> {
    if offset.checked_add(8).is_none_or(|end| end > field.size()) {
        return Err(Arm64AsyncCancellationPlanError::InvalidViewLayout);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Arm64AsyncCancellationPlanError {
    ImmediateFunction(MachineFunctionId),
    MissingStateTag(nocter_machine::MachineBlockId),
    UnknownAddress(nocter_machine::MachineAddressId),
    NonOwnedAddress(nocter_machine::MachineAddressId),
    ViewDestruction(nocter_machine::MachineAddressId),
    MissingViewBound(nocter_machine::MachineAddressId),
    UnavailableStack(nocter_machine::MachineStackId),
    UnavailableValue(nocter_machine::MachineValueId),
    UnavailableFlag(nocter_machine::MachineDropFlagId),
    NonWordValue(nocter_machine::MachineValueId),
    InvalidViewLayout,
    Frame(Arm64FrameLayoutError),
}

impl fmt::Display for Arm64AsyncCancellationPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 async cancellation planning failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64AsyncCancellationPlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            Self::ImmediateFunction(_)
            | Self::MissingStateTag(_)
            | Self::UnknownAddress(_)
            | Self::NonOwnedAddress(_)
            | Self::ViewDestruction(_)
            | Self::MissingViewBound(_)
            | Self::UnavailableStack(_)
            | Self::UnavailableValue(_)
            | Self::UnavailableFlag(_)
            | Self::NonWordValue(_)
            | Self::InvalidViewLayout => None,
        }
    }
}

impl From<Arm64FrameLayoutError> for Arm64AsyncCancellationPlanError {
    fn from(error: Arm64FrameLayoutError) -> Self {
        Self::Frame(error)
    }
}
