use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use nocter_archive_extraction::{ArchiveExtractionError, ArchiveLimits, extract_tar_gzip};
use nocter_content_integrity::{ContentDigest, ContentDigestParseError, sha256_bytes};

use crate::{
    CompilerInstallation, InstallationCompatibilityError, NocterHome, NocterHomeError,
    NocterHomeOrigin, canonicalize,
};

const TRANSACTION_MARKER: &[u8] = b"nocter-install-transaction-v1\n";
const TRANSACTION_SUFFIX: &str = ".nocter-install";

/// One explicit, local, digest-bound toolchain installation request.
#[derive(Clone, Debug)]
pub struct ArtifactInstallRequest {
    archive: PathBuf,
    expected_digest: ContentDigest,
    destination: PathBuf,
    active_home: PathBuf,
    compiler_host: Box<str>,
}

impl ArtifactInstallRequest {
    /// Creates an installation request without reading process or filesystem state.
    ///
    /// # Errors
    ///
    /// Returns an invalid trust digest. The digest must be exactly 64 lowercase hexadecimal
    /// characters and is the caller's sole artifact trust input.
    pub fn new(
        archive: impl Into<PathBuf>,
        expected_digest: &str,
        destination: impl Into<PathBuf>,
        active_installation: &CompilerInstallation,
    ) -> Result<Self, ArtifactInstallError> {
        let expected_digest = ContentDigest::from_str(expected_digest)
            .map_err(ArtifactInstallError::InvalidTrustDigest)?;
        Ok(Self {
            archive: archive.into(),
            expected_digest,
            destination: destination.into(),
            active_home: active_installation.root().into(),
            compiler_host: active_installation.manifest().host().into(),
        })
    }
}

/// A completed installation transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactInstallResult {
    destination: PathBuf,
    release: Box<str>,
    archive_digest: ContentDigest,
    replaced: bool,
    retained_transaction: Option<PathBuf>,
}

impl ArtifactInstallResult {
    #[must_use]
    pub fn destination(&self) -> &Path {
        &self.destination
    }

    #[must_use]
    pub fn release(&self) -> &str {
        &self.release
    }

    #[must_use]
    pub const fn archive_digest(&self) -> ContentDigest {
        self.archive_digest
    }

    #[must_use]
    pub const fn replaced(&self) -> bool {
        self.replaced
    }

    /// Returns a transaction directory retained only because old-image cleanup failed after the
    /// new installation had already committed.
    #[must_use]
    pub fn retained_transaction(&self) -> Option<&Path> {
        self.retained_transaction.as_deref()
    }
}

/// Validates and installs one local release archive without performing network acquisition.
///
/// The archive digest is checked before extraction, extraction occurs in a same-parent staging
/// directory, and the candidate home is physically validated before the destination changes.
/// Existing destinations may be replaced only when they are the active validated Nocter home.
///
/// # Errors
///
/// Returns the exact trust, archive, candidate, destination, recovery, or transaction failure.
pub fn install_artifact(
    request: &ArtifactInstallRequest,
) -> Result<ArtifactInstallResult, ArtifactInstallError> {
    let limits = ArchiveLimits::standard();
    let (archive, bytes) = read_archive(&request.archive, limits.maximum_compressed_bytes())?;
    let actual_digest = sha256_bytes(&bytes);
    if actual_digest != request.expected_digest {
        return Err(ArtifactInstallError::ArchiveDigestMismatch {
            path: archive,
            expected: request.expected_digest,
            actual: actual_digest,
        });
    }
    let destination = normalize_destination(&request.destination)?;
    let active_home = canonicalize("canonicalize active Nocter home", &request.active_home)
        .map_err(ArtifactInstallError::ActiveHome)?;
    let transaction_root = transaction_path(&destination)?;
    recover_interrupted_transaction(&transaction_root, &destination, &request.compiler_host)?;
    let existing = inspect_destination(&destination, &active_home)?;

    let mut transaction = InstallTransaction::create(transaction_root)?;
    let stage = transaction.root().join("stage");
    create_private_directory(&stage)
        .map_err(|error| filesystem("create installation stage", &stage, error))?;
    extract_tar_gzip(&bytes, &stage, limits).map_err(ArtifactInstallError::ArchiveExtraction)?;
    let candidate = exact_archive_root(&stage)?;
    let candidate = canonicalize("canonicalize candidate Nocter home", &candidate)
        .map_err(ArtifactInstallError::CandidateHome)?;
    let candidate_home = NocterHome::validate_root(candidate.clone(), NocterHomeOrigin::Configured)
        .map_err(ArtifactInstallError::CandidateHome)?;
    let candidate_installation = candidate_home
        .for_compiler(&request.compiler_host)
        .map_err(ArtifactInstallError::CandidateCompatibility)?;
    let release: Box<str> = candidate_installation.release().into();

    let retained_transaction = if existing {
        commit_replacement(&mut transaction, &candidate, &destination)?
    } else {
        commit_fresh(&mut transaction, &candidate, &destination)?
    };
    Ok(ArtifactInstallResult {
        destination,
        release,
        archive_digest: actual_digest,
        replaced: existing,
        retained_transaction,
    })
}

