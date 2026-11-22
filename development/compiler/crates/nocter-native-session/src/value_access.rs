//! Lowers a selected v2 package executable through the common native backend.
use std::{fmt, sync::Arc};

use nocter_target_program::{
    TargetUnavailable,
    value_access::{self, InstanceProgram, lifecycle},
};

use crate::{NativeImage, NativeImageError, native::image_from_machine};

#[derive(Debug)]
pub enum ValueAccessImageError {
    Target(TargetUnavailable),
    Instances(value_access::Failure),
    Lifecycle(lifecycle::Failure),
    Mir(nocter_mir::value_access::Failure),
    Machine(nocter_machine::value_access::ExecutableFailure),
    Image(NativeImageError),
}

impl fmt::Display for ValueAccessImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target(error) => error.fmt(formatter),
            Self::Instances(error) => {
                write!(formatter, "cannot specialize v2 executable: {error:?}")
            }
            Self::Lifecycle(error) => write!(formatter, "cannot plan v2 cleanup: {error:?}"),
            Self::Mir(error) => write!(formatter, "v2 MIR lowering failed: {error:?}"),
            Self::Machine(error) => write!(formatter, "v2 machine lowering failed: {error:?}"),
            Self::Image(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ValueAccessImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Target(error) => Some(error),
            Self::Image(error) => Some(error),
            _ => None,
        }
    }
}

/// Builds an image from the entry selected by the v2 package session.
///
/// # Errors
/// Rejects unimplemented targets and preserves lowering failures from each stage.
pub fn compile_value_access_image(
    executable: nocter_session::ValueAccessExecutable<'_>,
) -> Result<NativeImage, ValueAccessImageError> {
    let abi =
        value_access::executable_abi(executable.target()).map_err(ValueAccessImageError::Target)?;
    let (unit, root) = executable.into_entry();
    let instances =
        InstanceProgram::prepare(unit, root).map_err(ValueAccessImageError::Instances)?;
    let lifecycle = lifecycle::Program::prepare(Arc::new(instances))
        .map_err(ValueAccessImageError::Lifecycle)?;
    let mir = nocter_mir::value_access::Program::lower(&lifecycle, abi)
        .map_err(ValueAccessImageError::Mir)?;
    let machine =
        nocter_machine::value_access::lower(&mir).map_err(ValueAccessImageError::Machine)?;
    image_from_machine(&machine).map_err(ValueAccessImageError::Image)
}
