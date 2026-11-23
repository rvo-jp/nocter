//! Lowers selected v2 executables through the common native backend.
use std::{fmt, sync::Arc};

use nocter_diagnostics::SourceDiagnostic;
use nocter_session::{
    AnalyzedUnit, ExecutableSelector, ValueAccessExecutable, select_all_value_access_executables,
    select_value_access_executable,
};
use nocter_target_program::value_access::{self, InstanceProgram, lifecycle};

use crate::{
    CompiledNativeImageSet, NativeImage, NativeImageEntry, NativeImageError, NativeImageSetError,
    NativeSessionError, native::image_from_machine,
};

#[derive(Debug)]
pub enum ValueAccessLoweringError {
    Instances(value_access::Failure),
    Lifecycle(Box<lifecycle::Failure>),
    Destruction(Box<[SourceDiagnostic]>),
    Mir(nocter_mir::value_access::Failure),
    Machine(nocter_machine::value_access::ExecutableFailure),
}

impl fmt::Display for ValueAccessLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Instances(error) => {
                write!(formatter, "cannot specialize v2 executable: {error:?}")
            }
            Self::Lifecycle(error) => write!(formatter, "cannot plan v2 cleanup: {error:?}"),
            Self::Destruction(_) => formatter.write_str("v2 destructor effect checking failed"),
            Self::Mir(error) => write!(formatter, "v2 MIR lowering failed: {error:?}"),
            Self::Machine(error) => write!(formatter, "v2 machine lowering failed: {error:?}"),
        }
    }
}

impl std::error::Error for ValueAccessLoweringError {}

impl ValueAccessLoweringError {
    pub(super) fn source_diagnostics(&self) -> &[SourceDiagnostic] {
        match self {
            Self::Destruction(diagnostics) => diagnostics,
            _ => &[],
        }
    }
}

/// Selects and lowers one executable from a checked v2 source session.
///
/// # Errors
/// Preserves entry selection and native lowering failures.
pub fn compile_value_access_image(
    analysis: &AnalyzedUnit,
    selector: &ExecutableSelector,
) -> Result<NativeImageEntry, NativeSessionError> {
    let executable = select_value_access_executable(analysis, selector)
        .map_err(NativeSessionError::ValueAccessSelection)?;
    let identity = executable.identity();
    let image = lower_image(analysis, executable).map_err(NativeSessionError::Image)?;
    Ok(NativeImageEntry::new(identity, image))
}

/// Lowers every root-package executable from the same checked v2 package session.
///
/// # Errors
/// Preserves entry selection and lowering failures, with no partial image set.
pub fn compile_value_access_images(
    analysis: &AnalyzedUnit,
) -> Result<CompiledNativeImageSet, NativeImageSetError> {
    let executables = select_all_value_access_executables(analysis)
        .map_err(NativeImageSetError::ValueAccessSelection)?;
    if executables.is_empty() {
        return Err(NativeImageSetError::NoExecutable);
    }
    let entries = executables
        .into_iter()
        .map(|executable| {
            let identity = executable.identity();
            let image =
                lower_image(analysis, executable).map_err(|error| NativeImageSetError::Image {
                    executable: identity.clone(),
                    error,
                })?;
            Ok(NativeImageEntry::new(identity, image))
        })
        .collect::<Result<_, _>>()?;
    Ok(CompiledNativeImageSet::new(entries))
}

fn lower_image(
    analysis: &AnalyzedUnit,
    executable: ValueAccessExecutable<'_>,
) -> Result<NativeImage, NativeImageError> {
    let target = executable.target();
    let (unit, root) = executable.into_entry();
    lower_root_image(analysis, target, || InstanceProgram::prepare(unit, root))
}

pub(super) fn lower_root_image(
    analysis: &AnalyzedUnit,
    target: nocter_model::CompilationTarget,
    prepare: impl FnOnce() -> Result<InstanceProgram, value_access::Failure>,
) -> Result<NativeImage, NativeImageError> {
    let abi = value_access::executable_abi(target).map_err(NativeImageError::Target)?;
    let instances = prepare().map_err(ValueAccessLoweringError::Instances)?;
    let lifecycle = lifecycle::Program::prepare(Arc::new(instances)).map_err(|error| {
        if let lifecycle::Failure::Destructor(failure) = &error
            && let Some(diagnostics) = analysis.value_access_destructor_diagnostics(failure)
        {
            return ValueAccessLoweringError::Destruction(diagnostics);
        }
        ValueAccessLoweringError::Lifecycle(Box::new(error))
    })?;
    let mir = nocter_mir::value_access::Program::lower(&lifecycle, abi)
        .map_err(ValueAccessLoweringError::Mir)?;
    let instances = lifecycle.instance_program();
    let machine = nocter_machine::value_access::lower(&mir).map_err(|error| {
        if let nocter_machine::value_access::ExecutableFailure::DestructorBacking(id) = &error
            && let Some(instance) = instances.instances().get(*id)
            && let Some(failure) = instances.unit().destructor_backing_failure(instance.key())
            && let Some(diagnostics) = analysis.value_access_destructor_diagnostics(&failure)
        {
            return ValueAccessLoweringError::Destruction(diagnostics);
        }
        ValueAccessLoweringError::Machine(error)
    })?;
    image_from_machine(&machine)
}
