use std::path::{Path, PathBuf};

use nocter_command::{
    CommandToolchain, ParsedBuildCommand, ParsedCheckCommand, ParsedCommand, ParsedFetchCommand,
    ParsedRunCommand, ResolvedProgramInput, execute_prepared_build, execute_prepared_check,
    execute_prepared_fetch, execute_prepared_run,
};
use nocter_installation::CompilerInstallation;
use nocter_package_acquisition::EmbeddedPackageAcquisition;

use crate::presentation::InvocationDiagnosticPresentation;
use crate::{DoctorReport, InvocationError, InvocationErrorKind, InvocationOutcome, VersionReport};

pub(crate) fn execute_parsed_command(
    command: ParsedCommand,
    current_directory: &Path,
    installation: &CompilerInstallation,
    toolchain: &CommandToolchain,
    presentation: Option<InvocationDiagnosticPresentation>,
) -> Result<InvocationOutcome, InvocationError> {
    match command {
        ParsedCommand::Help(request) => Ok(InvocationOutcome::Help(request)),
        ParsedCommand::Version => Ok(InvocationOutcome::Version(
            VersionReport::from_installation(installation),
        )),
        ParsedCommand::Doctor => Ok(InvocationOutcome::Doctor(DoctorReport::from_installation(
            installation,
        ))),
        ParsedCommand::Fetch(command) => {
            execute_fetch(command, current_directory, toolchain, presentation)
        }
        ParsedCommand::Check(command) => {
            execute_check(command, current_directory, toolchain, presentation)
        }
        ParsedCommand::Build(command) => {
            execute_build(command, current_directory, toolchain, presentation)
        }
        ParsedCommand::Run(command) => {
            execute_run(command, current_directory, toolchain, presentation)
        }
    }
}

fn execute_fetch(
    command: ParsedFetchCommand,
    current_directory: &Path,
    toolchain: &CommandToolchain,
    presentation: Option<InvocationDiagnosticPresentation>,
) -> Result<InvocationOutcome, InvocationError> {
    let command = command.prepare(current_directory).map_err(|error| {
        InvocationError::new(
            InvocationErrorKind::Preparation(error),
            presentation.clone(),
        )
    })?;
    let mut acquisition = initialize_acquisition(presentation.as_ref())?;
    execute_prepared_fetch(command, toolchain.packages(), &mut acquisition)
        .map(InvocationOutcome::Fetch)
        .map_err(|error| {
            InvocationError::new(InvocationErrorKind::Fetch(Box::new(error)), presentation)
        })
}

fn execute_check(
    command: ParsedCheckCommand,
    current_directory: &Path,
    toolchain: &CommandToolchain,
    mut presentation: Option<InvocationDiagnosticPresentation>,
) -> Result<InvocationOutcome, InvocationError> {
    let command = command.prepare(current_directory).map_err(|error| {
        InvocationError::new(
            InvocationErrorKind::Preparation(error),
            presentation.clone(),
        )
    })?;
    if let Some(presentation) = presentation.as_mut() {
        let root: PathBuf = match command.plan().input() {
            ResolvedProgramInput::Package(package) => package.declaration().into(),
            ResolvedProgramInput::SingleFile(source) => source.source().into(),
        };
        presentation.root = Some(root.clone());
        presentation.root_absolute_path = Some(root);
    }
    let mut acquisition = initialize_acquisition(presentation.as_ref())?;
    execute_prepared_check(command, toolchain, &mut acquisition)
        .map(|result| InvocationOutcome::Check(Box::new(result)))
        .map_err(|error| {
            InvocationError::new(InvocationErrorKind::Check(Box::new(error)), presentation)
        })
}

fn execute_build(
    command: ParsedBuildCommand,
    current_directory: &Path,
    toolchain: &CommandToolchain,
    presentation: Option<InvocationDiagnosticPresentation>,
) -> Result<InvocationOutcome, InvocationError> {
    let command = command.prepare(current_directory).map_err(|error| {
        InvocationError::new(
            InvocationErrorKind::Preparation(error),
            presentation.clone(),
        )
    })?;
    let mut acquisition = initialize_acquisition(presentation.as_ref())?;
    execute_prepared_build(command, toolchain, &mut acquisition)
        .map(InvocationOutcome::Build)
        .map_err(|error| {
            InvocationError::new(InvocationErrorKind::Build(Box::new(error)), presentation)
        })
}

fn execute_run(
    command: ParsedRunCommand,
    current_directory: &Path,
    toolchain: &CommandToolchain,
    presentation: Option<InvocationDiagnosticPresentation>,
) -> Result<InvocationOutcome, InvocationError> {
    let command = command.prepare(current_directory).map_err(|error| {
        InvocationError::new(
            InvocationErrorKind::Preparation(error),
            presentation.clone(),
        )
    })?;
    let mut acquisition = initialize_acquisition(presentation.as_ref())?;
    execute_prepared_run(command, toolchain, &mut acquisition)
        .map(InvocationOutcome::Run)
        .map_err(|error| {
            InvocationError::new(InvocationErrorKind::Run(Box::new(error)), presentation)
        })
}

fn initialize_acquisition(
    presentation: Option<&InvocationDiagnosticPresentation>,
) -> Result<EmbeddedPackageAcquisition, InvocationError> {
    EmbeddedPackageAcquisition::new().map_err(|error| {
        InvocationError::new(
            InvocationErrorKind::AcquisitionInitialization(error),
            presentation.cloned(),
        )
    })
}
