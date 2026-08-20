use nocter_model::{
    Arena, BorrowCapability, BuiltinType, ExecutableItemId, MirPlaceId, MirValueId, TypeId,
    TypeKind,
};
use nocter_target_program::{ExecutableItemKey, ExecutableProgram};

use crate::program::{MirProgramBuildError, MirProgramOwner};
use crate::validation_closure::has_valid_closure_environment_signature;
use crate::{
    MirAggregate, MirBody, MirCallTarget, MirDestructionKind, MirDestructionPlan, MirFunction,
    MirOperationKind, MirPackSegment, MirPrimitiveDependency, MirRoot,
};

pub(crate) fn validate_program(
    functions: &Arena<ExecutableItemId, MirFunction>,
    root: &MirRoot,
    executable: &ExecutableProgram,
) -> Result<(), MirProgramBuildError> {
    for (caller, function) in functions.iter() {
        validate_body(
            MirProgramOwner::Function(caller),
            function.body(),
            functions,
            executable,
        )?;
    }
    match root {
        MirRoot::Process(process) => {
            let owner = MirProgramOwner::ProcessRoot;
            validate_root_entry(owner, process.body(), process.entry())?;
            validate_body(owner, process.body(), functions, executable)?;
        }
        MirRoot::Tests { cases, .. } => {
            for case in cases {
                let owner = MirProgramOwner::TestRoot(case.declaration());
                validate_root_entry(owner, case.body(), case.item())?;
                validate_body(owner, case.body(), functions, executable)?;
            }
        }
    }
    Ok(())
}

fn validate_root_entry(
    owner: MirProgramOwner,
    body: &MirBody,
    expected: ExecutableItemId,
) -> Result<(), MirProgramBuildError> {
    let mut calls = body
        .operations()
        .iter()
        .filter_map(|(_, operation)| match operation.kind() {
            MirOperationKind::Call(call) => Some(call),
            _ => None,
        });
    let Some(call) = calls.next() else {
        return Err(MirProgramBuildError::InvalidRootCall { owner, expected });
    };
    if calls.next().is_some()
        || call.target() != &MirCallTarget::Direct(expected)
        || !call.arguments().is_empty()
        || call.pack().is_some()
        || call.allocation() != crate::MirCallAllocation::Inherit
    {
        return Err(MirProgramBuildError::InvalidRootCall { owner, expected });
    }
    Ok(())
}