fn read_archive(
    path: &Path,
    maximum_bytes: u64,
) -> Result<(PathBuf, Vec<u8>), ArtifactInstallError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| filesystem("inspect release archive", path, error))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ArtifactInstallError::ArchiveNotRegular(path.into()));
    }
    let canonical = canonicalize("canonicalize release archive", path)
        .map_err(ArtifactInstallError::ArchivePath)?;
    let file = File::open(&canonical)
        .map_err(|error| filesystem("open release archive", &canonical, error))?;
    let opened = file
        .metadata()
        .map_err(|error| filesystem("inspect opened release archive", &canonical, error))?;
    if !opened.is_file() {
        return Err(ArtifactInstallError::ArchiveNotRegular(canonical));
    }
    let observed_length = metadata.len().max(opened.len());
    if observed_length > maximum_bytes {
        return Err(ArtifactInstallError::ArchiveTooLarge {
            path: canonical,
            bytes: observed_length,
            maximum: maximum_bytes,
        });
    }
    let mut bytes = Vec::new();
    file.take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| filesystem("read release archive", &canonical, error))?;
    let byte_length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if byte_length > maximum_bytes {
        return Err(ArtifactInstallError::ArchiveTooLarge {
            path: canonical,
            bytes: byte_length,
            maximum: maximum_bytes,
        });
    }
    Ok((canonical, bytes))
}

fn normalize_destination(path: &Path) -> Result<PathBuf, ArtifactInstallError> {
    let name = path
        .file_name()
        .ok_or_else(|| ArtifactInstallError::InvalidDestination(path.into()))?;
    let parent = path
        .parent()
        .ok_or_else(|| ArtifactInstallError::InvalidDestination(path.into()))?;
    let parent = canonicalize("canonicalize installation parent", parent)
        .map_err(ArtifactInstallError::Destination)?;
    if !parent.is_dir() {
        return Err(ArtifactInstallError::InvalidDestination(path.into()));
    }
    Ok(parent.join(name))
}

fn inspect_destination(
    destination: &Path,
    active_home: &Path,
) -> Result<bool, ArtifactInstallError> {
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            let existing = canonicalize("canonicalize existing destination", destination)
                .map_err(ArtifactInstallError::Destination)?;
            if existing != active_home {
                return Err(ArtifactInstallError::ExistingDestinationIsNotActive {
                    destination: existing,
                    active: active_home.into(),
                });
            }
            Ok(true)
        }
        Ok(_) => Err(ArtifactInstallError::DestinationNotDirectory(
            destination.into(),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(filesystem(
            "inspect installation destination",
            destination,
            error,
        )),
    }
}

