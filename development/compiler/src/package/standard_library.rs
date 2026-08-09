//! Policy for the implicit toolchain standard-library package.

use super::SourcePackage;
use std::path::{Path, PathBuf};

pub(crate) const STANDARD_LIBRARY_ALIAS: &str = "std";

#[derive(Debug, Clone)]
pub(crate) struct StandardLibrarySelection {
    root: PathBuf,
    expected_version: Option<String>,
}

impl StandardLibrarySelection {
    pub(crate) fn active() -> Option<Self> {
        let home = crate::home::resolve_nocter_home().ok()?;
        let expected_version = crate::home::read_nocter_home_version(&home).ok();
        Some(Self::new(home.join("std"), expected_version))
    }

    pub(crate) fn new(root: PathBuf, expected_version: Option<String>) -> Self {
        Self {
            root,
            expected_version,
        }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn expected_version(&self) -> Option<&str> {
        self.expected_version.as_deref()
    }
}

pub(crate) fn validation_errors(
    package: &SourcePackage,
    expected_version: Option<&str>,
) -> Vec<String> {
    let mut errors = Vec::new();
    if package.display_name() != STANDARD_LIBRARY_ALIAS {
        errors.push("toolchain standard-library package must declare `#name: \"std\"`".to_string());
    }
    if package.version().is_none() {
        errors.push("toolchain standard-library package must declare `#version`".to_string());
    } else if let Some(expected) = expected_version
        && package.version() != Some(expected)
    {
        errors.push(format!(
            "toolchain standard-library package version `{}` does not match Nocter home version `{expected}`",
            package.version().expect("checked standard-library version"),
        ));
    }
    if !package.dependencies().is_empty()
        || !package.locks().is_empty()
        || !package.executables().is_empty()
        || !package.tests().is_empty()
    {
        errors.push(
            "toolchain standard-library package cannot declare dependencies, locks, executables, or tests"
                .to_string(),
        );
    }
    errors
}
