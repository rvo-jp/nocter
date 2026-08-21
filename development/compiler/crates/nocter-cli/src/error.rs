use std::fmt;

use nocter_command::{
    BuildCommandExecutionError, CheckCommandExecutionError, CommandArgumentFailure,
    DiagnosticFormat, FetchCommandExecutionError, FormatCommandError, PreparedCommandError,
    ProgramInputError, RunCommandExecutionError, SourceInspectionCommandError,
    TestCommandExecutionError,
};
use nocter_diagnostics::{
    DiagnosticRenderError, SpanlessDiagnostic, render_source_diagnostic,
    render_source_diagnostics_json, render_spanless_diagnostic_json,
};
use nocter_installation::{InstallationCompatibilityError, NocterHomeError};
use nocter_package_acquisition::PackageAcquisitionError;

use crate::presentation::InvocationDiagnosticPresentation;

#[derive(Debug)]
pub struct InvocationError {
    kind: Box<InvocationErrorKind>,
    presentation: Option<InvocationDiagnosticPresentation>,
}

#[derive(Debug)]
pub enum InvocationErrorKind {
    Arguments(CommandArgumentFailure),
    Installation(NocterHomeError),
    InstallationCompatibility(InstallationCompatibilityError),
    AcquisitionInitialization(PackageAcquisitionError),
    Preparation(PreparedCommandError),
    Fetch(Box<FetchCommandExecutionError>),
    Check(Box<CheckCommandExecutionError>),
    Build(Box<BuildCommandExecutionError>),
    Run(Box<RunCommandExecutionError>),
    Test(Box<TestCommandExecutionError>),
    SourceInspection(SourceInspectionCommandError),
    Format(FormatCommandError),
}

/// Process-level failure class independent from diagnostic presentation and error code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvocationFailureClass {
    Source,
    User,
    Internal,
}

impl InvocationFailureClass {
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Source => 1,
            Self::User => 2,
            Self::Internal => 3,
        }
    }
}

impl InvocationError {
    pub(crate) fn new(
        kind: InvocationErrorKind,
        presentation: Option<InvocationDiagnosticPresentation>,
    ) -> Self {
        Self {
            kind: Box::new(kind),
            presentation,
        }
    }

    #[must_use]
    pub fn kind(&self) -> &InvocationErrorKind {
        self.kind.as_ref()
    }

