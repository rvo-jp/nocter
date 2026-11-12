use crate::PackageIdentity;

/// Exact package and normalized directory-module path, independent of physical source syntax.
/// Source discovery and topology validation establish the path; this identity performs no lookup.
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

/// A normalized outward audience, not permission to see a private physical source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleAudience {
    Public,
    Package(PackageIdentity),
    Descendants(ModuleIdentity),
}
impl ModuleAudience {
    #[must_use]
    pub fn contains(&self, caller: &ModuleIdentity) -> bool {
        match self {
            Self::Public => true,
            Self::Package(package) => caller.package() == package,
            Self::Descendants(boundary) => {
                caller.package() == boundary.package() && caller.path().starts_with(boundary.path())
            }
        }
    }
    #[must_use]
    pub fn is_within(&self, target: &Self) -> bool {
        match (self, target) {
            (_, Self::Public) => true,
            (Self::Public, _) => false,
            (Self::Package(a), Self::Package(b)) => a == b,
            (Self::Descendants(a), Self::Package(b)) => a.package() == b,
            (Self::Package(a), Self::Descendants(b)) => b.path().is_empty() && a == b.package(),
            (Self::Descendants(a), Self::Descendants(b)) => {
                a.package() == b.package() && a.path().starts_with(b.path())
            }
        }
    }
    #[must_use]
    pub fn intersection(self, owner: &Self) -> Option<Self> {
        if self.is_within(owner) {
            Some(self)
        } else if owner.is_within(&self) {
            Some(owner.clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;
