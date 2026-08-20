use nocter_mir::{MirOperationKind, MirPackContribution, MirPackSegment};
use nocter_model::MirOperationId;

use super::MachineProgramError;
use super::body::BodyIdentities;
use super::call::lower_call_target;
use super::context::ProgramLoweringContext;
use crate::{
    MachineFunctionId, MachinePack, MachinePackContribution, MachinePackNext, MachinePackSegment,
    MachinePackSpread,
};

pub(super) fn lower_packs(
    body: &nocter_mir::MirBody,
    context: ProgramLoweringContext<'_>,
    ids: &BodyIdentities,
) -> Result<Vec<MachinePack>, MachineProgramError> {
    body.operations()
        .iter()
        .filter_map(|(operation, value)| {
            let MirOperationKind::Call(call) = value.kind() else {
                return None;
            };
            call.pack()
                .map(|pack| lower_pack(operation, pack, context, ids))
        })
        .collect()
}

fn lower_pack(
    operation: MirOperationId,
    pack: &nocter_mir::MirPackArgument,
    context: ProgramLoweringContext<'_>,
    ids: &BodyIdentities,
) -> Result<MachinePack, MachineProgramError> {
    let segments = pack
        .segments()
        .iter()
        .enumerate()
        .map(|(segment_index, segment)| match segment {
            MirPackSegment::Value { value, destruction } => Ok(MachinePackSegment::Value {
                value: ids.value(*value)?,
                destruction: pack_destruction(
                    destruction.is_some(),
                    segment_index,
                    operation,
                    context,
                    ids,
                )?,
            }),
            MirPackSegment::Spread(spread) => {
                let target = lower_call_target(operation, spread.next_target(), context, ids)?;
                let contribution = match spread.contribution() {
                    MirPackContribution::Direct => MachinePackContribution::Direct,
                    MirPackContribution::CopyBorrowed => MachinePackContribution::CopyBorrowed,
                };
                let destruction = pack_destruction(
                    spread.destruction().is_some(),
                    segment_index,
                    operation,
                    context,
                    ids,
                )?;
                Ok(MachinePackSegment::Spread(MachinePackSpread::new(
                    ids.address(spread.iterator())?,
                    ids.value(spread.remaining())?,
                    MachinePackNext::new(
                        ids.value(spread.receiver())?,
                        target,
                        spread.next_result(),
                        spread.item(),
                    ),
                    contribution,
                    destruction,
                )))
            }
        })
        .collect::<Result<Vec<_>, MachineProgramError>>()?;
    Ok(MachinePack::new(
        pack.element(),
        pack.next(),
        crate::transport::plan_result(context.types, context.layouts, pack.next())?,
        ids.value(pack.length())?,
        segments,
    ))
}

fn pack_destruction(
    required: bool,
    segment: usize,
    operation: MirOperationId,
    context: ProgramLoweringContext<'_>,
    ids: &BodyIdentities,
) -> Result<Option<MachineFunctionId>, MachineProgramError> {
    if !required {
        return Ok(None);
    }
    let destruction = context
        .destructions
        .pack_segment(ids.owner(), operation, segment)
        .ok_or(MachineProgramError::MissingPackDestruction {
            owner: ids.owner(),
            operation,
            segment,
        })?;
    context
        .destruction_functions
        .get(&destruction)
        .copied()
        .map(Some)
        .ok_or(MachineProgramError::MissingDestruction(destruction))
}
