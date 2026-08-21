use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nocter_diagnostics::DiagnosticRenderError;
use nocter_source::{SourceError, SourceName};
use nocter_source_tooling::{InspectionGoal, SourceInspection};

use crate::{
    ParsedSourceInspectionCommand, ProgramInputError, ProgramInputOptions, ResolvedProgramInput,
    SourceInspectionKind, resolve_program_input,
};

/// Complete versioned output of one standalone source inspection.
#[derive(Debug)]
pub struct SourceInspectionCommandResult {
    json: String,
    succeeded: bool,
}

impl SourceInspectionCommandResult {
    #[must_use]
    pub fn json(&self) -> &str {
        &self.json
    }

    #[must_use]
    pub const fn succeeded(&self) -> bool {
        self.succeeded
    }
}

/// Reads and inspects exactly one `.nct` file without installation, package, or target state.
///
/// # Errors
///
/// Returns exact path selection, filesystem, normalization, or rendering failure.
pub fn execute_source_inspection(
    command: ParsedSourceInspectionCommand,
    current_directory: impl AsRef<Path>,
) -> Result<SourceInspectionCommandResult, SourceInspectionCommandError> {
    let (source, kind) = command.into_parts();
    let input = resolve_program_input(
        current_directory,
        ProgramInputOptions::positional_file(source),
    )
    .map_err(SourceInspectionCommandError::Input)?;
    let ResolvedProgramInput::SingleFile(input) = input else {
        unreachable!("an explicit positional source always resolves in single-file mode")
    };
    let path = input.source();
    let bytes = fs::read(path).map_err(|source| SourceInspectionCommandError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let name = path
        .to_str()
        .ok_or_else(|| SourceInspectionCommandError::NonUnicodePath(path.to_path_buf()))?;
    let goal = if path.file_name().is_some_and(|name| name == "nocter.nct") {
        InspectionGoal::PackageFile
    } else {
        InspectionGoal::ModuleSource
    };
    let inspection =
        SourceInspection::new(SourceName::new(name), &bytes, goal).map_err(|source| {
            SourceInspectionCommandError::Source {
                path: path.to_path_buf(),
                source,
            }
        })?;
    let (json, succeeded) = match kind {
        SourceInspectionKind::Tokens => (
            inspection.render_tokens_json(),
            inspection.tokens_succeeded(),
        ),
        SourceInspectionKind::Ast => (inspection.render_ast_json(), inspection.ast_succeeded()),
    };
    Ok(SourceInspectionCommandResult {
        json: json.map_err(SourceInspectionCommandError::Render)?,
        succeeded,
    })
}

#[derive(Debug)]
pub enum SourceInspectionCommandError {
    Input(ProgramInputError),
    Read { path: PathBuf, source: io::Error },
    NonUnicodePath(PathBuf),
    Source { path: PathBuf, source: SourceError },
    Render(DiagnosticRenderError),
}

impl SourceInspectionCommandError {
    #[must_use]
    pub const fn is_user_failure(&self) -> bool {
        !matches!(self, Self::Render(_))
    }
}

impl fmt::Display for SourceInspectionCommandError {
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
            Self::Render(error) => write!(formatter, "cannot render source inspection: {error}"),
        }
    }
}

impl std::error::Error for SourceInspectionCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            Self::Read { source, .. } => Some(source),
            Self::Render(error) => Some(error),
            Self::Source { source, .. } => Some(source),
            Self::NonUnicodePath(_) => None,
        }
    }
}
