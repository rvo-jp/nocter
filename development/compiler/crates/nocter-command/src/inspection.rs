use std::fmt;
use std::path::Path;

use nocter_diagnostics::DiagnosticRenderError;

use crate::standalone_source::{StandaloneSourceError, load_standalone_source};
use crate::{ParsedSourceInspectionCommand, SourceInspectionKind};

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
    let source = load_standalone_source(source, current_directory)
        .map_err(SourceInspectionCommandError::Source)?;
    let inspection = source.inspection();
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
    Source(StandaloneSourceError),
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
            Self::Source(error) => error.fmt(formatter),
            Self::Render(error) => write!(formatter, "cannot render source inspection: {error}"),
        }
    }
}

impl std::error::Error for SourceInspectionCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Render(error) => Some(error),
        }
    }
}
