use std::fmt;

use nocter_machine::{MachineFrameField, MachineFunctionExecution, MachineFunctionId};

use crate::{
    Arm64AsyncFrameField, Arm64AsyncFrameLayout, Arm64FrameObjectId, Arm64SelectedFunction,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Arm64AsyncActivationTarget {
    Value(nocter_machine::MachineValueId),
    DropFlag(Arm64FrameObjectId),
    Pack(Arm64FrameObjectId),
}

/// One exact persistent-field to transient-activation projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Arm64AsyncActivationField {
    persistent: Arm64AsyncFrameField,
    transient: Arm64AsyncActivationTarget,
}

impl Arm64AsyncActivationField {
    pub(crate) const fn persistent(self) -> Arm64AsyncFrameField {
        self.persistent
    }

    pub(crate) const fn transient(self) -> Arm64AsyncActivationTarget {
        self.transient
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Arm64AsyncActivationState {
    suspend: nocter_machine::MachineBlockId,
    tag: u64,
    fields: Box<[Arm64AsyncActivationField]>,
}

impl Arm64AsyncActivationState {
    pub(crate) const fn suspend(&self) -> nocter_machine::MachineBlockId {
        self.suspend
    }

    pub(crate) const fn tag(&self) -> u64 {
        self.tag
    }

    pub(crate) const fn fields(&self) -> &[Arm64AsyncActivationField] {
        &self.fields
    }
}

/// Complete projection between one persistent async frame and its ordinary native activation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Arm64AsyncActivationPlan {
    initial: Box<[Arm64AsyncActivationField]>,
    states: Box<[Arm64AsyncActivationState]>,
    values: Box<[nocter_machine::MachineValueRepresentation]>,
}

impl Arm64AsyncActivationPlan {
    pub(crate) fn build(
        owner: MachineFunctionId,
        function: &nocter_machine::MachineFunction,
        persistent: &Arm64AsyncFrameLayout,
        selected: &Arm64SelectedFunction,
    ) -> Result<Self, Arm64AsyncActivationPlanError> {
        let MachineFunctionExecution::Deferred(machine) = function.execution() else {
            return Err(Arm64AsyncActivationPlanError::ImmediateFunction(owner));
        };
        let initial = select_fields(machine.initial().fields(), persistent, selected)?;
        let states = machine
            .states()
            .iter()
            .map(|state| {
                let tag = persistent
                    .suspension_tags()
                    .iter()
                    .find(|tag| tag.suspend() == state.suspend())
                    .map(|tag| tag.tag())
                    .ok_or(Arm64AsyncActivationPlanError::MissingStateTag(
                        state.suspend(),
                    ))?;
                Ok(Arm64AsyncActivationState {
                    suspend: state.suspend(),
                    tag,
                    fields: select_fields(state.fields(), persistent, selected)?.into_boxed_slice(),
                })
            })
            .collect::<Result<Vec<_>, Arm64AsyncActivationPlanError>>()?;
        Ok(Self {
            initial: initial.into_boxed_slice(),
            states: states.into_boxed_slice(),
            values: function
                .body()
                .values()
                .map(|(_, value)| value.representation())
                .collect(),
        })
    }

    pub(crate) const fn initial(&self) -> &[Arm64AsyncActivationField] {
        &self.initial
    }

    pub(crate) const fn states(&self) -> &[Arm64AsyncActivationState] {
        &self.states
    }

    pub(crate) fn state(
        &self,
        suspend: nocter_machine::MachineBlockId,
    ) -> Option<&Arm64AsyncActivationState> {
        self.states.iter().find(|state| state.suspend == suspend)
    }

    pub(crate) fn value(
        &self,
        id: nocter_machine::MachineValueId,
    ) -> Option<nocter_machine::MachineValueRepresentation> {
        self.values.get(id.index()).copied()
    }
}

fn select_fields(
    fields: &[MachineFrameField],
    persistent: &Arm64AsyncFrameLayout,
    selected: &Arm64SelectedFunction,
) -> Result<Vec<Arm64AsyncActivationField>, Arm64AsyncActivationPlanError> {
    let mut selected_fields = Vec::new();
    for field in fields.iter().copied() {
        let (persistent, transient) = match field {
            // Stack identities present in the async frame are addressed there directly by
            // selected deferred code. Copying them through an activation slot would create a
            // second address and invalidate loans held by a suspended child.
            MachineFrameField::Stack(id) => {
                persistent
                    .stack_object(id)
                    .ok_or(Arm64AsyncActivationPlanError::MissingPersistentStack(id))?;
                continue;
            }
            MachineFrameField::Value(id) => (
                persistent
                    .value(id)
                    .ok_or(Arm64AsyncActivationPlanError::MissingPersistentValue(id))?,
                Arm64AsyncActivationTarget::Value(id),
            ),
            MachineFrameField::DropFlag(id) => (
                persistent
                    .drop_flag(id)
                    .ok_or(Arm64AsyncActivationPlanError::MissingPersistentFlag(id))?,
                Arm64AsyncActivationTarget::DropFlag(
                    selected
                        .frame()
                        .drop_flag(id)
                        .ok_or(Arm64AsyncActivationPlanError::MissingTransientFlag(id))?,
                ),
            ),
            MachineFrameField::Pack => (
                persistent
                    .pack_input()
                    .ok_or(Arm64AsyncActivationPlanError::MissingPersistentPack)?,
                Arm64AsyncActivationTarget::Pack(
                    selected
                        .frame()
                        .pack_input_pointer()
                        .ok_or(Arm64AsyncActivationPlanError::MissingTransientPack)?,
                ),
            ),
        };
        selected_fields.push(Arm64AsyncActivationField {
            persistent,
            transient,
        });
    }
    Ok(selected_fields)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64AsyncActivationPlanError {
    ImmediateFunction(MachineFunctionId),
    MissingStateTag(nocter_machine::MachineBlockId),
    MissingPersistentStack(nocter_machine::MachineStackId),
    MissingPersistentValue(nocter_machine::MachineValueId),
    MissingPersistentFlag(nocter_machine::MachineDropFlagId),
    MissingTransientFlag(nocter_machine::MachineDropFlagId),
    MissingPersistentPack,
    MissingTransientPack,
}

impl fmt::Display for Arm64AsyncActivationPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 async activation planning failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64AsyncActivationPlanError {}
