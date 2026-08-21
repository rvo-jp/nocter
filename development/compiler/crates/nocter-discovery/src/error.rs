use std::fmt;
use std::io;
use std::path::PathBuf;

use nocter_compile_input::{ModuleIdentity, PackageIdentity};
use nocter_source::SourceError;
use nocter_syntax::NodeId;
use nocter_target_selection::TargetSelectionError;

mod toolchain;

pub use toolchain::ToolchainDiscoveryError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportFailure {
    UnknownDependency { alias: Box<str> },
    OutsidePackage,
    NotFound,
    Ambiguous { source: PathBuf, module: PathBuf },
    CrossesPackage { root: PathBuf },
    CrossesModule { module: ModuleIdentity },
    InvalidModuleDirectory,
    SingleFileLocalImport,
}

#[derive(Debug)]
pub enum DiscoveryError {
    DuplicatePackage(PackageIdentity),
    UnknownPackage(PackageIdentity),
    Toolchain(ToolchainDiscoveryError),
    InvalidPackageRoot {
        package: PackageIdentity,
        path: PathBuf,
    },
    MissingPackageFile {
        package: PackageIdentity,
        path: PathBuf,
    },
    InvalidSingleFileExtension(PathBuf),
    DuplicateCanonicalRoot {
        first: PackageIdentity,
        second: PackageIdentity,
        path: PathBuf,
    },
    MissingModuleRoot {
        module: ModuleIdentity,
        path: PathBuf,
    },
    InvalidModulePath {
        module: ModuleIdentity,
        path: PathBuf,
        failure: ImportFailure,
    },
    Import {
        declaration: NodeId,
        path: Box<str>,
        failure: ImportFailure,
    },
    ConflictingSourceOwner {
        path: PathBuf,
        first: ModuleIdentity,
        second: ModuleIdentity,
    },
    NonUnicodeCanonicalPath(PathBuf),
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    Source {
        path: PathBuf,
        error: SourceError,
    },
    TargetSelection(TargetSelectionError),
    InconsistentSyntax(NodeId),
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePackage(package) => {
                write!(formatter, "duplicate resolved package {}", package.as_str())
            }
            Self::UnknownPackage(package) => {
                write!(formatter, "unknown resolved package {}", package.as_str())
            }
            Self::Toolchain(error) => error.fmt(formatter),
            Self::InvalidPackageRoot { package, path } => write!(
                formatter,
                "package {} has invalid root {}",
                package.as_str(),
                path.display()
            ),
            Self::MissingPackageFile { package, path } => write!(
                formatter,
                "package {} has no package file at {}",
                package.as_str(),
                path.display()
            ),
            Self::InvalidSingleFileExtension(path) => write!(
                formatter,
                "single-file input must have the .nct extension: {}",
                path.display()
            ),
            Self::DuplicateCanonicalRoot {
                first,
                second,
                path,
            } => write!(
                formatter,
                "packages {} and {} share canonical root {}",
                first.as_str(),
                second.as_str(),
                path.display()
            ),
            Self::MissingModuleRoot { module, path } => {
                write!(
                    formatter,
                    "module {module:?} has no root at {}",
                    path.display()
                )
            }
            Self::InvalidModulePath {
                module,
                path,
                failure,
            } => write!(
                formatter,
                "module {module:?} has invalid root {}: {failure:?}",
                path.display()
            ),
            Self::Import {
                declaration,
                path,
                failure,
            } => write!(
                formatter,
                "use {declaration:?} cannot resolve {path}: {failure:?}"
            ),
            Self::ConflictingSourceOwner {
                path,
                first,
                second,
            } => write!(
                formatter,
                "source {} is owned by both {first:?} and {second:?}",
                path.display()
            ),
            Self::NonUnicodeCanonicalPath(path) => {
                write!(
                    formatter,
                    "canonical path is not Unicode: {}",
                    path.display()
                )
            }
            Self::Filesystem {
                operation,
                path,
                error,
            } => write!(formatter, "cannot {operation} {}: {error}", path.display()),
            Self::Source { path, error } => {
                write!(formatter, "cannot ingest {}: {error:?}", path.display())
            }
            Self::TargetSelection(error) => {
                write!(formatter, "invalid target selection: {error:?}")
            }
            Self::InconsistentSyntax(node) => {
                write!(formatter, "syntax tree is inconsistent at {node:?}")
            }
        }
    }
}

impl std::error::Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filesystem { error, .. } => Some(error),
            Self::Toolchain(error) => Some(error),
            Self::DuplicatePackage(_)
            | Self::UnknownPackage(_)
            | Self::InvalidPackageRoot { .. }
            | Self::MissingPackageFile { .. }
            | Self::InvalidSingleFileExtension(_)
            | Self::DuplicateCanonicalRoot { .. }
            | Self::MissingModuleRoot { .. }
            | Self::InvalidModulePath { .. }
            | Self::Import { .. }
            | Self::ConflictingSourceOwner { .. }
            | Self::NonUnicodeCanonicalPath(_)
            | Self::Source { .. }
            | Self::TargetSelection(_)
            | Self::InconsistentSyntax(_) => None,
        }
    }
}
