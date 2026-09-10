use std::collections::BTreeSet;

use nocter_model::{ExecutableItemId, TypeKind};

use crate::validation_destruction::validate_destruction_plan;
use crate::validation_graph::place_values;
use crate::{
    MirCancellationAction, MirFrameField, MirFunction, MirFunctionExecution, MirLocalKind,
    MirPlaceRoot, MirTerminator, MirValidationEnvironment, MirValidationError,
};

pub(crate) fn validate_async_function(
    function: &MirFunction,
    environment: &impl MirValidationEnvironment,
) -> Result<(), MirValidationError> {
    let invalid = || MirValidationError::InvalidAsyncFrame(function.item());
    let Some(frame) = function.async_frame() else {
        return if function.execution() == MirFunctionExecution::Immediate {
            Ok(())
        } else {
            Err(invalid())
        };
    };
    let MirFunctionExecution::Deferred { output } = function.execution() else {
        return Err(invalid());
    };
    let body = function.body();
    let mut expected_initial = body
        .parameters()
        .iter()
        .copied()
        .map(MirFrameField::Local)
        .collect::<BTreeSet<_>>();
    if body.pack().is_some() {
        expected_initial.insert(MirFrameField::Pack);
    }
    if frame.initial().fields() != expected_initial.into_iter().collect::<Vec<_>>() {
        return Err(invalid());
    }
    validate_cancellation(
        function.item(),
        function,
        environment,
        frame.initial().fields(),
        frame.initial().cancellation(),
        None,
    )?;
    if let Some(plan) = frame.completed_destruction() {
        if plan.ty() != output {
            return Err(invalid());
        }
        validate_destruction_plan(environment, plan)?;
    }

    let suspension_count = body
        .blocks()
        .iter()
        .filter(|(_, block)| matches!(block.terminator(), MirTerminator::Suspend { .. }))
        .count();
    if suspension_count != frame.states().len() {
        return Err(invalid());
    }
    for state in frame.states() {
        let block = body
            .blocks()
            .get(state.suspend())
            .ok_or(MirValidationError::UnknownBlock(state.suspend()))?;
        let MirTerminator::Suspend {
            computation,
            resume,
        } = block.terminator()
        else {
            return Err(invalid());
        };
        let computation_type = body
            .values()
            .get(*computation)
            .ok_or(MirValidationError::UnknownValue(*computation))?
            .ty();
        if *computation != state.awaited()
            || resume.block() != state.resume()
            || !matches!(
                environment.types().get(computation_type),
                Some(TypeKind::Future(_))
            )
            || state.fields().windows(2).any(|pair| pair[0] >= pair[1])
            || state
                .stable_storage()
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid());
        }
        validate_fields(function.item(), function, state.fields())?;
        for local in state.stable_storage() {
            if function.locals().get(*local).is_none()
                || !state.fields().contains(&MirFrameField::Local(*local))
            {
                return Err(invalid());
            }
        }
        validate_cancellation(
            function.item(),
            function,
            environment,
            state.fields(),
            state.cancellation(),
            Some(*computation),
        )?;
    }
    Ok(())
}

fn validate_fields(
    item: ExecutableItemId,
    function: &MirFunction,
    fields: &[MirFrameField],
) -> Result<(), MirValidationError> {
    for field in fields {
        let exists = match field {
            MirFrameField::Pack => function.pack().is_some(),
            MirFrameField::Local(local) => function.locals().get(*local).is_some(),
            MirFrameField::Value(value) => function.values().get(*value).is_some(),
            MirFrameField::DropFlag(flag) => function.drop_flags().get(*flag).is_some(),
        };
        if !exists {
            return Err(MirValidationError::InvalidAsyncFrame(item));
        }
    }
    Ok(())
}

fn validate_cancellation(
    item: ExecutableItemId,
    function: &MirFunction,
    environment: &impl MirValidationEnvironment,
    fields: &[MirFrameField],
    actions: &[MirCancellationAction],
    awaited: Option<nocter_model::MirValueId>,
) -> Result<(), MirValidationError> {
    let invalid = || MirValidationError::InvalidAsyncFrame(item);
    if awaited.is_some()
        != matches!(
            actions.first(),
            Some(MirCancellationAction::ReleaseAwaited(_))
        )
    {
        return Err(invalid());
    }
    for action in actions {
        match action {
            MirCancellationAction::ReleaseAwaited(value) => {
                if Some(*value) != awaited || !fields.contains(&MirFrameField::Value(*value)) {
                    return Err(invalid());
                }
            }
            MirCancellationAction::Destroy {
                place,
                initialized,
                plan,
            } => {
                let place_value = function
                    .places()
                    .get(*place)
                    .ok_or(MirValidationError::UnknownPlace(*place))?;
                if place_value.ty() != plan.ty() {
                    return Err(invalid());
                }
                validate_destruction_plan(environment, plan)?;
                if let Some(flag) = initialized {
                    let flag_value = function
                        .drop_flags()
                        .get(*flag)
                        .ok_or(MirValidationError::UnknownDropFlag(*flag))?;
                    if flag_value.place() != *place
                        || !fields.contains(&MirFrameField::DropFlag(*flag))
                    {
                        return Err(invalid());
                    }
                }
                if let MirPlaceRoot::Local(local) = place_value.root()
                    && !fields.contains(&MirFrameField::Local(local))
                {
                    return Err(invalid());
                }
                for value in place_values(place_value) {
                    if !fields.contains(&MirFrameField::Value(value)) {
                        return Err(invalid());
                    }
                }
            }
            MirCancellationAction::ReleaseRegion(local) => {
                if function.locals().get(*local).map(|local| local.kind())
                    != Some(MirLocalKind::Region)
                    || !fields.contains(&MirFrameField::Local(*local))
                {
                    return Err(invalid());
                }
            }
            MirCancellationAction::DestroyPack => {
                if function.pack().is_none() || !fields.contains(&MirFrameField::Pack) {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}
