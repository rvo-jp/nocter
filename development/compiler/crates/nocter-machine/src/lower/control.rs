use nocter_mir::{MirBranchTarget, MirSwitchSubject, MirSwitchValue, MirTerminator};

use super::MachineProgramError;
use super::body::BodyIdentities;
use crate::{
    MachineBlock, MachineBranchTarget, MachineSwitchCase, MachineSwitchValue, MachineTerminator,
};

pub(super) fn lower_blocks(
    body: &nocter_mir::MirBody,
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
            let terminator = lower_terminator(block.terminator(), ids)?;
            Ok(MachineBlock::new(parameters, operations, terminator))
        })
        .collect()
}

fn lower_terminator(
    terminator: &MirTerminator,
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
            subject: MirSwitchSubject::Place(_),
            ..
        } => Err(MachineProgramError::UnsupportedPlaceSwitch(ids.owner())),
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
