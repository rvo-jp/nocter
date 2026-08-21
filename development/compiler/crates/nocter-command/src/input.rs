use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Raw package/file choices accepted by build, run, and check command parsers.
///
/// Positional and `--file` sources remain separate until validation so a parser cannot silently
/// discard a conflicting spelling.
#[derive(Debug, Default)]
pub struct ProgramInputOptions {
    root: Option<PathBuf>,
    positional_file: Option<PathBuf>,
    explicit_file: Option<PathBuf>,
}

impl ProgramInputOptions {
    #[must_use]
    pub fn new(
        root: Option<PathBuf>,
        positional_file: Option<PathBuf>,
        explicit_file: Option<PathBuf>,
    ) -> Self {
        Self {
            root,
            positional_file,
            explicit_file,
        }
    }

    #[must_use]
    pub fn package(root: Option<impl Into<PathBuf>>) -> Self {
        Self::new(root.map(Into::into), None, None)
    }

    #[must_use]
    pub fn positional_file(source: impl Into<PathBuf>) -> Self {
        Self::new(None, Some(source.into()), None)
    }

    #[must_use]
    pub fn explicit_file(source: impl Into<PathBuf>) -> Self {
        Self::new(None, None, Some(source.into()))
    }
}

/// One canonical package or explicit single-file input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedProgramInput {
    Package(PackageCommandInput),
    SingleFile(SingleFileCommandInput),
}

impl ResolvedProgramInput {
    #[must_use]
    pub const fn package(&self) -> Option<&PackageCommandInput> {
        match self {
            Self::Package(package) => Some(package),
            Self::SingleFile(_) => None,
        }
    }

    #[must_use]
    pub const fn single_file(&self) -> Option<&SingleFileCommandInput> {
        match self {
            Self::Package(_) => None,
            Self::SingleFile(source) => Some(source),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageCommandInput {
    root: PathBuf,
    declaration: PathBuf,
}

impl PackageCommandInput {
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn declaration(&self) -> &Path {
        &self.declaration
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingleFileCommandInput {
    source: PathBuf,
}

impl SingleFileCommandInput {
    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source
    }
}

/// Resolves a command's explicit input mode against one caller-owned current directory.
///
/// The function does not search ancestors, infer a source filename, or inspect package contents
/// beyond requiring the selected root's `nocter.nct`.
///
/// # Errors
///
/// Returns a conflict before filesystem access, or the exact invalid path/filesystem operation.
pub fn resolve_program_input(
    current_directory: impl AsRef<Path>,
    options: ProgramInputOptions,
) -> Result<ResolvedProgramInput, ProgramInputError> {
    let ProgramInputOptions {
        root,
        positional_file,
        explicit_file,
    } = options;
    if positional_file.is_some() && explicit_file.is_some() {
        return Err(ProgramInputError::ConflictingFileForms);
    }
    let file = positional_file.or(explicit_file);
    if root.is_some() && file.is_some() {
        return Err(ProgramInputError::RootWithFile);
    }
    let current_directory = canonicalize(current_directory.as_ref())?;
    match file.as_deref() {
        Some(file) => resolve_single_file(&current_directory, file),
        None => resolve_package(&current_directory, root.as_deref()),
    }
}

fn resolve_package(
    current_directory: &Path,
    root: Option<&Path>,
) -> Result<ResolvedProgramInput, ProgramInputError> {
    let selected = absolute_from(current_directory, root.unwrap_or_else(|| Path::new(".")));
    let metadata = metadata(&selected)?;
    if !metadata.is_dir() {
        return Err(ProgramInputError::PackageRootNotDirectory(selected));
    }
    let root = canonicalize(&selected)?;
    let declaration = root.join("nocter.nct");
    match fs::metadata(&declaration) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err(ProgramInputError::PackageDeclarationNotFile(declaration)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ProgramInputError::MissingPackageDeclaration(declaration));
        }
        Err(source) => {
            return Err(ProgramInputError::Filesystem {
                operation: InputOperation::ReadMetadata,
                path: declaration,
                source,
            });
        }
    }
    Ok(ResolvedProgramInput::Package(PackageCommandInput {
        root,
        declaration,
    }))
}

fn resolve_single_file(
    current_directory: &Path,
    source: &Path,
) -> Result<ResolvedProgramInput, ProgramInputError> {
    let selected = absolute_from(current_directory, source);
    if selected
        .extension()
        .is_none_or(|extension| extension != "nct")
    {
        return Err(ProgramInputError::InvalidSourceExtension(selected));
    }
    let metadata = metadata(&selected)?;
    if !metadata.is_file() {
        return Err(ProgramInputError::SourceNotFile(selected));
    }
    Ok(ResolvedProgramInput::SingleFile(SingleFileCommandInput {
        source: canonicalize(&selected)?,
    }))
}

fn absolute_from(current_directory: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        current_directory.join(path)
    }
}

fn metadata(path: &Path) -> Result<fs::Metadata, ProgramInputError> {
    fs::metadata(path).map_err(|source| ProgramInputError::Filesystem {
        operation: InputOperation::ReadMetadata,
        path: path.to_path_buf(),
        source,
    })
}

fn canonicalize(path: &Path) -> Result<PathBuf, ProgramInputError> {
    fs::canonicalize(path).map_err(|source| ProgramInputError::Filesystem {
        operation: InputOperation::Canonicalize,
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputOperation {
    ReadMetadata,
    Canonicalize,
}

impl fmt::Display for InputOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadMetadata => formatter.write_str("read metadata for"),
            Self::Canonicalize => formatter.write_str("canonicalize"),
        }
    }
}

#[derive(Debug)]
pub enum ProgramInputError {
    ConflictingFileForms,
    RootWithFile,
    InvalidSourceExtension(PathBuf),
    PackageRootNotDirectory(PathBuf),
    MissingPackageDeclaration(PathBuf),
    PackageDeclarationNotFile(PathBuf),
    SourceNotFile(PathBuf),
    Filesystem {
        operation: InputOperation,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for ProgramInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConflictingFileForms => {
                formatter.write_str("a positional source and --file cannot be combined")
            }
            Self::RootWithFile => formatter.write_str("--root cannot be combined with file mode"),
            Self::InvalidSourceExtension(path) => {
                write!(
                    formatter,
                    "single-file input {} must end in .nct",
                    path.display()
                )
            }
            Self::PackageRootNotDirectory(path) => {
                write!(
                    formatter,
                    "package root {} is not a directory",
                    path.display()
                )
            }
            Self::MissingPackageDeclaration(path) => {
                write!(formatter, "selected package has no {}", path.display())
            }
            Self::PackageDeclarationNotFile(path) => {
                write!(
                    formatter,
                    "package declaration {} is not a file",
                    path.display()
                )
            }
            Self::SourceNotFile(path) => {
                write!(
                    formatter,
                    "single-file input {} is not a file",
                    path.display()
                )
            }
            Self::Filesystem {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ProgramInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filesystem { source, .. } => Some(source),
            Self::ConflictingFileForms
            | Self::RootWithFile
            | Self::InvalidSourceExtension(_)
            | Self::PackageRootNotDirectory(_)
            | Self::MissingPackageDeclaration(_)
            | Self::PackageDeclarationNotFile(_)
            | Self::SourceNotFile(_) => None,
        }
    }
}
