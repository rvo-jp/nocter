//! Filesystem and process orchestration for completed native compiler sessions.
//!
//! This crate may persist or launch a completed native image. It never assembles semantic or
//! backend compiler stages and never reopens source or target identity.

mod arguments;
mod artifact;
mod build;
mod check;
mod command_schema;
mod compiler;
mod execute;
mod failure;
mod fetch;
mod format;
mod graph;
mod init;
mod input;
mod inspection;
mod output_plan;
mod package_state;
mod planning;
mod run;
mod run_invocation;
mod source;
mod standalone_source;
mod test;

pub use arguments::{
    CommandArgumentError, CommandArgumentFailure, DiagnosticFormat, GraphOutputFormat,
    ParsedBuildCommand, ParsedCheckCommand, ParsedCommand, ParsedFetchCommand, ParsedFormatCommand,
    ParsedGraphCommand, ParsedInitCommand, ParsedRunCommand, ParsedSourceInspectionCommand,
    ParsedTestCommand, PreparedBuildCommand, PreparedCheckCommand, PreparedCommandError,
    PreparedFetchCommand, PreparedGraphCommand, PreparedRunCommand, PreparedTestCommand,
    ResolutionOptions, SourceInspectionKind, parse_command_arguments, parse_command_invocation,
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
pub use compiler::CommandAnalysisError;
pub use execute::{
    BuildCommandExecutionError, BuildCommandResult, RunCommandExecutionError,
    execute_prepared_build, execute_prepared_run,
};
pub use failure::CommandCompilationFailure;
pub use fetch::{FetchCommandExecutionError, FetchCommandResult, execute_prepared_fetch};
pub use format::{FormatCommandError, FormatCommandResult, execute_format};
pub use graph::{
    GraphCommandError, GraphCommandResult, GraphDependency, GraphDependencySource, GraphPackage,
    execute_prepared_graph,
};
pub use init::{InitCommandError, InitCommandResult, InitializedPackageKind, execute_init};
pub use input::{
    InputOperation, PackageCommandInput, ProgramInputError, ProgramInputOptions,
    ResolvedProgramInput, SingleFileCommandInput, resolve_package_input, resolve_program_input,
    resolve_single_file_input,
};
pub use inspection::{
    SourceInspectionCommandError, SourceInspectionCommandResult, execute_source_inspection,
};
pub use output_plan::{BuildOutputPlan, OutputPlanError, PlannedOutput};
pub use package_state::{CommandPackageContext, CommandPackageStateError};
pub use planning::{
    BuildCommandOptions, BuildCommandPlan, BuildOperation, CheckCommandOptions, CheckCommandPlan,
    CommandPlanError, RunCommandOptions, RunCommandPlan, SelectedBuildOutput, TestCommandOptions,
    TestCommandPlan,
};
pub use run::{ExecutedProgram, RunCommandError, run_executable};
pub use run_invocation::RunProgramArguments;
pub use source::{CommandSourceError, CommandToolchain};
pub use standalone_source::StandaloneSourceError;
pub use test::{
    TestCommandExecutionError, TestCommandIntegrityError, TestCommandPresentation,
    TestCommandResult, TestRunDiagnostic, TestRunOutcome, TestRunResult, TestRunTarget,
    TestSummary, execute_prepared_test,
};

#[cfg(test)]
mod tests;
