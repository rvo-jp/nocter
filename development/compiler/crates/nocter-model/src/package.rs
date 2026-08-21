/// Stable identity assigned to one exactly resolved package.
///
/// Display names are presentation metadata and may collide. This identity distinguishes workspace
/// roots, dependencies, and content-addressed package resolutions.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageIdentity(Box<str>);

impl PackageIdentity {
    #[must_use]
    pub fn new(identity: impl Into<Box<str>>) -> Self {
        Self(identity.into())
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        &self.0
    }
}
