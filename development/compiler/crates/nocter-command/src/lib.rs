//! Filesystem and process orchestration for completed native compiler sessions.
//!
//! This crate may persist or launch a completed native image. It never assembles semantic or
//! backend compiler stages and never reopens source or target identity.

mod arguments;
mod artifact;
mod build;
mod check;
mod command_schema;
mod execute;
mod failure;
mod fetch;
mod input;
mod output_plan;
mod package_state;
mod planning;
mod run;
mod source;

pub use arguments::{
    CommandArgumentError, CommandArgumentFailure, DiagnosticFormat, ParsedBuildCommand,
    ParsedCheckCommand, ParsedCommand, ParsedFetchCommand, ParsedRunCommand, PreparedBuildCommand,
    PreparedCheckCommand, PreparedCommandError, PreparedFetchCommand, PreparedRunCommand,
    ResolutionOptions, parse_command_arguments, parse_command_invocation,
};
pub use artifact::{
    ArtifactError, ArtifactOperation, PersistentArtifact, TemporaryArtifact, persist_native_image,
    stage_temporary_image,
};
pub use build::{
    BuildCommandError, BuildSetCommandError, BuiltExecutable, BuiltExecutableEntry,
    BuiltExecutableSet, build_executable, build_executables, build_selected_executable,
};
pub use check::{
    CheckCommandExecutionError, CheckCommandPresentation, CheckCommandResult,
    execute_prepared_check,
};
pub use command_schema::HelpRequest;
pub use execute::{
    BuildCommandExecutionError, BuildCommandResult, RunCommandExecutionError,
    execute_prepared_build, execute_prepared_run,
};
pub use failure::CommandCompilationFailure;
pub use fetch::{FetchCommandExecutionError, FetchCommandResult, execute_prepared_fetch};
pub use input::{
    InputOperation, PackageCommandInput, ProgramInputError, ProgramInputOptions,
    ResolvedProgramInput, SingleFileCommandInput, resolve_package_input, resolve_program_input,
};
pub use output_plan::{BuildOutputPlan, OutputPlanError, PlannedOutput};
pub use package_state::{CommandPackageContext, CommandPackageStateError};
pub use planning::{
    BuildCommandOptions, BuildCommandPlan, BuildOperation, CheckCommandOptions, CheckCommandPlan,
    CommandPlanError, RunCommandOptions, RunCommandPlan, SelectedBuildOutput,
};
pub use run::{ExecutedProgram, RunCommandError, run_executable};
pub use source::{CommandSourceError, CommandToolchain};

#[cfg(test)]
mod tests;
