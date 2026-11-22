//! Adapts an immediate entry result to process termination.
use super::{Builder, Context, Failure, mir};
use crate::identity::MachineId;
use crate::{
    MachineBlockId, MachineBranchTarget, MachineCall, MachineCallAllocation, MachineCallTarget,
    MachineConstant, MachineFunction, MachineFunctionId, MachineFunctionKind, MachineLayout,
    MachineLayoutKind, MachineOperationKind, MachineOutcomeKind, MachineStackId,
    MachineStackObject, MachineStackPurpose, MachineSwitchCase, MachineSwitchValue,
    MachineTerminator, MachineValueId,
};
use nocter_runtime_contract::{RuntimeDataId, RuntimePrimitive, RuntimeType};

pub(in crate::value_access::executable) fn valid_result(
    program: &mir::Program,
    result: RuntimeDataId,
) -> bool {
    let types = program.runtime().types();
    let success = match types.get(result) {
        Some(RuntimeType::Fallible(success)) => *success,
        _ => result,
    };
    matches!(
        types.get(success),
        Some(
            RuntimeType::Unit
                | RuntimeType::Primitive(RuntimePrimitive::Signed(32) | RuntimePrimitive::Usize)
        )
    )
}

pub(in crate::value_access::executable) fn build(
    context: &Context<'_>,
    owner: MachineFunctionId,
    entry: MachineFunctionId,
    data: RuntimeDataId,
) -> Result<MachineFunction, Failure> {
    let mut builder = Builder::new(context, owner);
    let value = builder.value(
        data,
        false,
        MachineOperationKind::Call(MachineCall::new(
            MachineCallTarget::Direct(entry),
            [],
            MachineCallAllocation::Inherit,
            None,
        )),
    )?;
    match context.program.runtime().types().get(data) {
        Some(RuntimeType::Fallible(_)) => exit_fallible(&mut builder, data, value)?,
        Some(RuntimeType::Unit) => builder.terminate(MachineTerminator::Exit(None))?,
        _ => builder.terminate(MachineTerminator::Exit(Some(value)))?,
    }
    builder.finish(MachineFunctionKind::ProcessRoot, MachineBlockId::new(0))
}

fn exit_fallible(
    builder: &mut Builder<'_>,
    data: RuntimeDataId,
    value: MachineValueId,
) -> Result<(), Failure> {
    let layout = builder
        .context
        .layouts
        .get(data)
        .ok_or(Failure::InvalidRoot)?;
    let &MachineLayoutKind::Outcome {
        kind: kind @ MachineOutcomeKind::Fallible,
        tag_offset,
        payload_offset,
        primary,
        alternate: Some(error),
    } = MachineLayout::kind(layout)
    else {
        return Err(Failure::InvalidRoot);
    };
    let slot = MachineStackId::new(builder.stack.len());
    builder.stack.push(MachineStackObject::new(
        data,
        layout.size(),
        layout.alignment(),
        MachineStackPurpose::Temporary,
    ));
    let base = builder.slot_address(slot);
    builder.emit(MachineOperationKind::Store {
        destination: base,
        value,
    });

    let success = builder.block();
    let failure = builder.block();
    let invalid = builder.block();
    builder.terminate(MachineTerminator::SwitchTag {
        subject: base,
        tag_offset,
        cases: [
            MachineSwitchCase::new(
                MachineSwitchValue::Tag(kind.primary_tag()),
                MachineBranchTarget::new(success, []),
            ),
            MachineSwitchCase::new(
                MachineSwitchValue::Tag(kind.alternate_tag()),
                MachineBranchTarget::new(failure, []),
            ),
        ]
        .into(),
        fallback: MachineBranchTarget::new(invalid, []),
    })?;
    builder.current = invalid;
    builder.terminate(MachineTerminator::Unreachable)?;

    builder.current = success;
    let status = primary
        .filter(|&data| {
            !matches!(
                builder.context.program.runtime().types().get(data),
                Some(RuntimeType::Unit)
            )
        })
        .map(|data| {
            let source = builder.offset_address(base, data, payload_offset)?;
            builder.value(data, false, MachineOperationKind::Load { source })
        })
        .transpose()?;
    builder.terminate(MachineTerminator::Exit(status))?;

    builder.current = failure;
    let place = builder.offset_address(base, error, payload_offset)?;
    builder.emit(MachineOperationKind::ReportError { place });
    builder.emit(MachineOperationKind::ReleaseError { place });
    let status = builder.value(
        builder.primitive(RuntimePrimitive::Signed(32))?,
        false,
        MachineOperationKind::Constant(MachineConstant::Integer(1)),
    )?;
    builder.terminate(MachineTerminator::Exit(Some(status)))
}
