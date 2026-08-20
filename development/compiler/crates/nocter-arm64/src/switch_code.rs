use nocter_machine::MachineBlockId;

use crate::{
    Arm64AddSubtract, Arm64BranchCondition, Arm64CodeBuilder, Arm64DataRegister, Arm64DataSize,
    Arm64Instruction, Arm64MaterializationError, Arm64SelectedEdge, Arm64SelectedFunction,
    Arm64SelectedRegister, Arm64SelectedSwitchCase,
};

pub(crate) fn emit(
    function: &Arm64SelectedFunction,
    subject: &[Arm64SelectedRegister],
    cases: &[Arm64SelectedSwitchCase],
    fallback: &Arm64SelectedEdge,
    labels: &[(MachineBlockId, crate::Arm64LabelId)],
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    if subject.is_empty() || subject.len() > 2 {
        return Err(Arm64MaterializationError::InvalidSwitchWidth(subject.len()));
    }

    let copy_labels = cases
        .iter()
        .map(|case| case.edge().has_copies().then(|| code.create_label()))
        .collect::<Vec<_>>();
    for (case, copy_label) in cases.iter().zip(copy_labels.iter().copied()) {
        let target = if let Some(copy_label) = copy_label {
            copy_label
        } else {
            crate::selected_code::block_label(labels, case.edge().target())?
        };
        emit_case_comparison(function, subject, case.value(), target, code)?;
    }
    crate::selected_code::emit_edge(function, fallback, labels, code)?;

    for (case, copy_label) in cases.iter().zip(copy_labels) {
        if let Some(copy_label) = copy_label {
            code.bind(copy_label)?;
            crate::selected_code::emit_edge(function, case.edge(), labels, code)?;
        }
    }
    Ok(())
}

fn emit_case_comparison(
    function: &Arm64SelectedFunction,
    subject: &[Arm64SelectedRegister],
    expected: u128,
    target: crate::Arm64LabelId,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let low = u64::try_from(expected & u128::from(u64::MAX))
        .expect("a masked switch lane is exactly 64 bits");
    compare_lane(function, subject[0], low, code)?;
    if subject.len() == 1 {
        code.branch_conditional(target, Arm64BranchCondition::Equal);
        return Ok(());
    }

    let next_case = code.create_label();
    code.branch_conditional(next_case, Arm64BranchCondition::NotEqual);
    let high = u64::try_from(expected >> 64).expect("a shifted switch lane is exactly 64 bits");
    compare_lane(function, subject[1], high, code)?;
    code.branch_conditional(target, Arm64BranchCondition::Equal);
    code.bind(next_case)?;
    Ok(())
}

fn compare_lane(
    function: &Arm64SelectedFunction,
    subject: Arm64SelectedRegister,
    expected: u64,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    let subject = crate::selected_code::read_register(function, subject, 0, code)?;
    let expected_register = crate::frame_access::scratch(1);
    crate::frame_access::load_immediate(code, expected_register, expected, Arm64DataSize::Bits64);
    code.append(Arm64Instruction::AddSubtractRegister {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64DataRegister::Zero,
        left: Arm64DataRegister::General(subject),
        right: Arm64DataRegister::General(expected_register),
    });
    Ok(())
}