fn transaction_path(destination: &Path) -> Result<PathBuf, ArtifactInstallError> {
    let parent = destination
        .parent()
        .ok_or_else(|| ArtifactInstallError::InvalidDestination(destination.into()))?;
    let name = destination
        .file_name()
        .ok_or_else(|| ArtifactInstallError::InvalidDestination(destination.into()))?;
    let mut transaction_name = OsString::from(".");
    transaction_name.push(name);
    transaction_name.push(TRANSACTION_SUFFIX);
    Ok(parent.join(transaction_name))
}

fn recover_interrupted_transaction(
    transaction: &Path,
    destination: &Path,
    compiler_host: &str,
) -> Result<(), ArtifactInstallError> {
    let metadata = match fs::symlink_metadata(transaction) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(filesystem(
                "inspect prior installation transaction",
                transaction,
                error,
            ));
        }
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ArtifactInstallError::UnrecognizedTransaction(
            transaction.into(),
        ));
    }
    let marker = transaction.join("MARKER");
    if fs::read(&marker).ok().as_deref() != Some(TRANSACTION_MARKER) {
        return Err(ArtifactInstallError::UnrecognizedTransaction(
            transaction.into(),
        ));
    }
    let previous = transaction.join("previous");
    let destination_exists = path_exists(destination, "inspect recovered destination")?;
    let previous_exists = path_exists(&previous, "inspect previous installation")?;
    match (destination_exists, previous_exists) {
        (false, true) => {
            let previous = canonicalize("canonicalize recoverable Nocter home", &previous)
                .map_err(ArtifactInstallError::RecoveryHome)?;
            NocterHome::validate_root(previous.clone(), NocterHomeOrigin::Configured)
                .map_err(ArtifactInstallError::RecoveryHome)?
                .for_compiler(compiler_host)
                .map_err(ArtifactInstallError::RecoveryCompatibility)?;
            fs::rename(&previous, destination).map_err(|error| {
                filesystem("restore interrupted installation", destination, error)
            })?;
            remove_transaction(transaction)?;
            Err(ArtifactInstallError::RecoveredPreviousInstallation(
                destination.into(),
            ))
        }
        (true, _) => {
            let current = canonicalize("canonicalize recovered Nocter home", destination)
                .map_err(ArtifactInstallError::RecoveryHome)?;
            NocterHome::validate_root(current, NocterHomeOrigin::Configured)
                .map_err(ArtifactInstallError::RecoveryHome)?
                .for_compiler(compiler_host)
                .map_err(ArtifactInstallError::RecoveryCompatibility)?;
            remove_transaction(transaction)
        }
        (false, false) => remove_transaction(transaction),
    }
}

fn exact_archive_root(stage: &Path) -> Result<PathBuf, ArtifactInstallError> {
    let mut entries =
        fs::read_dir(stage).map_err(|error| filesystem("read installation stage", stage, error))?;
    let Some(entry) = entries.next() else {
        return Err(ArtifactInstallError::InvalidArchiveRoot);
    };
    let entry = entry.map_err(|error| filesystem("read installation stage", stage, error))?;
    if entry.file_name() != ".nocter" || entries.next().is_some() {
        return Err(ArtifactInstallError::InvalidArchiveRoot);
    }
    let root = entry.path();
    let metadata = fs::symlink_metadata(&root)
        .map_err(|error| filesystem("inspect archive root", &root, error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ArtifactInstallError::InvalidArchiveRoot);
    }
    Ok(root)
}

fn path_exists(path: &Path, operation: &'static str) -> Result<bool, ArtifactInstallError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(filesystem(operation, path, error)),
    }
}

fn commit_fresh(
    transaction: &mut InstallTransaction,
    candidate: &Path,
    destination: &Path,
) -> Result<Option<PathBuf>, ArtifactInstallError> {
    transaction.disarm();
    if let Err(error) = fs::rename(candidate, destination) {
        transaction.arm();
        return Err(filesystem("publish fresh Nocter home", destination, error));
    }
    Ok(transaction.finish())
}

