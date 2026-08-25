use nocter_source_index::SourceIndex;
use nocter_target_program::ExecutableProgram;
use nocter_target_program::TargetProgram;

use crate::ExecutableIdentity;

/// A target-validated semantic program and its independent source projection.
#[derive(Debug)]
pub struct CompiledTarget {
    program: TargetProgram,
    source_index: SourceIndex,
}

/// One fully selected and specialized process executable plus independent source projection.
#[derive(Debug)]
pub struct CompiledExecutable {
    identity: ExecutableIdentity,
    program: ExecutableProgram,
    source_index: SourceIndex,
}

impl CompiledExecutable {
    pub(crate) const fn new(
        identity: ExecutableIdentity,
        program: ExecutableProgram,
        source_index: SourceIndex,
    ) -> Self {
        Self {
            identity,
            program,
            source_index,
        }
    }

    #[must_use]
    pub const fn identity(&self) -> &ExecutableIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn program(&self) -> &ExecutableProgram {
        &self.program
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(self) -> (ExecutableProgram, SourceIndex) {
        (self.program, self.source_index)
    }
}

impl CompiledTarget {
    pub(crate) const fn new(program: TargetProgram, source_index: SourceIndex) -> Self {
        Self {
            program,
            source_index,
        }
    }

    #[must_use]
    pub const fn program(&self) -> &TargetProgram {
        &self.program
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(self) -> (TargetProgram, SourceIndex) {
        (self.program, self.source_index)
    }
}
