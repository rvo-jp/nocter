//! Filesystem and process orchestration for completed native compiler sessions.
//!
//! This crate may persist or launch a completed native image. It never assembles semantic or
//! backend compiler stages and never reopens source or target identity.

mod artifact;
mod build;
mod run;

pub use artifact::{
    ArtifactError, ArtifactOperation, PersistentArtifact, TemporaryArtifact, persist_native_image,
    stage_temporary_image,
};
pub use build::{BuildCommandError, BuiltExecutable, build_executable};
pub use run::{ExecutedProgram, RunCommandError, run_executable};

#[cfg(test)]
mod tests;
