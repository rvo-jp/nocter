use nocter_mir::{MirBranchTarget, MirSwitchSubject, MirSwitchValue, MirTerminator};

use super::MachineProgramError;
use super::body::BodyIdentities;
use crate::{
    MachineBlock, MachineBranchTarget, MachineLayoutKind, MachineLayoutStore, MachineOutcomeKind,
    MachineSwitchCase, MachineSwitchValue, MachineTerminator,
};

pub(super) fn lower_blocks(
    body: &nocter_mir::MirBody,
    layouts: &MachineLayoutStore,
    ids: &BodyIdentities,
) -> Result<Vec<MachineBlock>, MachineProgramError> {
    body.blocks()
        .iter()
        .map(|(_, block)| {
            let parameters = block
                .parameters()
                .iter()
                .map(|value| ids.value(*value))
                .collect::<Result<Vec<_>, _>>()?;
            let operations = block
                .operations()
                .iter()
                .map(|operation| ids.operation(*operation))
                .collect::<Result<Vec<_>, _>>()?;
            let terminator = lower_terminator(body, block.terminator(), layouts, ids)?;
            Ok(MachineBlock::new(parameters, operations, terminator))
        })
        .collect()
}

fn lower_terminator(
    body: &nocter_mir::MirBody,
    terminator: &MirTerminator,
    layouts: &MachineLayoutStore,
    ids: &BodyIdentities,
) -> Result<MachineTerminator, MachineProgramError> {
    match terminator {
        MirTerminator::Goto(target) => {
            Ok(MachineTerminator::Goto(lower_branch_target(target, ids)?))
        }
        MirTerminator::Branch {
            condition,
            then_target,
            else_target,
        } => Ok(MachineTerminator::Branch {
            condition: ids.value(*condition)?,
            then_target: lower_branch_target(then_target, ids)?,
            else_target: lower_branch_target(else_target, ids)?,
        }),
        MirTerminator::BranchDropFlag {
            flag,
            initialized,
            uninitialized,
        } => Ok(MachineTerminator::BranchDropFlag {
            flag: ids.drop_flag(*flag)?,
            initialized: lower_branch_target(initialized, ids)?,
            uninitialized: lower_branch_target(uninitialized, ids)?,
        }),
        MirTerminator::Switch {
            subject: MirSwitchSubject::Value(subject),
            cases,
            fallback,
        } => lower_value_switch(*subject, cases, fallback, ids),
        MirTerminator::Switch {
            subject: MirSwitchSubject::Place(subject),
            cases,
            fallback,
        } => lower_tag_switch(body, *subject, cases, fallback, layouts, ids),
        MirTerminator::Return(value) => Ok(MachineTerminator::Return(
            value.map(|value| ids.value(value)).transpose()?,
        )),
        MirTerminator::Exit(value) => Ok(MachineTerminator::Exit(
            value.map(|value| ids.value(value)).transpose()?,
        )),
        MirTerminator::Trap => Ok(MachineTerminator::Trap),
        MirTerminator::Unreachable => Ok(MachineTerminator::Unreachable),
    }
}

fn lower_tag_switch(
    body: &nocter_mir::MirBody,
    subject: nocter_model::MirPlaceId,
    cases: &[nocter_mir::MirSwitchCase],
    fallback: &MirBranchTarget,
    layouts: &MachineLayoutStore,
    ids: &BodyIdentities,
) -> Result<MachineTerminator, MachineProgramError> {
    let ty = body
        .places()
        .get(subject)
        .ok_or(MachineProgramError::UnsupportedPlaceSwitch(ids.owner()))?
        .ty();
    let layout = layouts
        .get(ty)
        .ok_or(MachineProgramError::MissingStoredLayout(ty))?;
    let (tag_offset, cases) = match layout.kind() {
        MachineLayoutKind::Enum {
            tag_offset,
            variants,
            ..
        } => (
            *tag_offset,
            cases
                .iter()
                .map(|case| {
                    let MirSwitchValue::Variant(variant) = case.value() else {
                        return Err(MachineProgramError::InvalidTagSwitch(ids.owner()));
                    };
                    let tag = variants
                        .iter()
                        .find(|candidate| candidate.variant() == variant)
                        .map(crate::MachineEnumVariantLayout::tag)
                        .ok_or(MachineProgramError::InvalidTagSwitch(ids.owner()))?;
                    lower_tag_case(tag, case.target(), ids)
                })
                .collect::<Result<Vec<_>, MachineProgramError>>()?,
        ),
        MachineLayoutKind::Outcome {
            kind, tag_offset, ..
        } => (
            *tag_offset,
            cases
                .iter()
                .map(|case| {
                    let tag = outcome_tag(*kind, case.value())
                        .ok_or(MachineProgramError::InvalidTagSwitch(ids.owner()))?;
                    lower_tag_case(tag, case.target(), ids)
                })
                .collect::<Result<Vec<_>, MachineProgramError>>()?,
        ),
        _ => return Err(MachineProgramError::InvalidTagSwitch(ids.owner())),
    };
    Ok(MachineTerminator::SwitchTag {
        subject: ids.address(subject)?,
        tag_offset,
        cases: cases.into_boxed_slice(),
        fallback: lower_branch_target(fallback, ids)?,
    })
}

fn lower_tag_case(
    tag: u8,
    target: &MirBranchTarget,
    ids: &BodyIdentities,
) -> Result<MachineSwitchCase, MachineProgramError> {
    Ok(MachineSwitchCase::new(
        MachineSwitchValue::Tag(tag),
        lower_branch_target(target, ids)?,
    ))
}

const fn outcome_tag(kind: MachineOutcomeKind, value: MirSwitchValue) -> Option<u8> {
    match (kind, value) {
        (MachineOutcomeKind::Optional, MirSwitchValue::OptionalPresent)
        | (MachineOutcomeKind::Fallible, MirSwitchValue::FallibleSuccess) => Some(0),
        (MachineOutcomeKind::Optional, MirSwitchValue::OptionalAbsent)
        | (MachineOutcomeKind::Fallible, MirSwitchValue::FallibleFailure) => Some(1),
        _ => None,
    }
}

fn lower_value_switch(
    subject: nocter_model::MirValueId,
    cases: &[nocter_mir::MirSwitchCase],
    fallback: &MirBranchTarget,
    ids: &BodyIdentities,
) -> Result<MachineTerminator, MachineProgramError> {
    let cases = cases
        .iter()
        .map(|case| {
            let MirSwitchValue::Integer(value) = case.value() else {
                return Err(MachineProgramError::InvalidValueSwitch(ids.owner()));
            };
            Ok(MachineSwitchCase::new(
                MachineSwitchValue::Integer(value),
                lower_branch_target(case.target(), ids)?,
            ))
        })
        .collect::<Result<Vec<_>, MachineProgramError>>()?;
    Ok(MachineTerminator::SwitchValue {
        subject: ids.value(subject)?,
        cases: cases.into_boxed_slice(),
        fallback: lower_branch_target(fallback, ids)?,
    })
}

fn lower_branch_target(
    target: &MirBranchTarget,
    ids: &BodyIdentities,
) -> Result<MachineBranchTarget, MachineProgramError> {
    let arguments = target
        .arguments()
        .iter()
        .map(|argument| ids.value(*argument))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MachineBranchTarget::new(
        ids.block(target.block())?,
        arguments,
    ))
}