fn commit_replacement(
    transaction: &mut InstallTransaction,
    candidate: &Path,
    destination: &Path,
) -> Result<Option<PathBuf>, ArtifactInstallError> {
    let previous = transaction.root().join("previous");
    transaction.disarm();
    if let Err(error) = fs::rename(destination, &previous) {
        transaction.arm();
        return Err(filesystem("stage previous Nocter home", destination, error));
    }
    if let Err(publication) = fs::rename(candidate, destination) {
        match fs::rename(&previous, destination) {
            Ok(()) => {
                transaction.arm();
                return Err(filesystem(
                    "publish replacement Nocter home",
                    destination,
                    publication,
                ));
            }
            Err(rollback) => {
                return Err(ArtifactInstallError::ReplacementAndRollbackFailed {
                    destination: destination.into(),
                    previous,
                    publication,
                    rollback,
                });
            }
        }
    }
    Ok(transaction.finish())
}

fn remove_transaction(path: &Path) -> Result<(), ArtifactInstallError> {
    fs::remove_dir_all(path)
        .map_err(|error| filesystem("remove installation transaction", path, error))
}

struct InstallTransaction {
    root: PathBuf,
    cleanup_on_drop: bool,
}

impl InstallTransaction {
    fn create(root: PathBuf) -> Result<Self, ArtifactInstallError> {
        create_private_directory(&root)
            .map_err(|error| filesystem("create installation transaction", &root, error))?;
        let marker = root.join("MARKER");
        if let Err(error) = fs::write(&marker, TRANSACTION_MARKER) {
            let _ = fs::remove_dir_all(&root);
            return Err(filesystem(
                "write installation transaction marker",
                &marker,
                error,
            ));
        }
        Ok(Self {
            root,
            cleanup_on_drop: true,
        })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn arm(&mut self) {
        self.cleanup_on_drop = true;
    }

    fn disarm(&mut self) {
        self.cleanup_on_drop = false;
    }

    fn finish(&mut self) -> Option<PathBuf> {
        self.cleanup_on_drop = false;
        fs::remove_dir_all(&self.root)
            .err()
            .map(|_| self.root.clone())
    }
}

impl Drop for InstallTransaction {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
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

fn filesystem(operation: &'static str, path: &Path, source: io::Error) -> ArtifactInstallError {
    ArtifactInstallError::Filesystem {
        operation,
        path: path.into(),
        source,
    }
}

#[derive(Debug)]
pub enum ArtifactInstallError {
    InvalidTrustDigest(ContentDigestParseError),
    ArchiveNotRegular(PathBuf),
    ArchiveTooLarge {
        path: PathBuf,
        bytes: u64,
        maximum: u64,
    },
    ArchivePath(NocterHomeError),
    ArchiveDigestMismatch {
        path: PathBuf,
        expected: ContentDigest,
        actual: ContentDigest,
    },
    ArchiveExtraction(ArchiveExtractionError),
    InvalidArchiveRoot,
    InvalidDestination(PathBuf),
    Destination(NocterHomeError),
    DestinationNotDirectory(PathBuf),
    ExistingDestinationIsNotActive {
        destination: PathBuf,
        active: PathBuf,
    },
    ActiveHome(NocterHomeError),
    CandidateHome(NocterHomeError),
    CandidateCompatibility(InstallationCompatibilityError),
    RecoveryHome(NocterHomeError),
    RecoveryCompatibility(InstallationCompatibilityError),
    UnrecognizedTransaction(PathBuf),
    RecoveredPreviousInstallation(PathBuf),
    ReplacementAndRollbackFailed {
        destination: PathBuf,
        previous: PathBuf,
        publication: io::Error,
        rollback: io::Error,
    },
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for ArtifactInstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTrustDigest(error) => write!(formatter, "invalid --sha256 value: {error}"),
            Self::ArchiveNotRegular(path) => write!(
                formatter,
                "release archive is not a physical regular file: {}",
                path.display()
            ),
            Self::ArchiveTooLarge {
                path,
                bytes,
                maximum,
            } => write!(
                formatter,
                "release archive {} contains {bytes} bytes, exceeding the {maximum}-byte limit",
                path.display()
            ),
            Self::ArchivePath(error)
            | Self::Destination(error)
            | Self::ActiveHome(error)
            | Self::CandidateHome(error)
            | Self::RecoveryHome(error) => error.fmt(formatter),
            Self::ArchiveDigestMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "release archive {} does not match the trusted SHA-256: expected {expected}, actual {actual}",
                path.display()
            ),
            Self::ArchiveExtraction(error) => error.fmt(formatter),
            Self::InvalidArchiveRoot => {
                formatter.write_str("release archive must contain exactly one `.nocter/` root")
            }
            Self::InvalidDestination(path) => {
                write!(
                    formatter,
                    "invalid Nocter home destination: {}",
                    path.display()
                )
            }
            Self::DestinationNotDirectory(path) => write!(
                formatter,
                "Nocter home destination is not a physical directory: {}",
                path.display()
            ),
            Self::ExistingDestinationIsNotActive {
                destination,
                active,
            } => write!(
                formatter,
                "refusing to replace inactive directory {}; the active Nocter home is {}",
                destination.display(),
                active.display()
            ),
            Self::CandidateCompatibility(error) | Self::RecoveryCompatibility(error) => {
                error.fmt(formatter)
            }
            Self::UnrecognizedTransaction(path) => write!(
                formatter,
                "installation transaction path is not owned by Nocter: {}",
                path.display()
            ),
            Self::RecoveredPreviousInstallation(path) => write!(
                formatter,
                "restored the previous Nocter home at {}; rerun the install command",
                path.display()
            ),
            Self::ReplacementAndRollbackFailed {
                destination,
                previous,
                publication,
                rollback,
            } => write!(
                formatter,
                "cannot publish replacement at {} ({publication}) or restore previous home from {} ({rollback})",
                destination.display(),
                previous.display()
            ),
            Self::Filesystem {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for ArtifactInstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidTrustDigest(error) => Some(error),
            Self::ArchivePath(error)
            | Self::Destination(error)
            | Self::ActiveHome(error)
            | Self::CandidateHome(error)
            | Self::RecoveryHome(error) => Some(error),
            Self::ArchiveExtraction(error) => Some(error),
            Self::CandidateCompatibility(error) | Self::RecoveryCompatibility(error) => Some(error),
            Self::Filesystem { source, .. } => Some(source),
            Self::ReplacementAndRollbackFailed { publication, .. } => Some(publication),
            Self::ArchiveNotRegular(_)
            | Self::ArchiveTooLarge { .. }
            | Self::ArchiveDigestMismatch { .. }
            | Self::InvalidArchiveRoot
            | Self::InvalidDestination(_)
            | Self::DestinationNotDirectory(_)
            | Self::ExistingDestinationIsNotActive { .. }
            | Self::UnrecognizedTransaction(_)
            | Self::RecoveredPreviousInstallation(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;
    use nocter_content_integrity::{TreeHashOptions, sha256_file, sha256_regular_tree};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "nocter-artifact-install-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn home(&self, name: &str, release: &str) -> PathBuf {
            let root = self.0.join(name);
            fs::create_dir_all(root.join("std")).unwrap();
            fs::write(root.join("VERSION"), format!("{release}\n")).unwrap();
            fs::write(root.join("nocter"), format!("compiler-{release}")).unwrap();
            fs::write(root.join("LICENSE"), b"license").unwrap();
            fs::write(root.join("NOTICE"), b"notice").unwrap();
            fs::write(
                root.join("std/index.nct"),
                format!("#package: {{ name: \"std\", version: \"{release}\", }}\n"),
            )
            .unwrap();
            let compiler = sha256_file(&root.join("nocter")).unwrap();
            let standard =
                sha256_regular_tree(&root.join("std"), TreeHashOptions::complete()).unwrap();
            fs::write(
                root.join("MANIFEST.json"),
                manifest(release, compiler, standard),
            )
            .unwrap();
            root
        }

        fn archive(&self, name: &str, home: &Path) -> (PathBuf, ContentDigest) {
            let encoder = GzEncoder::new(Vec::new(), Compression::default());
            let mut builder = tar::Builder::new(encoder);
            builder.append_dir(".nocter", home).unwrap();
            builder.append_dir(".nocter/std", home.join("std")).unwrap();
            for relative in [
                "VERSION",
                "nocter",
                "LICENSE",
                "NOTICE",
                "MANIFEST.json",
                "std/index.nct",
            ] {
                builder
                    .append_path_with_name(home.join(relative), Path::new(".nocter").join(relative))
                    .unwrap();
            }
            let bytes = builder.into_inner().unwrap().finish().unwrap();
            let archive = self.0.join(name);
            fs::write(&archive, bytes).unwrap();
            let digest = sha256_file(&archive).unwrap();
            (archive, digest)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn manifest(release: &str, compiler: ContentDigest, standard: ContentDigest) -> String {
        format!(
            r#"{{
  "schema": "nocter.manifest",
  "schema_version": 2,
  "release": "{release}",
  "host": "arm64-darwin",
  "default_target": "arm64-darwin",
  "compiler": {{ "path": "nocter", "sha256": "{compiler}" }},
  "std": {{ "path": "std", "tree_sha256": "{standard}" }},
  "license": {{ "id": "Apache-2.0", "path": "LICENSE", "notice": "NOTICE" }},
  "implemented_targets": [{{
    "name": "arm64-darwin",
    "backend": "arm64",
    "executable": "macho",
    "os": "darwin"
  }}],
  "archive": {{
    "name": "nocter-v{release}-arm64-darwin.tar.gz",
    "root": ".nocter"
  }}
}}"#
        )
    }

