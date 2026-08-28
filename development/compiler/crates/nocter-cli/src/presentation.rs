use std::path::PathBuf;

use nocter_command::{CommandArgumentFailure, DiagnosticFormat};
use nocter_diagnostics::{DiagnosticJsonContext, DiagnosticRenderError};

use crate::dispatch::InstalledCommand;

/// Presentation facts that become available progressively during one check invocation.
#[derive(Clone, Debug)]
pub(crate) struct InvocationDiagnosticPresentation {
    pub(crate) command: &'static str,
    pub(crate) format: DiagnosticFormat,
    pub(crate) target: Option<&'static str>,
    pub(crate) root: Option<PathBuf>,
    pub(crate) root_absolute_path: Option<PathBuf>,
}

impl InvocationDiagnosticPresentation {
    pub(crate) fn from_argument_failure(failure: &CommandArgumentFailure) -> Option<Self> {
        let command = failure.command()?;
        matches!(command, "check" | "test").then_some(Self {
            command,
            format: failure.format(),
            target: None,
            root: failure.root_hint().map(PathBuf::from),
            root_absolute_path: None,
        })
    }

    pub(crate) fn from_command(command: &InstalledCommand) -> Option<Self> {
        match command {
            InstalledCommand::Check(command) => Some(Self {
                command: "check",
                format: command.format(),
                target: None,
                root: command.root_hint(),
                root_absolute_path: None,
            }),
            InstalledCommand::Test(command) => Some(Self {
                command: "test",
                format: command.format(),
                target: None,
                root: Some(command.root_hint()),
                root_absolute_path: None,
            }),
            InstalledCommand::Version
            | InstalledCommand::Doctor
            | InstalledCommand::Graph(_)
            | InstalledCommand::Fetch(_)
            | InstalledCommand::Build(_)
            | InstalledCommand::Run(_)
            | InstalledCommand::Lsp => None,
        }
    }

    pub(crate) fn json_context(&self) -> Result<DiagnosticJsonContext<'_>, DiagnosticRenderError> {
        let root = self
            .root
            .as_deref()
            .map(|path| path.to_str().ok_or(DiagnosticRenderError::NonUnicodePath))
            .transpose()?;
        let root_absolute_path = self
            .root_absolute_path
            .as_deref()
            .map(|path| path.to_str().ok_or(DiagnosticRenderError::NonUnicodePath))
            .transpose()?;
        Ok(DiagnosticJsonContext::new(
            self.command,
            self.target,
            root,
            root_absolute_path,
        ))
    }
}
