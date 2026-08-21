//! Filesystem and process orchestration for completed native compiler sessions.
//!
//! This crate may persist or launch a completed native image. It never assembles semantic or
//! backend compiler stages and never reopens source or target identity.

mod arguments;
mod artifact;
mod build;
mod input;
mod output_plan;
mod planning;
mod run;

pub use arguments::{
    CommandArgumentError, ParsedBuildCommand, ParsedCommand, ParsedRunCommand,
    PreparedBuildCommand, PreparedCommandError, PreparedRunCommand, ResolutionOptions,
    parse_command_arguments,
};
pub use artifact::{
    ArtifactError, ArtifactOperation, PersistentArtifact, TemporaryArtifact, persist_native_image,
    stage_temporary_image,
};
pub use build::{
    BuildCommandError, BuildSetCommandError, BuiltExecutable, BuiltExecutableEntry,
    BuiltExecutableSet, build_executable, build_executables, build_selected_executable,
};
pub use input::{
    InputOperation, PackageCommandInput, ProgramInputError, ProgramInputOptions,
    ResolvedProgramInput, SingleFileCommandInput, resolve_program_input,
};
pub use output_plan::{BuildOutputPlan, OutputPlanError, PlannedOutput};
pub use planning::{
    BuildCommandOptions, BuildCommandPlan, BuildOperation, CommandPlanError, RunCommandOptions,
    RunCommandPlan, SelectedBuildOutput,
};
pub use run::{ExecutedProgram, RunCommandError, run_executable};

#[cfg(test)]
mod tests;
