//! Definition boundary for compiler-owned asynchronous primitive target families.

use crate::{
    Arm64AsyncInterestLifecycleTargets, Arm64AsyncPrimitiveTargets, Arm64LoweringError,
    Arm64MaterializationError, Arm64ProgramBuilder,
};

pub(crate) fn define(
    targets: &Arm64AsyncPrimitiveTargets,
    builder: &mut Arm64ProgramBuilder,
) -> Result<(), Arm64LoweringError> {
    define_interest_primitives(targets, builder)?;
    define_pair_primitives(targets, builder)?;
    define_group_primitive(targets, builder)
}

fn define_interest_primitives(
    targets: &Arm64AsyncPrimitiveTargets,
    builder: &mut Arm64ProgramBuilder,
) -> Result<(), Arm64LoweringError> {
    if let Some(lifecycle) = targets.single_interest_lifecycle() {
        if let Some(constructor) = targets.descriptor_readiness() {
            builder.define_function(
                constructor,
                crate::async_interest_code::materialize_descriptor_constructor(lifecycle)
                    .map_err(Arm64MaterializationError::Code)?,
            )?;
        }
        if let Some(constructor) = targets.monotonic_deadline() {
            builder.define_function(
                constructor,
                crate::async_interest_code::materialize_deadline_constructor(lifecycle)
                    .map_err(Arm64MaterializationError::Code)?,
            )?;
        }
        if let Some(constructor) = targets.process_completion() {
            builder.define_function(
                constructor,
                crate::async_interest_code::materialize_process_constructor(lifecycle)
                    .map_err(Arm64MaterializationError::Code)?,
            )?;
        }
        define_interest_lifecycle(lifecycle, builder)?;
    }
    if let (Some(constructor), Some(lifecycle)) = (
        targets.descriptor_readiness_or_deadline(),
        targets.dual_interest_lifecycle(),
    ) {
        builder.define_function(
            constructor,
            crate::async_interest_code::materialize_descriptor_or_deadline_constructor(lifecycle)
                .map_err(Arm64MaterializationError::Code)?,
        )?;
        define_interest_lifecycle(lifecycle, builder)?;
    }
    Ok(())
}

fn define_pair_primitives(
    targets: &Arm64AsyncPrimitiveTargets,
    builder: &mut Arm64ProgramBuilder,
) -> Result<(), Arm64LoweringError> {
    if let Some(join) = targets.task_join() {
        builder.define_function(
            join.constructor(),
            crate::async_pair_constructor_code::materialize(join)
                .map_err(Arm64MaterializationError::Code)?,
        )?;
        builder.define_function(
            join.resume(),
            crate::async_pair_code::materialize_join_resume()
                .map_err(Arm64MaterializationError::Code)?,
        )?;
        builder.define_function(
            join.cancel(),
            crate::async_pair_code::materialize_join_cancel()
                .map_err(Arm64MaterializationError::Code)?,
        )?;
        builder.define_function(
            join.consume(),
            crate::async_pair_code::materialize_join_consume()
                .map_err(Arm64MaterializationError::Code)?,
        )?;
    }
    if let Some(race) = targets.task_race() {
        builder.define_function(
            race.constructor(),
            crate::async_pair_constructor_code::materialize(race)
                .map_err(Arm64MaterializationError::Code)?,
        )?;
        builder.define_function(
            race.resume(),
            crate::async_pair_code::materialize_race_resume()
                .map_err(Arm64MaterializationError::Code)?,
        )?;
        builder.define_function(
            race.cancel(),
            crate::async_pair_code::materialize_race_cancel()
                .map_err(Arm64MaterializationError::Code)?,
        )?;
        builder.define_function(
            race.consume(),
            crate::async_pair_code::materialize_race_consume()
                .map_err(Arm64MaterializationError::Code)?,
        )?;
    }
    Ok(())
}

fn define_group_primitive(
    targets: &Arm64AsyncPrimitiveTargets,
    builder: &mut Arm64ProgramBuilder,
) -> Result<(), Arm64LoweringError> {
    let Some(group) = targets.task_group_ready() else {
        return Ok(());
    };
    builder.define_function(
        group.constructor(),
        crate::async_group_constructor_code::materialize(group)
            .map_err(Arm64MaterializationError::Code)?,
    )?;
    builder.define_function(
        group.resume(),
        crate::async_group_code::materialize_resume().map_err(Arm64MaterializationError::Code)?,
    )?;
    builder.define_function(
        group.cancel(),
        crate::async_group_code::materialize_cancel().map_err(Arm64MaterializationError::Code)?,
    )?;
    builder.define_function(
        group.consume(),
        crate::async_group_code::materialize_consume().map_err(Arm64MaterializationError::Code)?,
    )?;
    Ok(())
}

fn define_interest_lifecycle(
    lifecycle: Arm64AsyncInterestLifecycleTargets,
    builder: &mut Arm64ProgramBuilder,
) -> Result<(), Arm64LoweringError> {
    builder.define_function(
        lifecycle.resume(),
        crate::async_interest_code::materialize_resume(lifecycle.interest_count())
            .map_err(Arm64MaterializationError::Code)?,
    )?;
    builder.define_function(
        lifecycle.cancel(),
        crate::async_interest_code::materialize_cancel(lifecycle.interest_count())
            .map_err(Arm64MaterializationError::Code)?,
    )?;
    builder.define_function(
        lifecycle.consume(),
        crate::async_interest_code::materialize_consume(lifecycle.interest_count())
            .map_err(Arm64MaterializationError::Code)?,
    )?;
    Ok(())
}