    fn request(
        archive: &Path,
        digest: ContentDigest,
        destination: &Path,
        active: &Path,
    ) -> ArtifactInstallRequest {
        let active =
            NocterHome::resolve(crate::NocterHomeRequest::new(None, active.join("nocter")))
                .unwrap()
                .for_compiler("arm64-darwin")
                .unwrap();
        ArtifactInstallRequest::new(archive, &digest.to_string(), destination, &active).unwrap()
    }

    #[test]
    fn installs_a_validated_archive_into_a_fresh_destination() {
        let tree = TempTree::new();
        let active = tree.home("active", "1.0.0");
        let candidate = tree.home("candidate", "2.0.0");
        let (archive, digest) = tree.archive("nocter-v2.0.0-arm64-darwin.tar.gz", &candidate);
        let destination = tree.0.join("installed");

        let request = request(&archive, digest, &destination, &active);
        let result = install_artifact(&request).unwrap();

        assert!(!result.replaced());
        assert_eq!(result.release(), "2.0.0");
        assert_eq!(result.archive_digest(), digest);
        assert_eq!(
            result.destination(),
            fs::canonicalize(&tree.0).unwrap().join("installed")
        );
        assert!(result.retained_transaction().is_none());
        let installed = NocterHome::resolve(crate::NocterHomeRequest::new(
            None,
            destination.join("nocter"),
        ))
        .unwrap();
        assert_eq!(installed.release(), "2.0.0");
    }

