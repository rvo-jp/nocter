use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nocter_diagnostics::DiagnosticCode;
use nocter_source::SourceMap;

use crate::ParsedFormatCommand;
use crate::standalone_source::{StandaloneSourceError, load_standalone_source};

static NEXT_FORMAT_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormatCommandResult {
    Unchanged,
    Rewritten,
}

/// Formats exactly one `.nct` source without installation, package, or target state.
///
/// Publication uses a same-directory temporary file and one final rename, so a failed formatter
/// or write never truncates the authored source.
///
/// # Errors
///
/// Returns the exact input, read, normalization, syntax, check-difference, or publication failure.
pub fn execute_format(
    command: ParsedFormatCommand,
    current_directory: impl AsRef<Path>,
) -> Result<FormatCommandResult, FormatCommandError> {
    let (source, check) = command.into_parts();
    let source =
        load_standalone_source(source, current_directory).map_err(FormatCommandError::Source)?;
    let path = source.path();
    let bytes = source.bytes();
    let inspection = source.inspection();
    let formatted = match inspection.format() {
        Ok(formatted) => formatted,
        Err(nocter_source_tooling::FormatError::Diagnostics(diagnostics)) => {
            return Err(FormatCommandError::Diagnostics {
                diagnostics,
                sources: inspection.sources().clone(),
            });
        }
        Err(nocter_source_tooling::FormatError::ChangedSyntax) => {
            return Err(FormatCommandError::ChangedSyntax);
        }
    };
    if formatted.as_bytes() == bytes {
        return Ok(FormatCommandResult::Unchanged);
    }
    if check {
        return Err(FormatCommandError::WouldChange(path.to_path_buf()));
    }
    publish(path, formatted.as_bytes())?;
    Ok(FormatCommandResult::Rewritten)
}

fn publish(path: &Path, contents: &[u8]) -> Result<(), FormatCommandError> {
    let parent = path
        .parent()
        .ok_or_else(|| FormatCommandError::InvalidPath(path.to_path_buf()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| FormatCommandError::NonUnicodePath(path.to_path_buf()))?;
    let permissions = fs::metadata(path)
        .map_err(|source| publication_error("read source metadata", path, source))?
        .permissions();
    let serial = NEXT_FORMAT_FILE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{name}.nocter-fmt-{}-{serial}.tmp",
        std::process::id()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|source| {
                publication_error("create formatting temporary", &temporary, source)
            })?;
        file.write_all(contents).map_err(|source| {
            publication_error("write formatting temporary", &temporary, source)
        })?;
        file.set_permissions(permissions).map_err(|source| {
            publication_error("preserve source permissions", &temporary, source)
        })?;
        file.sync_all()
            .map_err(|source| publication_error("sync formatting temporary", &temporary, source))?;
        drop(file);
        fs::rename(&temporary, path)
            .map_err(|source| publication_error("replace formatted source", path, source))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn publication_error(
    operation: &'static str,
    path: &Path,
    source: io::Error,
) -> FormatCommandError {
    FormatCommandError::Publication {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[derive(Debug)]
pub enum FormatCommandError {
    Source(StandaloneSourceError),
    NonUnicodePath(PathBuf),
    InvalidPath(PathBuf),
    Diagnostics {
        diagnostics: Box<[nocter_diagnostics::SourceDiagnostic]>,
        sources: SourceMap,
    },
    WouldChange(PathBuf),
    Publication {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    ChangedSyntax,
}

impl FormatCommandError {
    #[must_use]
    pub fn source_diagnostics(
        &self,
    ) -> Option<(&[nocter_diagnostics::SourceDiagnostic], &SourceMap)> {
        match self {
            Self::Diagnostics {
                diagnostics,
                sources,
            } => Some((diagnostics, sources)),
            _ => None,
        }
    }

    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<DiagnosticCode> {
        match self {
            Self::WouldChange(_) => Some(DiagnosticCode::E0602),
            Self::Source(_)
            | Self::NonUnicodePath(_)
            | Self::InvalidPath(_)
            | Self::Publication { .. } => Some(DiagnosticCode::E0702),
            Self::Diagnostics { .. } | Self::ChangedSyntax => None,
        }
    }

    #[must_use]
    pub const fn is_source_failure(&self) -> bool {
        matches!(self, Self::Diagnostics { .. } | Self::WouldChange(_))
    }

    #[must_use]
    pub const fn is_user_failure(&self) -> bool {
        !matches!(self, Self::ChangedSyntax)
    }
}

impl fmt::Display for FormatCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::NonUnicodePath(path) => write!(
                formatter,
                "source path {} cannot be represented as Unicode",
                path.display()
            ),
            Self::InvalidPath(path) => {
                write!(
                    formatter,
                    "source path {} has no parent directory",
                    path.display()
                )
            }
            Self::Diagnostics { .. } => formatter.write_str("source cannot be formatted"),
            Self::WouldChange(path) => {
                write!(formatter, "{} is not formatted", path.display())
            }
            Self::Publication {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
            Self::ChangedSyntax => formatter.write_str("formatter output changed concrete syntax"),
        }
    }
}

impl std::error::Error for FormatCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Publication { source, .. } => Some(source),
            Self::NonUnicodePath(_)
            | Self::InvalidPath(_)
            | Self::Diagnostics { .. }
            | Self::WouldChange(_)
            | Self::ChangedSyntax => None,
        }
    }
}
