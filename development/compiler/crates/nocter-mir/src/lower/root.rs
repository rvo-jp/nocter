use nocter_model::{BuiltinType, ExecutableItemId, MirBlockId, TypeId, TypeKind};
use nocter_target_program::{
    ExecutableExecution, ExecutableProgram, ExecutableRoot, ProcessResultContract,
    ProcessSuccessType,
};

use super::MirLoweringError;
use crate::{
    MirBody, MirBodyBuilder, MirBranchTarget, MirCall, MirCallTarget, MirConstant, MirLocalKind,
    MirOperationKind, MirPlaceRoot, MirProcessRoot, MirProjection, MirProjectionKind, MirReadMode,
    MirRoot, MirSwitchCase, MirSwitchSubject, MirSwitchValue, MirTerminator, MirTestRoot,
};

pub(super) fn lower_root(executable: &ExecutableProgram) -> Result<MirRoot, MirLoweringError> {
    match executable.root() {
        ExecutableRoot::Process {
            target,
            entry,
            result,
            execution,
        } => {
            let result_type = executable
                .items()
                .get(*entry)
                .ok_or(MirLoweringError::InvalidRootItem(*entry))?
                .signature()
                .result();
            let body = lower_process_body(executable, *entry, *result, *execution, result_type)?;
            Ok(MirRoot::Process(MirProcessRoot::new(
                *target, *entry, *result, *execution, body,
            )))
        }
        ExecutableRoot::Tests { target, cases } => {
            let cases = cases
                .iter()
                .map(|case| {
                    let result_type = executable
                        .items()
                        .get(case.item())
                        .ok_or(MirLoweringError::InvalidRootItem(case.item()))?
                        .signature()
                        .result();
                    let body = lower_fallible_body(executable, case.item(), result_type)?;
                    Ok(MirTestRoot::new(
                        case.declaration(),
                        case.name(),
                        case.item(),
                        body,
                    ))
                })
                .collect::<Result<Vec<_>, MirLoweringError>>()?
                .into_boxed_slice();
            Ok(MirRoot::Tests {
                target: *target,
                cases,
            })
        }
    }
}

fn lower_process_body(
    executable: &ExecutableProgram,
    entry: ExecutableItemId,
    contract: ProcessResultContract,
    execution: ExecutableExecution,
    result_type: TypeId,
) -> Result<MirBody, MirLoweringError> {
    let expected = process_success_type(executable, contract.success());
    let process_output = match execution {
        ExecutableExecution::Immediate => result_type,
        ExecutableExecution::Deferred { output } => {
            if !matches!(executable.types().get(result_type), Some(TypeKind::Async(actual)) if *actual == output)
            {
                return Err(MirLoweringError::InvalidRootItem(entry));
            }
            output
        }
    };
    let result_valid = if contract.is_fallible() {
        matches!(executable.types().get(process_output), Some(TypeKind::Fallible(payload)) if *payload == expected)
    } else {
        process_output == expected
    };
    if !result_valid {
        return Err(MirLoweringError::InvalidRootItem(entry));
    }
    let mut builder = MirBodyBuilder::new();
    let (block, _) = builder.create_block([]);
    let result = append_process_entry(
        &mut builder,
        block,
        entry,
        execution,
        result_type,
        process_output,
        executable.types(),
    )?;
    if contract.is_fallible() {
        return finish_fallible_body(
            executable,
            entry,
            builder,
            block,
            result.ok_or(MirLoweringError::InvalidRootItem(entry))?,
            process_output,
        );
    }
    let status = match contract.success() {
        ProcessSuccessType::Void => None,
        ProcessSuccessType::I32 | ProcessSuccessType::Usize => {
            Some(result.ok_or(MirLoweringError::InvalidRootItem(entry))?)
        }
    };
    builder.terminate(block, MirTerminator::Exit(status))?;
    builder.finish(block).map_err(Into::into)
}

fn lower_fallible_body(
    executable: &ExecutableProgram,
    entry: ExecutableItemId,
    result_type: TypeId,
) -> Result<MirBody, MirLoweringError> {
    let Some(TypeKind::Fallible(success_type)) = executable.types().get(result_type) else {
        return Err(MirLoweringError::InvalidRootItem(entry));
    };
    let success_type = *success_type;
    let valid_success = [
        executable.types().builtin(BuiltinType::Void),
        executable.types().builtin(BuiltinType::I32),
        executable.types().builtin(BuiltinType::Usize),
    ]
    .contains(&success_type);
    if !valid_success {
        return Err(MirLoweringError::InvalidRootItem(entry));
    }

    let mut builder = MirBodyBuilder::new();
    let (entry_block, _) = builder.create_block([]);
    let outcome = append_entry_call(&mut builder, entry_block, entry, result_type)?;
    finish_fallible_body(
        executable,
        entry,
        builder,
        entry_block,
        outcome,
        result_type,
    )
}

