use nocter_model::PackageIdentity;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModuleIdentity {
    package: PackageIdentity,
    path: Box<[Box<str>]>,
}

impl ModuleIdentity {
    #[must_use]
    pub fn new<S>(package: PackageIdentity, path: impl IntoIterator<Item = S>) -> Self
    where
        S: Into<Box<str>>,
    {
        Self {
            package,
            path: path
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    #[must_use]
    pub const fn package(&self) -> &PackageIdentity {
        &self.package
    }

    #[must_use]
    pub const fn path(&self) -> &[Box<str>] {
        &self.path
    }
}
