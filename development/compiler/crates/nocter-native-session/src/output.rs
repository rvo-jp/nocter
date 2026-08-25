use nocter_macho::MachOImage;
use nocter_session::ExecutableIdentity;
use nocter_source_index::SourceIndex;

/// A complete native executable artifact whose container format remains backend-private.
#[derive(Debug)]
pub struct NativeImage {
    bytes: Box<[u8]>,
}

impl NativeImage {
    pub(crate) fn from_macho(image: MachOImage) -> Self {
        Self {
            bytes: image.into_bytes(),
        }
    }

    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// One deterministic native executable image plus its independent source projection.
#[derive(Debug)]
pub struct CompiledNativeImage {
    identity: ExecutableIdentity,
    image: NativeImage,
    source_index: SourceIndex,
}

impl CompiledNativeImage {
    pub(crate) const fn new(
        identity: ExecutableIdentity,
        image: NativeImage,
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
    pub const fn image(&self) -> &NativeImage {
        &self.image
    }

    #[must_use]
    pub fn into_parts(self) -> (NativeImage, SourceIndex) {
        (self.image, self.source_index)
    }
}

/// One native image in package-target declaration order.
#[derive(Debug)]
pub struct NativeImageEntry {
    identity: ExecutableIdentity,
    image: NativeImage,
}

impl NativeImageEntry {
    pub(crate) const fn new(identity: ExecutableIdentity, image: NativeImage) -> Self {
        Self { identity, image }
    }

    #[must_use]
    pub const fn identity(&self) -> &ExecutableIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn image(&self) -> &NativeImage {
        &self.image
    }

    #[must_use]
    pub fn into_parts(self) -> (ExecutableIdentity, NativeImage) {
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
    pub fn into_parts(self) -> (Box<[NativeImageEntry]>, SourceIndex) {
        (self.entries, self.source_index)
    }
}