    #[test]
    fn replaces_only_the_active_validated_home() {
        let tree = TempTree::new();
        let active = tree.home("active", "1.0.0");
        let candidate = tree.home("candidate", "2.0.0");
        let (archive, digest) = tree.archive("nocter-v2.0.0-arm64-darwin.tar.gz", &candidate);

        let request = request(&archive, digest, &active, &active);
        let result = install_artifact(&request).unwrap();

        assert!(result.replaced());
        assert_eq!(
            fs::read_to_string(active.join("VERSION")).unwrap(),
            "2.0.0\n"
        );
        assert!(!transaction_path(&active).unwrap().exists());
    }

    #[test]
    fn trust_or_destination_failure_leaves_existing_content_unchanged() {
        let tree = TempTree::new();
        let active = tree.home("active", "1.0.0");
        let inactive = tree.home("inactive", "1.0.0");
        let candidate = tree.home("candidate", "2.0.0");
        let (archive, digest) = tree.archive("nocter-v2.0.0-arm64-darwin.tar.gz", &candidate);
        let wrong = ContentDigest::from_bytes([0; 32]);

        assert!(matches!(
            install_artifact(&request(&archive, wrong, &active, &active)),
            Err(ArtifactInstallError::ArchiveDigestMismatch { .. })
        ));
        assert_eq!(
            fs::read_to_string(active.join("VERSION")).unwrap(),
            "1.0.0\n"
        );

        assert!(matches!(
            install_artifact(&request(&archive, digest, &inactive, &active)),
            Err(ArtifactInstallError::ExistingDestinationIsNotActive { .. })
        ));
        assert_eq!(
            fs::read_to_string(inactive.join("VERSION")).unwrap(),
            "1.0.0\n"
        );
    }

