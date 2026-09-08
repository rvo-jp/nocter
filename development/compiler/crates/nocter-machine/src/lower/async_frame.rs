use nocter_mir::{MirCancellationAction, MirFrameField};

use super::MachineProgramError;
use super::body::BodyIdentities;
use super::context::ProgramLoweringContext;
use super::destruction::lower_async_destruction;
use crate::{
    MachineAsyncFrame, MachineCancellationAction, MachineFrameField, MachineInitialAsyncState,
    MachineSuspensionState,
};

pub(super) fn lower_async_frame(
    output: nocter_model::TypeId,
    frame: &nocter_mir::MirAsyncFrame,
    context: ProgramLoweringContext<'_>,
    ids: &BodyIdentities,
) -> Result<MachineAsyncFrame, MachineProgramError> {
    let initial = MachineInitialAsyncState::new(
        lower_fields(frame.initial().fields(), ids)?,
        lower_actions(frame.initial().cancellation(), context, ids)?,
    );
    let states = frame
        .states()
        .iter()
        .map(|state| {
            Ok(MachineSuspensionState::new(
                ids.block(state.suspend())?,
                ids.block(state.resume())?,
                ids.value(state.awaited())?,
                lower_fields(state.fields(), ids)?,
                lower_actions(state.cancellation(), context, ids)?,
            ))
        })
        .collect::<Result<Vec<_>, MachineProgramError>>()?;
    let completed_destruction = frame
        .completed_destruction()
        .map(|plan| lower_async_destruction(plan, ids.owner(), context.layouts, context.functions))
        .transpose()?;
    Ok(MachineAsyncFrame::new(
        super::body::value_representation(output, context.types, context.layouts)?,
        initial,
        states,
        completed_destruction,
    ))
}

fn lower_fields(
    fields: &[MirFrameField],
    ids: &BodyIdentities,
) -> Result<Vec<MachineFrameField>, MachineProgramError> {
    fields
        .iter()
        .map(|field| match field {
            MirFrameField::Pack => Ok(MachineFrameField::Pack),
            MirFrameField::Local(local) => ids.stack(*local).map(MachineFrameField::Stack),
            MirFrameField::Value(value) => ids.value(*value).map(MachineFrameField::Value),
            MirFrameField::DropFlag(flag) => ids.drop_flag(*flag).map(MachineFrameField::DropFlag),
        })
        .collect()
}

fn lower_actions(
    actions: &[MirCancellationAction],
    context: ProgramLoweringContext<'_>,
    ids: &BodyIdentities,
) -> Result<Vec<MachineCancellationAction>, MachineProgramError> {
    actions
        .iter()
        .map(|action| match action {
            MirCancellationAction::ReleaseAwaited(value) => ids
                .value(*value)
                .map(MachineCancellationAction::ReleaseAwaited),
            MirCancellationAction::Destroy {
                place,
                initialized,
                plan,
            } => Ok(MachineCancellationAction::Destroy {
                address: ids.address(*place)?,
                initialized: initialized.map(|flag| ids.drop_flag(flag)).transpose()?,
                plan: lower_async_destruction(
                    plan,
                    ids.owner(),
                    context.layouts,
                    context.functions,
                )?,
            }),
            MirCancellationAction::ReleaseRegion(local) => ids
                .stack(*local)
                .map(MachineCancellationAction::ReleaseRegion),
            MirCancellationAction::DestroyPack => Ok(MachineCancellationAction::DestroyPack),
        })
        .collect()
}
