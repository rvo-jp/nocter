//! Filesystem and process orchestration for completed native compiler sessions.
//!
//! This crate may persist or launch a completed native image. It never assembles semantic or
//! backend compiler stages and never reopens source or target identity.

mod artifact;
mod build;
mod input;
mod output_plan;
mod run;

pub use artifact::{
    ArtifactError, ArtifactOperation, PersistentArtifact, TemporaryArtifact, persist_native_image,
    stage_temporary_image,
};
pub use build::{
    BuildCommandError, BuildSetCommandError, BuiltExecutable, BuiltExecutableEntry,
    BuiltExecutableSet, build_executable, build_executables,
};
pub use input::{
    InputOperation, PackageCommandInput, ProgramInputError, ProgramInputOptions,
    ResolvedProgramInput, SingleFileCommandInput, resolve_program_input,
};
pub use output_plan::{BuildOutputPlan, OutputPlanError, PlannedOutput};
pub use run::{ExecutedProgram, RunCommandError, run_executable};

#[cfg(test)]
mod tests;
