use nocter_mir::{
    MirCallTarget, MirOperationKind, MirPackContribution, MirPackSegment, MirValueDefinition,
};
use nocter_model::{BorrowCapability, MirOperationId, TypeKind};

use super::MachineProgramError;
use super::body::BodyIdentities;
use super::context::ProgramLoweringContext;
use crate::{
    MachineAddress, MachineAddressRoot, MachineAddressStep, MachineArgumentLocation,
    MachineFunctionId, MachinePack, MachinePackContribution, MachinePackNext, MachinePackSegment,
    MachinePackSpread, MachineValueClass,
};

pub(super) fn lower_packs(
    body: &nocter_mir::MirBody,
    addresses: &[MachineAddress],
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
                .map(|pack| lower_pack(operation, pack, body, addresses, context, ids))
        })
        .collect()
}

fn lower_pack(
    operation: MirOperationId,
    pack: &nocter_mir::MirPackArgument,
    body: &nocter_mir::MirBody,
    addresses: &[MachineAddress],
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
                let MirCallTarget::Direct(item) = spread.next_target() else {
                    return Err(MachineProgramError::InvalidPackTarget {
                        owner: ids.owner(),
                        operation,
                    });
                };
                let target = context
                    .functions
                    .get(item)
                    .copied()
                    .ok_or(MachineProgramError::MissingItemFunction(*item))?;
                let abi = context
                    .abi
                    .get(*item)
                    .cloned()
                    .ok_or(MachineProgramError::MissingCallableAbi(*item))?;
                let receiver_offset =
                    receiver_offset(operation, spread, body, addresses, context, ids, &abi)?;
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
                        receiver_offset,
                        target,
                        abi,
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

fn receiver_offset(
    operation: MirOperationId,
    spread: &nocter_mir::MirPackSpread,
    body: &nocter_mir::MirBody,
    addresses: &[MachineAddress],
    context: ProgramLoweringContext<'_>,
    ids: &BodyIdentities,
    abi: &crate::MachineCallableAbi,
) -> Result<u64, MachineProgramError> {
    let invalid = || MachineProgramError::InvalidPackReceiver {
        owner: ids.owner(),
        operation,
    };
    let receiver = body
        .values()
        .get(spread.receiver())
        .copied()
        .ok_or_else(invalid)?;
    let MirValueDefinition::Operation(receiver_operation) = receiver.definition() else {
        return Err(invalid());
    };
    let receiver_operation = body
        .operations()
        .get(receiver_operation)
        .ok_or_else(invalid)?;
    let MirOperationKind::Borrow {
        place: receiver_place,
        capability: BorrowCapability::ReadWrite,
    } = receiver_operation.kind()
    else {
        return Err(invalid());
    };
    let Some(TypeKind::Borrow {
        capability: BorrowCapability::ReadWrite,
        referent,
    }) = context.types.get(receiver.ty())
    else {
        return Err(invalid());
    };
    let iterator =
        machine_address(addresses, ids.address(spread.iterator())?).ok_or_else(invalid)?;
    let receiver_address =
        machine_address(addresses, ids.address(*receiver_place)?).ok_or_else(invalid)?;
    if receiver_address.ty() != *referent
        || iterator.root() != receiver_address.root()
        || !matches!(iterator.root(), MachineAddressRoot::Stack(_))
    {
        return Err(invalid());
    }
    validate_next_abi(abi, receiver.ty(), spread.next_result(), context).map_err(|()| invalid())?;
    let iterator_offset = static_address_offset(iterator).ok_or_else(invalid)?;
    let receiver_offset = static_address_offset(receiver_address).ok_or_else(invalid)?;
    let relative = receiver_offset
        .checked_sub(iterator_offset)
        .ok_or_else(invalid)?;
    let receiver_end = relative
        .checked_add(receiver_address.size())
        .ok_or_else(invalid)?;
    if receiver_end > iterator.size() {
        return Err(invalid());
    }
    Ok(relative)
}

fn validate_next_abi(
    abi: &crate::MachineCallableAbi,
    receiver: nocter_model::TypeId,
    result: nocter_model::TypeId,
    context: ProgramLoweringContext<'_>,
) -> Result<(), ()> {
    let [argument] = abi.arguments() else {
        return Err(());
    };
    let Some(MachineArgumentLocation::Registers(registers)) = argument.location() else {
        return Err(());
    };
    if argument.ty() != receiver
        || argument.class() != (MachineValueClass::Direct { words: 1 })
        || registers.first() != 0
        || registers.words() != 1
        || abi.pack().is_some()
        || abi.stack_argument_size() != 0
        || abi.result()
            != crate::transport::plan_result(context.types, context.layouts, result)
                .map_err(|_| ())?
    {
        return Err(());
    }
    Ok(())
}

fn machine_address(
    addresses: &[MachineAddress],
    id: crate::MachineAddressId,
) -> Option<&MachineAddress> {
    addresses.get(id.index())
}

fn static_address_offset(address: &MachineAddress) -> Option<u64> {
    address.steps().iter().try_fold(0_u64, |offset, step| {
        let MachineAddressStep::Offset(step) = step else {
            return None;
        };
        offset.checked_add(*step)
    })
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
