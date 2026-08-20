use nocter_mir::{MirOperationKind, MirPackContribution, MirPackSegment};
use nocter_model::MirOperationId;

use super::MachineProgramError;
use super::body::BodyIdentities;
use super::call::lower_call_target;
use super::context::ProgramLoweringContext;
use super::destruction::lower_destruction;
use crate::{
    MachinePack, MachinePackContribution, MachinePackNext, MachinePackSegment, MachinePackSpread,
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
        .map(|segment| match segment {
            MirPackSegment::Value { value, destruction } => Ok(MachinePackSegment::Value {
                value: ids.value(*value)?,
                destruction: destruction
                    .as_ref()
                    .map(|plan| {
                        lower_destruction(
                            plan,
                            ids.owner(),
                            operation,
                            context.layouts,
                            context.functions,
                        )
                    })
                    .transpose()?,
            }),
            MirPackSegment::Spread(spread) => {
                let target = lower_call_target(operation, spread.next_target(), context, ids)?;
                let contribution = match spread.contribution() {
                    MirPackContribution::Direct => MachinePackContribution::Direct,
                    MirPackContribution::CopyBorrowed => MachinePackContribution::CopyBorrowed,
                };
                let destruction = spread
                    .destruction()
                    .map(|plan| {
                        lower_destruction(
                            plan,
                            ids.owner(),
                            operation,
                            context.layouts,
                            context.functions,
                        )
                    })
                    .transpose()?;
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
        ids.value(pack.length())?,
        segments,
    ))
}
