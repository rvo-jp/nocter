use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

/// One persistent executable committed atomically at its final path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentArtifact {
    path: PathBuf,
}

impl PersistentArtifact {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// One private executable whose directory is removed explicitly after a run.
#[derive(Debug)]
pub struct TemporaryArtifact {
    directory: PathBuf,
    executable: PathBuf,
    removed: bool,
}

impl TemporaryArtifact {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.executable
    }

    /// Removes the executable and its private directory.
    ///
    /// # Errors
    ///
    /// Returns a typed filesystem failure if either object cannot be removed.
    pub fn remove(mut self) -> Result<(), ArtifactError> {
        remove_temporary(&self.executable, &self.directory)?;
        self.removed = true;
        Ok(())
    }
}

impl Drop for TemporaryArtifact {
    fn drop(&mut self) {
        if !self.removed {
            let _ = remove_temporary(&self.executable, &self.directory);
        }
    }
}

/// Commits one complete native image without exposing a partially written output.
///
/// # Errors
///
/// Returns the exact create, write, permission, synchronization, or rename operation that failed.
pub fn persist_native_image(
    image: &[u8],
    output: impl AsRef<Path>,
) -> Result<PersistentArtifact, ArtifactError> {
    let output = output.as_ref();
    persist_bytes(image, output)?;
    Ok(PersistentArtifact {
        path: output.to_path_buf(),
    })
}

/// Places one complete native image in a unique private temporary directory.
///
/// # Errors
///
/// Returns the exact directory or executable operation that failed. Any created temporary object
/// is removed before an error is returned.
pub fn stage_temporary_image(image: &[u8]) -> Result<TemporaryArtifact, ArtifactError> {
    let parent = std::env::temp_dir();
    let directory = create_unique_directory(&parent)?;
    let executable = directory.join("program");
    if let Err(error) = persist_bytes(image, &executable) {
        let _ = fs::remove_dir_all(&directory);
        return Err(error);
    }
    Ok(TemporaryArtifact {
        directory,
        executable,
        removed: false,
    })
}

pub(crate) fn persist_bytes(bytes: &[u8], output: &Path) -> Result<(), ArtifactError> {
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = output.file_name().ok_or_else(|| {
        ArtifactError::new(ArtifactOperation::SelectOutput, output, invalid_output())
    })?;
    let (temporary, mut file) = create_unique_file(parent, name)?;
    let result = write_and_commit(&mut file, bytes, &temporary, output);
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_and_commit(
    file: &mut File,
    bytes: &[u8],
    temporary: &Path,
    output: &Path,
) -> Result<(), ArtifactError> {
    file.write_all(bytes)
        .map_err(|error| ArtifactError::new(ArtifactOperation::Write, temporary, error))?;
    make_executable(file, temporary)?;
    file.sync_all()
        .map_err(|error| ArtifactError::new(ArtifactOperation::Synchronize, temporary, error))?;
    fs::rename(temporary, output)
        .map_err(|error| ArtifactError::new(ArtifactOperation::Commit, output, error))
}

#[cfg(unix)]
fn make_executable(file: &File, path: &Path) -> Result<(), ArtifactError> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = file
        .metadata()
        .map_err(|error| ArtifactError::new(ArtifactOperation::ReadMetadata, path, error))?
        .permissions();
    permissions.set_mode(0o755);
    file.set_permissions(permissions)
        .map_err(|error| ArtifactError::new(ArtifactOperation::SetPermissions, path, error))
}

#[cfg(not(unix))]
fn make_executable(_file: &File, path: &Path) -> Result<(), ArtifactError> {
    Err(ArtifactError::new(
        ArtifactOperation::SetPermissions,
        path,
        io::Error::new(
            io::ErrorKind::Unsupported,
            "native artifacts require Unix permissions",
        ),
    ))
}

fn create_unique_file(
    parent: &Path,
    name: &std::ffi::OsStr,
) -> Result<(PathBuf, File), ArtifactError> {
    for _ in 0..128 {
        let id = TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".{}.nocter-{}-{id}.tmp",
            name.to_string_lossy(),
            std::process::id()
        ));
        match open_private_file(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(ArtifactError::new(ArtifactOperation::Create, path, error)),
        }
    }
    Err(ArtifactError::new(
        ArtifactOperation::Create,
        parent,
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary artifact namespace exhausted",
        ),
    ))
}

fn create_unique_directory(parent: &Path) -> Result<PathBuf, ArtifactError> {
    for _ in 0..128 {
        let id = TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!("nocter-run-{}-{id}", std::process::id()));
        match create_private_directory(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(ArtifactError::new(
                    ArtifactOperation::CreateDirectory,
                    path,
                    error,
                ));
            }
        }
    }
    Err(ArtifactError::new(
        ArtifactOperation::CreateDirectory,
        parent,
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary run namespace exhausted",
        ),
    ))
}

fn remove_temporary(executable: &Path, directory: &Path) -> Result<(), ArtifactError> {
    match fs::remove_file(executable) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(ArtifactError::new(
                ArtifactOperation::Remove,
                executable,
                error,
            ));
        }
    }
    fs::remove_dir(directory)
        .map_err(|error| ArtifactError::new(ArtifactOperation::RemoveDirectory, directory, error))
}

#[cfg(unix)]
fn open_private_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_private_file(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private_directory(path: &Path) -> io::Result<()> {
    fs::create_dir(path)
}

fn invalid_output() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "output path has no file name")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactOperation {
    SelectOutput,
    Create,
    Write,
    ReadMetadata,
    SetPermissions,
    Synchronize,
    Commit,
    CreateDirectory,
    Remove,
    RemoveDirectory,
}

#[derive(Debug)]
pub struct ArtifactError {
    operation: ArtifactOperation,
    path: PathBuf,
    source: io::Error,
}

impl ArtifactError {
    fn new(operation: ArtifactOperation, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self {
            operation,
            path: path.into(),
            source,
        }
    }

    #[must_use]
    pub const fn operation(&self) -> ArtifactOperation {
        self.operation
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for ArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native artifact {:?} failed for {}: {}",
            self.operation,
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for ArtifactError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
