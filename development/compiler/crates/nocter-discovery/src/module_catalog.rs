use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use nocter_compile_input::ModuleIdentity;
use nocter_filesystem::SourceOverlay;
use nocter_model::PackageIdentity;
use nocter_syntax::SourceSyntaxProvider;

use crate::DiscoveryError;

/// Inventories every physical source owned by one directory module.
///
/// A descendant directory containing `index.nct` starts another module and is never traversed.
/// Directories without `index.nct` remain source folders of the selected module.
pub(crate) fn module_sources(
    package: &PackageIdentity,
    package_root: &Path,
    module_directory: &Path,
    source_overlay: &SourceOverlay,
) -> Result<Vec<PathBuf>, DiscoveryError> {
    let mut pending = BTreeSet::from([module_directory.to_path_buf()]);
    let mut sources = BTreeSet::new();
    while let Some(directory) = pending.pop_first() {
        if directory != module_directory
            && source_overlay
                .is_file(&directory.join("index.nct"))
                .map_err(|error| {
                    filesystem_error("inspect child module boundary", &directory, error)
                })?
        {
            continue;
        }
        let entries = fs::read_dir(&directory)
            .map_err(|error| filesystem_error("inventory module sources", &directory, error))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                filesystem_error("read module directory entry", &directory, error)
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| {
                filesystem_error("inspect module directory entry", &path, error)
            })?;
            if file_type.is_dir() {
                pending.insert(path);
            } else if file_type.is_file()
                && path.extension().and_then(|extension| extension.to_str()) == Some("nct")
            {
                sources.insert(path);
            }
        }
    }
    let identity = module_identity(package, package_root, module_directory)?;
    for (path, _) in source_overlay.sources() {
        if path.starts_with(module_directory)
            && path.extension().and_then(|extension| extension.to_str()) == Some("nct")
            && module_for_source(package, package_root, path, source_overlay)? == identity
        {
            sources.insert(path.to_path_buf());
        }
    }
    let mut sources: Vec<_> = sources.into_iter().collect();
    let root = module_directory.join("index.nct");
    if !sources.iter().any(|source| source == &root) {
        return Err(DiscoveryError::MissingModuleRoot {
            module: module_identity(package, package_root, module_directory)?,
            path: root,
        });
    }
    sources.sort_unstable_by(|left, right| {
        (left != &root)
            .cmp(&(right != &root))
            .then_with(|| left.cmp(right))
    });
    Ok(sources)
}

/// Catalogs every directory module in one exact toolchain-standard package.
///
/// Ordinary compilation follows selected targets and authored module edges. Editor analysis of the
/// standard package is different: any installed source may be opened directly, so one standard
/// snapshot must contain every module without inventing a second root-package identity.
pub(crate) fn toolchain_standard_modules(
    package: &PackageIdentity,
    root: &Path,
    package_roots: &mut nocter_package::PackageRootCatalogBuilder,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<Vec<ModuleIdentity>, DiscoveryError> {
    let source_overlay = package_roots.source_overlay().clone();
    let mut pending = BTreeSet::from([root.to_path_buf()]);
    let mut modules = Vec::new();
    while let Some(directory) = pending.pop_first() {
        if directory != root
            && package_roots
                .has_package_declaration(&directory, source_syntax)
                .map_err(DiscoveryError::PackageRootProbe)?
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

/// Resolves physical source ownership to the nearest enclosing directory module.
///
/// The package root is itself a module root. Descendant `index.nct` files establish child modules;
/// ordinary source folders do not. Ownership selection does not grant visibility between sources.
///
/// # Errors
///
/// Returns a typed discovery failure when the source is outside the package, no module root exists,
/// or filesystem inspection fails.
pub fn module_for_source(
    package: &PackageIdentity,
    package_root: &Path,
    source: &Path,
    source_overlay: &SourceOverlay,
) -> Result<ModuleIdentity, DiscoveryError> {
    if !source.starts_with(package_root) {
        return Err(DiscoveryError::InvalidPackageRoot {
            package: package.clone(),
            path: source.to_path_buf(),
        });
    }
    let mut directory = source
        .parent()
        .ok_or_else(|| DiscoveryError::InvalidPackageRoot {
            package: package.clone(),
            path: source.to_path_buf(),
        })?;
    loop {
        let root = directory.join("index.nct");
        if source_overlay
            .is_file(&root)
            .map_err(|error| filesystem_error("inspect module ownership", &root, error))?
        {
            return module_identity(package, package_root, directory);
        }
        if directory == package_root {
            return Err(DiscoveryError::MissingModuleRoot {
                module: ModuleIdentity::new(package.clone(), Vec::<Box<str>>::new()),
                path: root,
            });
        }
        directory = directory
            .parent()
            .ok_or_else(|| DiscoveryError::InvalidPackageRoot {
                package: package.clone(),
                path: source.to_path_buf(),
            })?;
    }
}

fn filesystem_error(operation: &'static str, path: &Path, error: std::io::Error) -> DiscoveryError {
    DiscoveryError::Filesystem {
        operation,
        path: PathBuf::from(path),
        error,
    }
}
