use std::fmt;
use std::path::Path;

use nocter_session::{
    ExecutableCompileRequest, ExecutableIdentity, NativeImageSetCompileRequest,
    NativeImageSetError, NativeSessionError, compile_native_image, compile_native_images,
};
use nocter_source_index::SourceIndex;

use crate::{
    ArtifactError, BuildOutputPlan, OutputPlanError, PersistentArtifact, persist_native_image,
};

/// One persistent executable and the independent source projection from its compile session.
#[derive(Debug)]
pub struct BuiltExecutable {
    artifact: PersistentArtifact,
    source_index: SourceIndex,
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
    source_index: SourceIndex,
}

impl BuiltExecutableSet {
    #[must_use]
    pub const fn entries(&self) -> &[BuiltExecutableEntry] {
        &self.entries
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }
}

impl BuiltExecutable {
    #[must_use]
    pub const fn artifact(&self) -> &PersistentArtifact {
        &self.artifact
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(self) -> (PersistentArtifact, SourceIndex) {
        (self.artifact, self.source_index)
    }
}

/// Compiles and atomically commits one selected executable.
///
/// # Errors
///
/// Returns either the exact compiler-session failure or the exact artifact operation that failed.
pub fn build_executable(
    request: ExecutableCompileRequest<'_>,
    output: impl AsRef<Path>,
) -> Result<BuiltExecutable, BuildCommandError> {
    let compiled = compile_native_image(request)?;
    let (image, source_index) = compiled.into_parts();
    let artifact = persist_native_image(&image, output)?;
    Ok(BuiltExecutable {
        artifact,
        source_index,
    })
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
    request: NativeImageSetCompileRequest<'_>,
    package_root: impl AsRef<Path>,
) -> Result<BuiltExecutableSet, BuildSetCommandError> {
    let compiled = compile_native_images(request)?;
    let plan = BuildOutputPlan::for_package(compiled.entries(), package_root)?;
    let (images, source_index) = compiled.into_parts();
    let mut entries = Vec::with_capacity(images.len());
    for (image, output) in images.into_vec().into_iter().zip(plan.outputs()) {
        debug_assert_eq!(image.identity(), output.identity());
        let (identity, image) = image.into_parts();
        let artifact = persist_native_image(&image, output.path()).map_err(|error| {
            BuildSetCommandError::Artifact {
                executable: identity.clone(),
                error,
            }
        })?;
        entries.push(BuiltExecutableEntry { identity, artifact });
    }
    Ok(BuiltExecutableSet {
        entries: entries.into_boxed_slice(),
        source_index,
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
    Artifact(ArtifactError),
}

impl fmt::Display for BuildCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(error) => write!(formatter, "native compilation failed: {error}"),
            Self::Artifact(error) => write!(formatter, "executable publication failed: {error}"),
        }
    }
}

impl std::error::Error for BuildCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compile(error) => Some(error),
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