fn finish_fallible_body(
    executable: &ExecutableProgram,
    entry: ExecutableItemId,
    mut builder: MirBodyBuilder,
    entry_block: MirBlockId,
    outcome: nocter_model::MirValueId,
    result_type: TypeId,
) -> Result<MirBody, MirLoweringError> {
    let Some(TypeKind::Fallible(success_type)) = executable.types().get(result_type) else {
        return Err(MirLoweringError::InvalidRootItem(entry));
    };
    let success_type = *success_type;
    let outcome_local = builder.add_local(result_type, MirLocalKind::Temporary, false);
    let outcome_place = builder.add_place(MirPlaceRoot::Local(outcome_local), [], result_type);
    builder.append_effect(
        entry_block,
        MirOperationKind::Initialize {
            destination: outcome_place,
            value: outcome,
        },
    )?;

    let (success, _) = builder.create_block([]);
    let (failure, _) = builder.create_block([]);
    let (invalid_tag, _) = builder.create_block([]);
    builder.terminate(
        entry_block,
        MirTerminator::Switch {
            subject: MirSwitchSubject::Place(outcome_place),
            cases: vec![
                MirSwitchCase::new(
                    MirSwitchValue::FallibleSuccess,
                    MirBranchTarget::new(success, []),
                ),
                MirSwitchCase::new(
                    MirSwitchValue::FallibleFailure,
                    MirBranchTarget::new(failure, []),
                ),
            ]
            .into_boxed_slice(),
            fallback: MirBranchTarget::new(invalid_tag, []),
        },
    )?;
    builder.terminate(invalid_tag, MirTerminator::Unreachable)?;

    let status = if success_type == executable.types().builtin(BuiltinType::Void) {
        None
    } else {
        let place = builder.add_place(
            MirPlaceRoot::Local(outcome_local),
            [MirProjection::new(
                MirProjectionKind::FallibleSuccess,
                success_type,
            )],
            success_type,
        );
        Some(builder.append_value(
            success,
            success_type,
            MirOperationKind::Read {
                place,
                mode: MirReadMode::Copy,
            },
        )?)
    };
    builder.terminate(success, MirTerminator::Exit(status))?;

    let error_type = executable.types().builtin(BuiltinType::Error);
    let error_place = builder.add_place(
        MirPlaceRoot::Local(outcome_local),
        [MirProjection::new(
            MirProjectionKind::FallibleFailure,
            error_type,
        )],
        error_type,
    );
    builder.append_effect(
        failure,
        MirOperationKind::ReportError { place: error_place },
    )?;
    builder.append_effect(
        failure,
        MirOperationKind::ReleaseError { place: error_place },
    )?;
    let one = builder.append_value(
        failure,
        executable.types().builtin(BuiltinType::I32),
        MirOperationKind::Constant(MirConstant::Integer(1)),
    )?;
    builder.terminate(failure, MirTerminator::Exit(Some(one)))?;
    builder.finish(entry_block).map_err(Into::into)
}

fn append_process_entry(
    builder: &mut MirBodyBuilder,
    block: MirBlockId,
    entry: ExecutableItemId,
    execution: ExecutableExecution,
    result_type: TypeId,
    process_output: TypeId,
    types: &nocter_model::TypeStore,
) -> Result<Option<nocter_model::MirValueId>, MirLoweringError> {
    let invoked = append_entry_call(builder, block, entry, result_type)?;
    if execution == ExecutableExecution::Immediate {
        return Ok(Some(invoked));
    }

    let computation_local = builder.add_local(result_type, MirLocalKind::Temporary, false);
    let computation = builder.add_place(MirPlaceRoot::Local(computation_local), [], result_type);
    builder.append_effect(
        block,
        MirOperationKind::Initialize {
            destination: computation,
            value: invoked,
        },
    )?;
    let destination = if matches!(
        types.get(process_output),
        Some(TypeKind::Builtin(BuiltinType::Void))
    ) {
        None
    } else {
        let output_local = builder.add_local(process_output, MirLocalKind::Temporary, false);
        Some(builder.add_place(MirPlaceRoot::Local(output_local), [], process_output))
    };
    builder.append_effect(
        block,
        MirOperationKind::DriveComputation {
            computation,
            destination,
        },
    )?;
    destination
        .map(|place| {
            builder.append_value(
                block,
                process_output,
                MirOperationKind::Read {
                    place,
                    mode: MirReadMode::Move,
                },
            )
        })
        .transpose()
        .map_err(Into::into)
}

fn append_entry_call(
    builder: &mut MirBodyBuilder,
    block: MirBlockId,
    entry: ExecutableItemId,
    result: TypeId,
) -> Result<nocter_model::MirValueId, MirLoweringError> {
    builder
        .append_value(
            block,
            result,
            MirOperationKind::Call(MirCall::new(MirCallTarget::Direct(entry), [])),
        )
        .map_err(Into::into)
}

fn process_success_type(executable: &ExecutableProgram, success: ProcessSuccessType) -> TypeId {
    let builtin = match success {
        ProcessSuccessType::Void => BuiltinType::Void,
        ProcessSuccessType::I32 => BuiltinType::I32,
        ProcessSuccessType::Usize => BuiltinType::Usize,
    };
    executable.types().builtin(builtin)
}