    /// Returns the public code selected independently from process status.
    #[must_use]
    pub fn diagnostic_code(&self) -> Option<&'static str> {
        self.user_diagnostic_code().or_else(|| {
            (self.failure_class() == InvocationFailureClass::Internal).then_some("E0900")
        })
    }

    fn user_diagnostic_code(&self) -> Option<&'static str> {
        match self.kind.as_ref() {
            InvocationErrorKind::Arguments(_)
            | InvocationErrorKind::Preparation(
                PreparedCommandError::Plan(_)
                | PreparedCommandError::Input(
                    ProgramInputError::ConflictingFileForms
                    | ProgramInputError::RootWithFile
                    | ProgramInputError::InvalidSourceExtension(_),
                ),
            ) => Some("E0700"),
            InvocationErrorKind::Installation(_)
            | InvocationErrorKind::InstallationCompatibility(_) => Some("E0703"),
            InvocationErrorKind::Preparation(PreparedCommandError::Input(
                ProgramInputError::PackageRootNotDirectory(_)
                | ProgramInputError::MissingPackageDeclaration(_)
                | ProgramInputError::PackageDeclarationNotFile(_),
            )) => Some("E0800"),
            InvocationErrorKind::Preparation(PreparedCommandError::Input(
                ProgramInputError::SourceNotFile(_) | ProgramInputError::Filesystem { .. },
            ))
            | InvocationErrorKind::SourceInspection(_) => Some("E0702"),
            InvocationErrorKind::Check(error) => error.diagnostic_code(),
            InvocationErrorKind::Fetch(error) => Some(error.diagnostic_code()),
            InvocationErrorKind::Build(error) => error.diagnostic_code(),
            InvocationErrorKind::Run(error) => error.diagnostic_code(),
            InvocationErrorKind::Test(error) => error.diagnostic_code(),
            InvocationErrorKind::Format(error) => error.diagnostic_code(),
            InvocationErrorKind::AcquisitionInitialization(_) => None,
        }
    }

    /// Renders diagnostics already selected by source-processing phases.
    ///
    /// # Errors
    ///
    /// Returns an integrity failure when a retained diagnostic does not belong to its invocation
    /// source snapshot.
    pub fn render_source_diagnostics(&self) -> Result<Option<String>, DiagnosticRenderError> {
        let context = self.source_diagnostics();
        let Some((diagnostics, sources)) = context else {
            return Ok(None);
        };
        let mut output = String::new();
        for diagnostic in diagnostics {
            output.push_str(&render_source_diagnostic(diagnostic, sources)?);
        }
        Ok(Some(output))
    }

    /// Renders the selected JSON check response from retained invocation state.
    ///
    /// # Errors
    ///
    /// Returns a source/range or root-path presentation-integrity failure.
    pub fn render_json_diagnostics(&self) -> Result<Option<String>, DiagnosticRenderError> {
        let Some(presentation) = &self.presentation else {
            return Ok(None);
        };
        if presentation.format != DiagnosticFormat::Json {
            return Ok(None);
        }
        if presentation.command == "test" {
            if let Some((diagnostics, sources)) = self.source_diagnostics() {
                return crate::test_report::render_test_source_failure_json(
                    presentation.target,
                    diagnostics,
                    sources,
                )
                .map(Some);
            }
            let code = self.diagnostic_code().unwrap_or("E0900");
            return Ok(Some(crate::test_report::render_test_spanless_failure_json(
                presentation.target,
                code,
                &self.to_string(),
            )));
        }
        if let Some((diagnostics, sources)) = self.source_diagnostics() {
            return render_source_diagnostics_json(
                presentation.json_context()?,
                diagnostics,
                sources,
            )
            .map(Some);
        }
        let Some(code) = self.diagnostic_code() else {
            return Ok(None);
        };
        let message = self.to_string();
        render_spanless_diagnostic_json(
            presentation.json_context()?,
            SpanlessDiagnostic::new(code, &message, None),
        )
        .map(Some)
    }

    /// Returns the compiler-owned process status for this typed failure.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        self.failure_class().exit_code()
    }

    #[must_use]
    pub fn failure_class(&self) -> InvocationFailureClass {
        if self.source_diagnostics().is_some()
            || matches!(self.kind.as_ref(), InvocationErrorKind::Format(error) if error.is_source_failure())
        {
            return InvocationFailureClass::Source;
        }
        let user = match self.kind.as_ref() {
            InvocationErrorKind::Arguments(_)
            | InvocationErrorKind::Installation(_)
            | InvocationErrorKind::InstallationCompatibility(_)
            | InvocationErrorKind::Preparation(_)
            | InvocationErrorKind::Fetch(_) => true,
            InvocationErrorKind::Check(error) => error.is_user_failure(),
            InvocationErrorKind::Build(error) => error.is_user_failure(),
            InvocationErrorKind::Run(error) => error.is_user_failure(),
            InvocationErrorKind::Test(error) => error.is_user_failure(),
            InvocationErrorKind::SourceInspection(error) => error.is_user_failure(),
            InvocationErrorKind::Format(error) => error.is_user_failure(),
            InvocationErrorKind::AcquisitionInitialization(_) => false,
        };
        if user {
            InvocationFailureClass::User
        } else {
            InvocationFailureClass::Internal
        }
    }

    fn source_diagnostics(
        &self,
    ) -> Option<(
        &[nocter_diagnostics::SourceDiagnostic],
        &nocter_source::SourceMap,
    )> {
        match self.kind.as_ref() {
            InvocationErrorKind::Build(error) => error.source_diagnostics(),
            InvocationErrorKind::Check(error) => error.source_diagnostics(),
            InvocationErrorKind::Run(error) => error.source_diagnostics(),
            InvocationErrorKind::Test(error) => error.source_diagnostics(),
            InvocationErrorKind::Format(error) => error.source_diagnostics(),
            InvocationErrorKind::Arguments(_)
            | InvocationErrorKind::Installation(_)
            | InvocationErrorKind::InstallationCompatibility(_)
            | InvocationErrorKind::AcquisitionInitialization(_)
            | InvocationErrorKind::Fetch(_)
            | InvocationErrorKind::Preparation(_)
            | InvocationErrorKind::SourceInspection(_) => None,
        }
        .filter(|(diagnostics, _)| !diagnostics.is_empty())
    }
}

impl fmt::Display for InvocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind.as_ref() {
            InvocationErrorKind::Arguments(error) => error.fmt(formatter),
            InvocationErrorKind::Installation(error) => error.fmt(formatter),
            InvocationErrorKind::InstallationCompatibility(error) => error.fmt(formatter),
            InvocationErrorKind::AcquisitionInitialization(error) => {
                write!(formatter, "cannot initialize package acquisition: {error}")
            }
            InvocationErrorKind::Preparation(error) => error.fmt(formatter),
            InvocationErrorKind::Fetch(error) => error.fmt(formatter),
            InvocationErrorKind::Check(error) => error.fmt(formatter),
            InvocationErrorKind::Build(error) => error.fmt(formatter),
            InvocationErrorKind::Run(error) => error.fmt(formatter),
            InvocationErrorKind::Test(error) => error.fmt(formatter),
            InvocationErrorKind::SourceInspection(error) => error.fmt(formatter),
            InvocationErrorKind::Format(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InvocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind.as_ref() {
            InvocationErrorKind::Arguments(error) => Some(error),
            InvocationErrorKind::Installation(error) => Some(error),
            InvocationErrorKind::InstallationCompatibility(error) => Some(error),
            InvocationErrorKind::Preparation(error) => Some(error),
            InvocationErrorKind::AcquisitionInitialization(error) => Some(error),
            InvocationErrorKind::Fetch(error) => Some(error),
            InvocationErrorKind::Check(error) => Some(error),
            InvocationErrorKind::Build(error) => Some(error),
            InvocationErrorKind::Run(error) => Some(error),
            InvocationErrorKind::Test(error) => Some(error),
            InvocationErrorKind::SourceInspection(error) => Some(error),
            InvocationErrorKind::Format(error) => Some(error),
        }
    }
}
