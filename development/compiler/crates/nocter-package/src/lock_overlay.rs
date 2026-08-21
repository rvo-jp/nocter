use std::collections::BTreeMap;
use std::fmt;

use nocter_model::PackageIdentity;

use crate::ExactDependencyLock;

/// Exact lock selections supplied by a package-state transaction before source commit.
///
/// The overlay is immutable resolution input. It does not grant the resolver authority to edit a
/// package declaration or package store.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageLockOverlay {
    packages: BTreeMap<PackageIdentity, BTreeMap<Box<str>, ExactDependencyLock>>,
}

impl PackageLockOverlay {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            packages: BTreeMap::new(),
        }
    }

    /// Adds one exact selection without permitting a previous selection to change.
    ///
    /// Repeating the same selection is idempotent.
    ///
    /// # Errors
    ///
    /// Returns an error when the same package dependency already selects a different lock.
    pub fn insert(
        &mut self,
        package: PackageIdentity,
        alias: impl Into<Box<str>>,
        lock: ExactDependencyLock,
    ) -> Result<(), PackageLockOverlayError> {
        let alias = alias.into();
        let locks = self.packages.entry(package.clone()).or_default();
        if let Some(first) = locks.get(&alias) {
            if first != &lock {
                return Err(PackageLockOverlayError {
                    package,
                    alias,
                    first: first.clone(),
                    second: lock,
                });
            }
            return Ok(());
        }
        locks.insert(alias, lock);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, package: &PackageIdentity, alias: &str) -> Option<&ExactDependencyLock> {
        self.packages.get(package)?.get(alias)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packages.values().all(BTreeMap::is_empty)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageLockOverlayError {
    package: PackageIdentity,
    alias: Box<str>,
    first: ExactDependencyLock,
    second: ExactDependencyLock,
}

impl PackageLockOverlayError {
    #[must_use]
    pub const fn package(&self) -> &PackageIdentity {
        &self.package
    }

    #[must_use]
    pub const fn alias(&self) -> &str {
        &self.alias
    }

    #[must_use]
    pub const fn first(&self) -> &ExactDependencyLock {
        &self.first
    }

    #[must_use]
    pub const fn second(&self) -> &ExactDependencyLock {
        &self.second
    }
}

impl fmt::Display for PackageLockOverlayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "package {} dependency {} selects both {} and {}",
            self.package.as_str(),
            self.alias,
            self.first.literal(),
            self.second.literal()
        )
    }
}

impl std::error::Error for PackageLockOverlayError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_selection_is_idempotent_but_change_is_rejected() {
        let package = PackageIdentity::new("root");
        let first = ExactDependencyLock::git("7db21c1000000000000000000000000000000000").unwrap();
        let second = ExactDependencyLock::git("8db21c1000000000000000000000000000000000").unwrap();
        let mut overlay = PackageLockOverlay::new();

        overlay
            .insert(package.clone(), "json", first.clone())
            .unwrap();
        overlay.insert(package.clone(), "json", first).unwrap();
        assert!(matches!(
            overlay.insert(package, "json", second),
            Err(PackageLockOverlayError { .. })
        ));
    }
}
