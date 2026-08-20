use nocter_source_index::SourceIndex;
use nocter_target_program::TargetProgram;

/// A target-validated semantic program and its independent source projection.
#[derive(Debug)]
pub struct CompiledTarget {
    program: TargetProgram,
    source_index: SourceIndex,
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
