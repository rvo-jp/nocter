use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use nocter_compile_input::ModuleIdentity;
use nocter_filesystem::SourceOverlay;
use nocter_model::PackageIdentity;

use crate::DiscoveryError;

/// Catalogs every directory module in one exact toolchain-standard package.
///
/// Ordinary compilation follows selected targets and authored module edges. Editor analysis of the
/// standard package is different: any installed source may be opened directly, so one standard
/// snapshot must contain every module without inventing a second root-package identity.
pub(crate) fn toolchain_standard_modules(
    package: &PackageIdentity,
    root: &Path,
    source_overlay: &SourceOverlay,
) -> Result<Vec<ModuleIdentity>, DiscoveryError> {
    let mut pending = BTreeSet::from([root.to_path_buf()]);
    let mut modules = Vec::new();
    while let Some(directory) = pending.pop_first() {
        let package_declaration = directory.join("nocter.nct");
        if directory != root
            && source_overlay
                .is_file(&package_declaration)
                .map_err(|error| {
                    filesystem_error(
                        "inspect nested package boundary",
                        &package_declaration,
                        error,
                    )
                })?
        {
            continue;
        }
        let module_root = directory.join("index.nct");
        if source_overlay
            .is_file(&module_root)
            .map_err(|error| filesystem_error("inspect module root", &module_root, error))?
        {
            modules.push(module_identity(package, root, &directory)?);
        }
        let entries = fs::read_dir(&directory).map_err(|error| {
            filesystem_error("catalog standard package modules", &directory, error)
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                filesystem_error("read standard package directory entry", &directory, error)
            })?;
            let file_type = entry.file_type().map_err(|error| {
                filesystem_error(
                    "inspect standard package directory entry",
                    &entry.path(),
                    error,
                )
            })?;
            if file_type.is_dir() {
                pending.insert(entry.path());
            }
        }
    }
    modules.sort_unstable();
    Ok(modules)
}

fn module_identity(
    package: &PackageIdentity,
    root: &Path,
    directory: &Path,
) -> Result<ModuleIdentity, DiscoveryError> {
    let relative =
        directory
            .strip_prefix(root)
            .map_err(|_| DiscoveryError::InvalidPackageRoot {
                package: package.clone(),
                path: directory.to_path_buf(),
            })?;
    let mut path = Vec::new();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(DiscoveryError::InvalidPackageRoot {
                package: package.clone(),
                path: directory.to_path_buf(),
            });
        };
        let segment = segment
            .to_str()
            .ok_or_else(|| DiscoveryError::NonUnicodeCanonicalPath(directory.to_path_buf()))?;
        path.push(Box::<str>::from(segment));
    }
    Ok(ModuleIdentity::new(package.clone(), path))
}

fn filesystem_error(operation: &'static str, path: &Path, error: std::io::Error) -> DiscoveryError {
    DiscoveryError::Filesystem {
        operation,
        path: PathBuf::from(path),
        error,
    }
}
