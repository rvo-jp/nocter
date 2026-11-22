use std::fmt;
use std::path::Path;

use nocter_diagnostics::DiagnosticCode;
use nocter_native_session::{
    CompiledNativeImageSet, NativeImageEntry, NativeImageSetCompileRequest, NativeImageSetError,
    NativeSessionError, compile_native_image, compile_native_images,
};
use nocter_session::{ExecutableCompileRequest, ExecutableIdentity};

use crate::SelectedBuildOutput;
use crate::{
    ArtifactError, BuildOutputPlan, OutputPlanError, PersistentArtifact, persist_native_image,
};

/// One persistent executable.
#[derive(Debug)]
pub struct BuiltExecutable {
    artifact: PersistentArtifact,
}

/// One persistent artifact paired with its exact selected executable identity.
#[derive(Debug)]
pub struct BuiltExecutableEntry {
    identity: ExecutableIdentity,
    artifact: PersistentArtifact,
}

impl BuiltExecutableEntry {
    #[must_use]
    pub const fn identity(&self) -> &ExecutableIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn artifact(&self) -> &PersistentArtifact {
        &self.artifact
    }
}

/// Persistent outputs for every executable owned by the command-root package set.
#[derive(Debug)]
pub struct BuiltExecutableSet {
    entries: Box<[BuiltExecutableEntry]>,
}

impl BuiltExecutableSet {
    #[must_use]
    pub const fn entries(&self) -> &[BuiltExecutableEntry] {
        &self.entries
    }
}

impl BuiltExecutable {
    #[must_use]
    pub const fn artifact(&self) -> &PersistentArtifact {
        &self.artifact
    }
}

/// Compiles and atomically commits one selected executable.
///
/// # Errors
///
/// Returns either the exact native lowering failure or the exact artifact operation that failed.
pub fn build_executable(
    request: ExecutableCompileRequest,
    output: impl AsRef<Path>,
) -> Result<BuiltExecutable, BuildCommandError> {
    build_selected_executable(
        request,
        SelectedBuildOutput::Exact(output.as_ref().to_path_buf()),
    )
}

/// Compiles and atomically commits one selected executable through a closed output policy.
///
/// # Errors
///
/// Returns the exact native lowering, default-name planning, or artifact failure.
pub fn build_selected_executable(
    request: ExecutableCompileRequest,
    output: SelectedBuildOutput,
) -> Result<BuiltExecutable, BuildCommandError> {
    persist_selected_executable(&compile_native_image(request)?, output)
}

pub(crate) fn persist_selected_executable(
    compiled: &NativeImageEntry,
    output: SelectedBuildOutput,
) -> Result<BuiltExecutable, BuildCommandError> {
    let output = match output {
        SelectedBuildOutput::Exact(path) => path,
        SelectedBuildOutput::TargetNameIn(directory) => {
            BuildOutputPlan::for_selected(compiled.identity(), directory)?
                .path()
                .to_path_buf()
        }
    };
    let artifact = persist_native_image(compiled.image().bytes(), output)?;
    Ok(BuiltExecutable { artifact })
}

/// Compiles and publishes every root-package executable below the selected package root.
///
/// Output planning completes before the first filesystem mutation. Each individual image is
/// committed failure-atomically; if a later commit fails, any earlier path contains a complete
/// executable rather than partial bytes.
///
/// # Errors
///
/// Returns the exact compile-set, output-plan, or target-specific artifact failure.
pub fn build_executables(
    request: NativeImageSetCompileRequest,
    package_root: impl AsRef<Path>,
) -> Result<BuiltExecutableSet, BuildSetCommandError> {
    persist_executables(compile_native_images(request)?, package_root)
}

pub(crate) fn persist_executables(
    compiled: CompiledNativeImageSet,
    package_root: impl AsRef<Path>,
) -> Result<BuiltExecutableSet, BuildSetCommandError> {
    let plan = BuildOutputPlan::for_package(compiled.entries(), package_root)?;
    let images = compiled.into_entries();
    let mut entries = Vec::with_capacity(images.len());
    for (image, output) in images.into_vec().into_iter().zip(plan.outputs()) {
        debug_assert_eq!(image.identity(), output.identity());
        let (identity, image) = image.into_parts();
        let artifact = persist_native_image(image.bytes(), output.path()).map_err(|error| {
            BuildSetCommandError::Artifact {
                executable: identity.clone(),
                error,
            }
        })?;
        entries.push(BuiltExecutableEntry { identity, artifact });
    }
    Ok(BuiltExecutableSet {
        entries: entries.into_boxed_slice(),
    })
}

#[derive(Debug)]
pub enum BuildSetCommandError {
    Compile(NativeImageSetError),
    Plan(OutputPlanError),
    Artifact {
        executable: ExecutableIdentity,
        error: ArtifactError,
    },
}

impl BuildSetCommandError {
    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<DiagnosticCode> {
        match self {
            Self::Compile(error) => error.diagnostic_code(),
            Self::Plan(_) => Some(DiagnosticCode::E0800),
            Self::Artifact { .. } => Some(DiagnosticCode::E0702),
        }
    }
}

impl fmt::Display for BuildSetCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(error) => write!(formatter, "native compilation failed: {error}"),
            Self::Plan(error) => write!(formatter, "output planning failed: {error}"),
            Self::Artifact { executable, error } => write!(
                formatter,
                "publication of {} ({}) failed: {error}",
                executable.name(),
                executable.package().as_str()
            ),
        }
    }
}

impl std::error::Error for BuildSetCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compile(error) => Some(error),
            Self::Plan(error) => Some(error),
            Self::Artifact { error, .. } => Some(error),
        }
    }
}

impl From<NativeImageSetError> for BuildSetCommandError {
    fn from(error: NativeImageSetError) -> Self {
        Self::Compile(error)
    }
}

impl From<OutputPlanError> for BuildSetCommandError {
    fn from(error: OutputPlanError) -> Self {
        Self::Plan(error)
    }
}

#[derive(Debug)]
pub enum BuildCommandError {
    Compile(NativeSessionError),
    Plan(OutputPlanError),
    Artifact(ArtifactError),
}

impl BuildCommandError {
    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<DiagnosticCode> {
        match self {
            Self::Compile(error) => error.diagnostic_code(),
            Self::Plan(_) => Some(DiagnosticCode::E0800),
            Self::Artifact(_) => Some(DiagnosticCode::E0702),
        }
    }
}

impl fmt::Display for BuildCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(error) => write!(formatter, "native compilation failed: {error}"),
            Self::Plan(error) => write!(formatter, "output planning failed: {error}"),
            Self::Artifact(error) => write!(formatter, "executable publication failed: {error}"),
        }
    }
}

impl std::error::Error for BuildCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compile(error) => Some(error),
            Self::Plan(error) => Some(error),
            Self::Artifact(error) => Some(error),
        }
    }
}

impl From<NativeSessionError> for BuildCommandError {
    fn from(error: NativeSessionError) -> Self {
        Self::Compile(error)
    }
}

impl From<ArtifactError> for BuildCommandError {
    fn from(error: ArtifactError) -> Self {
        Self::Artifact(error)
    }
}

impl From<OutputPlanError> for BuildCommandError {
    fn from(error: OutputPlanError) -> Self {
        Self::Plan(error)
    }
}
