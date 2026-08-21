use nocter_macho::MachOImage;
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

/// One deterministic native executable image plus its independent source projection.
#[derive(Debug)]
pub struct CompiledNativeImage {
    identity: ExecutableIdentity,
    image: MachOImage,
    source_index: SourceIndex,
}

/// One native image in package-target declaration order.
#[derive(Debug)]
pub struct NativeImageEntry {
    identity: ExecutableIdentity,
    image: MachOImage,
}

impl NativeImageEntry {
    pub(crate) const fn new(identity: ExecutableIdentity, image: MachOImage) -> Self {
        Self { identity, image }
    }

    #[must_use]
    pub const fn identity(&self) -> &ExecutableIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn image(&self) -> &MachOImage {
        &self.image
    }

    #[must_use]
    pub fn into_parts(self) -> (ExecutableIdentity, MachOImage) {
        (self.identity, self.image)
    }
}

/// Complete native output for every executable owned by the command-root packages.
#[derive(Debug)]
pub struct CompiledNativeImageSet {
    entries: Box<[NativeImageEntry]>,
    source_index: SourceIndex,
}

impl CompiledNativeImageSet {
    pub(crate) fn new(entries: Vec<NativeImageEntry>, source_index: SourceIndex) -> Self {
        Self {
            entries: entries.into_boxed_slice(),
            source_index,
        }
    }

    #[must_use]
    pub const fn entries(&self) -> &[NativeImageEntry] {
        &self.entries
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(self) -> (Box<[NativeImageEntry]>, SourceIndex) {
        (self.entries, self.source_index)
    }
}

impl CompiledNativeImage {
    pub(crate) const fn new(
        identity: ExecutableIdentity,
        image: MachOImage,
        source_index: SourceIndex,
    ) -> Self {
        Self {
            identity,
            image,
            source_index,
        }
    }

    #[must_use]
    pub const fn identity(&self) -> &ExecutableIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn image(&self) -> &MachOImage {
        &self.image
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(self) -> (MachOImage, SourceIndex) {
        (self.image, self.source_index)
    }
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