    #[test]
    fn recovers_a_previous_home_before_attempting_another_install() {
        let tree = TempTree::new();
        let active = tree.home("active", "1.0.0");
        let candidate = tree.home("candidate", "2.0.0");
        let (archive, digest) = tree.archive("nocter-v2.0.0-arm64-darwin.tar.gz", &candidate);
        let transaction = transaction_path(&active).unwrap();
        fs::create_dir(&transaction).unwrap();
        fs::write(transaction.join("MARKER"), TRANSACTION_MARKER).unwrap();
        fs::rename(&active, transaction.join("previous")).unwrap();

        let request = request(&archive, digest, &active, &transaction.join("previous"));
        let error = install_artifact(&request).unwrap_err();

        assert!(matches!(
            error,
            ArtifactInstallError::RecoveredPreviousInstallation(_)
        ));
        assert_eq!(
            fs::read_to_string(active.join("VERSION")).unwrap(),
            "1.0.0\n"
        );
        assert!(!transaction.exists());
    }

    #[test]
    fn closes_only_recognized_stale_transactions() {
        let tree = TempTree::new();
        let active = tree.home("active", "1.0.0");
        let candidate = tree.home("candidate", "2.0.0");
        let (archive, digest) = tree.archive("nocter-v2.0.0-arm64-darwin.tar.gz", &candidate);
        let transaction = transaction_path(&active).unwrap();
        fs::create_dir(&transaction).unwrap();
        fs::write(transaction.join("foreign"), b"content").unwrap();

        let error = install_artifact(&request(&archive, digest, &active, &active)).unwrap_err();
        assert!(matches!(
            error,
            ArtifactInstallError::UnrecognizedTransaction(_)
        ));
        assert!(transaction.join("foreign").exists());
        assert_eq!(
            fs::read_to_string(active.join("VERSION")).unwrap(),
            "1.0.0\n"
        );

        fs::remove_dir_all(&transaction).unwrap();
        fs::create_dir(&transaction).unwrap();
        fs::write(transaction.join("MARKER"), TRANSACTION_MARKER).unwrap();
        fs::create_dir(transaction.join("stage")).unwrap();

        let result = install_artifact(&request(&archive, digest, &active, &active)).unwrap();
        assert!(result.replaced());
        assert!(!transaction.exists());
        assert_eq!(
            fs::read_to_string(active.join("VERSION")).unwrap(),
            "2.0.0\n"
        );
    }
}
