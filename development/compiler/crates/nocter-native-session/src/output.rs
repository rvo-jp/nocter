use nocter_macho::MachOImage;
use nocter_session::ExecutableIdentity;

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

/// A complete native image paired with its selected package executable.
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
}

impl CompiledNativeImageSet {
    pub(crate) fn new(entries: Vec<NativeImageEntry>) -> Self {
        Self {
            entries: entries.into_boxed_slice(),
        }
    }

    #[must_use]
    pub const fn entries(&self) -> &[NativeImageEntry] {
        &self.entries
    }

    #[must_use]
    pub fn into_entries(self) -> Box<[NativeImageEntry]> {
        self.entries
    }
}