fn validate_body(
    caller: MirProgramOwner,
    body: &MirBody,
    functions: &Arena<ExecutableItemId, MirFunction>,
    executable: &ExecutableProgram,
) -> Result<(), MirProgramBuildError> {
    for (_, operation) in body.operations().iter() {
        match operation.kind() {
            MirOperationKind::Call(call) => {
                if let MirCallTarget::Direct(callee) = call.target() {
                    validate_direct_call(
                        caller,
                        body,
                        *callee,
                        call.arguments(),
                        operation.result().and_then(|value| value_type(body, value)),
                        call.pack().map(|pack| (pack.element(), pack.next())),
                        functions,
                    )?;
                }
                if let MirCallTarget::StandardPrimitive {
                    dependency:
                        MirPrimitiveDependency::Destruction {
                            plan: Some(plan), ..
                        },
                    ..
                } = call.target()
                {
                    validate_deferred_drop_calls(caller, plan, functions, executable)?;
                }
                validate_pack_dependencies(caller, body, call, functions, executable)?;
            }
            MirOperationKind::InvokeDrop {
                body: drop_body,
                place,
            } => {
                let place_type = place_type(body, *place);
                if !place_type.is_some_and(|ty| {
                    has_valid_drop_signature(*drop_body, ty, functions, executable)
                }) {
                    return Err(MirProgramBuildError::DropCallSignature {
                        caller,
                        callee: *drop_body,
                        place: *place,
                    });
                }
            }
            MirOperationKind::Aggregate(MirAggregate::Closure { body, .. }) => {
                validate_closure(caller, *body, functions, executable)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_closure(
    caller: MirProgramOwner,
    body: ExecutableItemId,
    functions: &Arena<ExecutableItemId, MirFunction>,
    executable: &ExecutableProgram,
) -> Result<(), MirProgramBuildError> {
    let function = functions
        .get(body)
        .ok_or(MirProgramBuildError::UnknownItem(body))?;
    let layout = executable
        .closure_layout(body)
        .ok_or(MirProgramBuildError::ClosureConstructionSignature { caller, body })?;
    if !has_valid_closure_environment_signature(function, layout, executable.types()) {
        return Err(MirProgramBuildError::ClosureConstructionSignature { caller, body });
    }
    Ok(())
}

fn validate_direct_call(
    owner: MirProgramOwner,
    body: &MirBody,
    target: ExecutableItemId,
    arguments: &[MirValueId],
    result: Option<TypeId>,
    pack: Option<(TypeId, TypeId)>,
    functions: &Arena<ExecutableItemId, MirFunction>,
) -> Result<(), MirProgramBuildError> {
    let target_function = functions
        .get(target)
        .ok_or(MirProgramBuildError::UnknownItem(target))?;
    let parameter_types = target_function
        .parameters()
        .iter()
        .map(|parameter| {
            target_function
                .locals()
                .get(*parameter)
                .copied()
                .map(crate::MirLocal::ty)
                .expect("validated MIR parameter must exist")
        })
        .collect::<Vec<_>>();
    let expected_pack = target_function
        .pack()
        .map(|input| (input.element(), input.next()));
    if parameter_types.len() != arguments.len()
        || parameter_types
            .iter()
            .copied()
            .zip(arguments.iter().copied())
            .any(|(expected, argument)| value_type(body, argument) != Some(expected))
        || result != Some(target_function.result())
        || pack != expected_pack
    {
        return Err(MirProgramBuildError::DirectCallSignature {
            caller: owner,
            callee: target,
        });
    }
    Ok(())
}

fn validate_pack_dependencies(
    caller: MirProgramOwner,
    body: &MirBody,
    call: &crate::MirCall,
    functions: &Arena<ExecutableItemId, MirFunction>,
    executable: &ExecutableProgram,
) -> Result<(), MirProgramBuildError> {
    let Some(pack) = call.pack() else {
        return Ok(());
    };
    for segment in pack.segments() {
        match segment {
            MirPackSegment::Value { destruction, .. } => {
                if let Some(plan) = destruction {
                    validate_deferred_drop_calls(caller, plan, functions, executable)?;
                }
            }
            MirPackSegment::Spread(spread) => {
                if let MirCallTarget::Direct(callee) = spread.next_target() {
                    validate_direct_call(
                        caller,
                        body,
                        *callee,
                        &[spread.receiver()],
                        Some(spread.next_result()),
                        None,
                        functions,
                    )?;
                }
                if let Some(plan) = spread.destruction() {
                    validate_deferred_drop_calls(caller, plan, functions, executable)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_deferred_drop_calls(
    caller: MirProgramOwner,
    plan: &MirDestructionPlan,
    functions: &Arena<ExecutableItemId, MirFunction>,
    executable: &ExecutableProgram,
) -> Result<(), MirProgramBuildError> {
    let (drop, children): (Option<ExecutableItemId>, Vec<&MirDestructionPlan>) = match plan.kind() {
        MirDestructionKind::Struct { drop, fields } => (
            *drop,
            fields
                .iter()
                .map(crate::MirFieldDestruction::plan)
                .collect(),
        ),
        MirDestructionKind::Enum { drop, variants } => (
            *drop,
            variants
                .iter()
                .flat_map(crate::MirVariantDestruction::payload)
                .map(crate::MirPayloadDestruction::plan)
                .collect(),
        ),
        MirDestructionKind::FixedArray { element, .. }
        | MirDestructionKind::Optional(element)
        | MirDestructionKind::Fallible(element)
        | MirDestructionKind::Opaque { plan: element, .. } => (None, vec![element]),
        MirDestructionKind::Closure(captures) => (
            None,
            captures
                .iter()
                .map(crate::MirCaptureDestruction::plan)
                .collect(),
        ),
    };
    if let Some(drop) = drop
        && !has_valid_drop_signature(drop, plan.ty(), functions, executable)
    {
        return Err(MirProgramBuildError::DeferredDropSignature {
            caller,
            callee: drop,
            ty: plan.ty(),
        });
    }
    for child in children {
        validate_deferred_drop_calls(caller, child, functions, executable)?;
    }
    Ok(())
}

fn has_valid_drop_signature(
    body: ExecutableItemId,
    referent: TypeId,
    functions: &Arena<ExecutableItemId, MirFunction>,
    executable: &ExecutableProgram,
) -> bool {
    let Some(function) = functions.get(body) else {
        return false;
    };
    let is_drop = matches!(
        executable
            .items()
            .get(body)
            .map(nocter_target_program::ExecutableItem::key),
        Some(ExecutableItemKey::Drop(_))
    );
    function.parameters().len() == 1
        && function.result() == executable.types().builtin(BuiltinType::Void)
        && is_drop
        && matches!(
            parameter_type(function, 0).and_then(|ty| executable.types().get(ty)),
            Some(TypeKind::Borrow {
                capability: BorrowCapability::ReadWrite,
                referent: actual,
            }) if *actual == referent
        )
}

fn value_type(body: &MirBody, value: MirValueId) -> Option<TypeId> {
    body.values().get(value).copied().map(crate::MirValue::ty)
}

fn place_type(body: &MirBody, place: MirPlaceId) -> Option<TypeId> {
    body.places().get(place).map(crate::MirPlace::ty)
}

fn parameter_type(function: &MirFunction, position: usize) -> Option<TypeId> {
    function
        .parameters()
        .get(position)
        .and_then(|parameter| function.locals().get(*parameter))
        .copied()
        .map(crate::MirLocal::ty)
}
