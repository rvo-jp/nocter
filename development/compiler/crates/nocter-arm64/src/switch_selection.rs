use nocter_machine::{
    MachineAddressId, MachineBranchTarget, MachineFunction, MachineSwitchCase, MachineSwitchValue,
    MachineValueId,
};

use crate::{
    Arm64FunctionFrame, Arm64NocterAbi, Arm64SelectedAddressPlan, Arm64SelectedInstruction,
    Arm64SelectedLoadExtension, Arm64SelectedRegister, Arm64SelectedSwitchCase,
    Arm64SelectedTerminator, Arm64SelectionError, Arm64ValuePlan,
};

#[derive(Clone, Copy)]
pub(crate) struct SwitchSelectionContext<'a> {
    function: &'a MachineFunction,
    values: &'a Arm64ValuePlan,
    frame: &'a Arm64FunctionFrame,
}

impl<'a> SwitchSelectionContext<'a> {
    pub(crate) const fn new(
        function: &'a MachineFunction,
        values: &'a Arm64ValuePlan,
        frame: &'a Arm64FunctionFrame,
    ) -> Self {
        Self {
            function,
            values,
            frame,
        }
    }
}

pub(crate) fn select_value(
    context: SwitchSelectionContext<'_>,
    subject: MachineValueId,
    cases: &[MachineSwitchCase],
    fallback: &MachineBranchTarget,
) -> Result<Arm64SelectedTerminator, Arm64SelectionError> {
    let subject = crate::selection::direct_value(context.values, subject)?;
    if subject.is_empty() || subject.len() > usize::from(Arm64NocterAbi::direct_value_word_limit())
    {
        return Err(Arm64SelectionError::SwitchSubject);
    }
    build_switch(
        context,
        subject
            .iter()
            .map(|register| Arm64SelectedRegister::Virtual(*register))
            .collect(),
        cases,
        fallback,
    )
}

pub(crate) fn select_tag(
    context: SwitchSelectionContext<'_>,
    subject: MachineAddressId,
    tag_offset: u64,
    cases: &[MachineSwitchCase],
    fallback: &MachineBranchTarget,
    addresses: &Arm64SelectedAddressPlan,
    selected: &mut Vec<Arm64SelectedInstruction>,
) -> Result<Arm64SelectedTerminator, Arm64SelectionError> {
    let register = Arm64SelectedRegister::Fixed(
        Arm64NocterAbi::compiler_scratch_register(0)
            .expect("the ABI reserves x16 for compiler-owned tag transport"),
    );
    let address = addresses.use_address(subject, selected)?;
    selected.push(Arm64SelectedInstruction::LoadMemory {
        bytes: 1,
        extension: Arm64SelectedLoadExtension::Zero,
        destination: register,
        source: crate::memory_selection::offset_memory_address(address, tag_offset)?,
    });
    build_switch(context, Box::new([register]), cases, fallback)
}

fn build_switch(
    context: SwitchSelectionContext<'_>,
    subject: Box<[Arm64SelectedRegister]>,
    cases: &[MachineSwitchCase],
    fallback: &MachineBranchTarget,
) -> Result<Arm64SelectedTerminator, Arm64SelectionError> {
    let cases = cases
        .iter()
        .map(|case| {
            let value = match case.value() {
                MachineSwitchValue::Integer(value) => value.cast_unsigned(),
                MachineSwitchValue::Tag(value) => u128::from(value),
            };
            Ok(Arm64SelectedSwitchCase::new(
                value,
                crate::selection::select_edge(
                    context.function,
                    case.target(),
                    context.values,
                    context.frame,
                )?,
            ))
        })
        .collect::<Result<Vec<_>, Arm64SelectionError>>()?;
    Ok(Arm64SelectedTerminator::Switch {
        subject,
        cases: cases.into_boxed_slice(),
        fallback: crate::selection::select_edge(
            context.function,
            fallback,
            context.values,
            context.frame,
        )?,
    })
}
