use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nocter_source::{SourceError, SourceName};
use nocter_source_tooling::{InspectionGoal, SourceInspection};

use crate::{ProgramInputError, ProgramInputOptions, ResolvedProgramInput, resolve_program_input};

/// One exact standalone source and the immutable syntax snapshot derived from its original bytes.
pub(super) struct StandaloneSource {
    path: PathBuf,
    bytes: Vec<u8>,
    inspection: SourceInspection,
}

impl StandaloneSource {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(super) const fn inspection(&self) -> &SourceInspection {
        &self.inspection
    }
}

/// Resolves, reads, normalizes, and parses exactly one explicitly named `.nct` source.
///
/// This is the only command-layer authority that selects the standalone parse goal. It performs
/// no installation, package, target, import, or semantic work.
pub(super) fn load_standalone_source(
    source: PathBuf,
    current_directory: impl AsRef<Path>,
) -> Result<StandaloneSource, StandaloneSourceError> {
    let input = resolve_program_input(
        current_directory,
        ProgramInputOptions::positional_file(source),
    )
    .map_err(StandaloneSourceError::Input)?;
    let ResolvedProgramInput::SingleFile(input) = input else {
        unreachable!("an explicit positional source always resolves in single-file mode")
    };
    let path = input.source().to_path_buf();
    let bytes = fs::read(&path).map_err(|source| StandaloneSourceError::Read {
        path: path.clone(),
        source,
    })?;
    let name = path
        .to_str()
        .ok_or_else(|| StandaloneSourceError::NonUnicodePath(path.clone()))?;
    let goal = if path.file_name().is_some_and(|name| name == "nocter.nct") {
        InspectionGoal::PackageFile
    } else {
        InspectionGoal::SourceFile
    };
    let inspection =
        SourceInspection::new(SourceName::new(name), &bytes, goal).map_err(|source| {
            StandaloneSourceError::Source {
                path: path.clone(),
                source,
            }
        })?;
    Ok(StandaloneSource {
        path,
        bytes,
        inspection,
    })
}

#[derive(Debug)]
pub enum StandaloneSourceError {
    Input(ProgramInputError),
    Read { path: PathBuf, source: io::Error },
    NonUnicodePath(PathBuf),
    Source { path: PathBuf, source: SourceError },
}

impl fmt::Display for StandaloneSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => error.fmt(formatter),
            Self::Read { path, source } => {
                write!(formatter, "cannot read {}: {source}", path.display())
            }
            Self::NonUnicodePath(path) => write!(
                formatter,
                "source path {} cannot be represented as Unicode",
                path.display()
            ),
            Self::Source { path, source } => {
                write!(formatter, "cannot normalize {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for StandaloneSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            Self::Read { source, .. } => Some(source),
            Self::Source { source, .. } => Some(source),
            Self::NonUnicodePath(_) => None,
        }
    }
}
