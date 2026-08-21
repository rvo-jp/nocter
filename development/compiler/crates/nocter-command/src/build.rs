use std::fmt;
use std::path::Path;

use nocter_session::{ExecutableCompileRequest, NativeSessionError, compile_native_image};
use nocter_source_index::SourceIndex;

use crate::{ArtifactError, PersistentArtifact, persist_native_image};

/// One persistent executable and the independent source projection from its compile session.
#[derive(Debug)]
pub struct BuiltExecutable {
    artifact: PersistentArtifact,
    source_index: SourceIndex,
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
