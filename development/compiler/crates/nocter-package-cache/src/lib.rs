//! Exact package cache representation and content-integrity authority.

use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use nocter_content_integrity::{ContentIntegrityError, TreeHashOptions, sha256_regular_tree};

const MANIFEST_NAME: &str = ".nocter-exact-package";
const MANIFEST_FORMAT: &str = "nocter-exact-package-v2";

/// One physical exact-package root whose current tree matches its sealed identity and digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedExactPackageRoot {
    root: PathBuf,
}

impl VerifiedExactPackageRoot {
    /// Returns the verified physical root.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.root
    }

    /// Consumes the capability without discarding its selected path.
    #[must_use]
    pub fn into_path(self) -> PathBuf {
        self.root
    }
}

/// Seals one newly acquired package tree and verifies the completed cache entry.
///
/// # Errors
///
/// Returns an error for an invalid package identity, a missing or unsafe package tree, a reserved
/// manifest collision, an I/O failure, or a tree that changes while it is being sealed.
pub fn seal_exact_package(
    root: &Path,
    identity: &str,
) -> Result<VerifiedExactPackageRoot, ExactPackageCacheError> {
    validate_identity(root, identity)?;
    validate_root_shape(root)?;
    let manifest = root.join(MANIFEST_NAME);
    match fs::symlink_metadata(&manifest) {
        Ok(_) => {
            return Err(invalid(
                &manifest,
                "reserved exact-package manifest already exists",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(io_error(
                "inspect exact-package manifest",
                &manifest,
                source,
            ));
        }
    }
    let tree_digest = hash_tree(root)?;
    let contents = format!("{MANIFEST_FORMAT}\nidentity={identity}\ntree-sha256={tree_digest}\n");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&manifest)
        .map_err(|source| io_error("create exact-package manifest", &manifest, source))?;
    file.write_all(contents.as_bytes())
        .map_err(|source| io_error("write exact-package manifest", &manifest, source))?;
    file.sync_all()
        .map_err(|source| io_error("synchronize exact-package manifest", &manifest, source))?;
    verify_exact_package(root, identity)
}

/// Verifies one existing exact package before it enters a resolution view.
///
/// # Errors
///
/// Returns an error when the root shape, sealed identity, manifest, or current tree content is not
/// exactly the value published by [`seal_exact_package`].
pub fn verify_exact_package(
    root: &Path,
    identity: &str,
) -> Result<VerifiedExactPackageRoot, ExactPackageCacheError> {
    validate_identity(root, identity)?;
    validate_root_shape(root)?;
    let manifest = root.join(MANIFEST_NAME);
    require_regular_file(&manifest, "validate exact-package manifest")?;
    let contents = fs::read_to_string(&manifest)
        .map_err(|source| io_error("read exact-package manifest", &manifest, source))?;
    let sealed = decode_manifest(&manifest, &contents)?;
    if sealed.identity != identity {
        return Err(invalid(
            &manifest,
            "exact-package identity does not match its cache key",
        ));
    }
    let actual = hash_tree(root)?;
    if actual.to_string() != sealed.tree_digest {
        return Err(invalid(
            &manifest,
            "exact-package content does not match its sealed digest",
        ));
    }
    Ok(VerifiedExactPackageRoot {
        root: root.to_path_buf(),
    })
}

fn validate_identity(path: &Path, identity: &str) -> Result<(), ExactPackageCacheError> {
    if identity.is_empty()
        || !identity
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(invalid(path, "invalid exact-package cache identity"));
    }
    Ok(())
}

fn validate_root_shape(root: &Path) -> Result<(), ExactPackageCacheError> {
    require_directory(root)?;
    require_regular_file(
        &root.join("index.nct"),
        "validate exact-package root source",
    )
}

fn require_directory(path: &Path) -> Result<(), ExactPackageCacheError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| io_error("inspect exact-package directory", path, source))?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(invalid(
            path,
            "exact-package root is not a physical directory",
        ))
    }
}

fn require_regular_file(
    path: &Path,
    operation: &'static str,
) -> Result<(), ExactPackageCacheError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|source| io_error(operation, path, source))?;
    if metadata.is_file() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(invalid(
            path,
            "exact-package entry is not a physical regular file",
        ))
    }
}

fn hash_tree(
    root: &Path,
) -> Result<nocter_content_integrity::ContentDigest, ExactPackageCacheError> {
    sha256_regular_tree(
        root,
        TreeHashOptions::excluding_root_entry(OsStr::new(MANIFEST_NAME)),
    )
    .map_err(content_error)
}

fn content_error(error: ContentIntegrityError) -> ExactPackageCacheError {
    match error {
        ContentIntegrityError::Io {
            operation,
            path,
            source,
        } => ExactPackageCacheError::Io {
            operation,
            path,
            source,
        },
        ContentIntegrityError::Invalid { path, reason } => {
            ExactPackageCacheError::Invalid { path, reason }
        }
    }
}

struct SealedManifest<'manifest> {
    identity: &'manifest str,
    tree_digest: &'manifest str,
}

fn decode_manifest<'manifest>(
    path: &Path,
    contents: &'manifest str,
) -> Result<SealedManifest<'manifest>, ExactPackageCacheError> {
    let mut lines = contents.lines();
    if lines.next() != Some(MANIFEST_FORMAT) {
        return Err(invalid(path, "unsupported exact-package manifest format"));
    }
    let identity = lines
        .next()
        .and_then(|line| line.strip_prefix("identity="))
        .ok_or_else(|| invalid(path, "exact-package manifest has no identity"))?;
    let tree_digest = lines
        .next()
        .and_then(|line| line.strip_prefix("tree-sha256="))
        .ok_or_else(|| invalid(path, "exact-package manifest has no tree digest"))?;
    if lines.next().is_some()
        || tree_digest.len() != 64
        || !tree_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(path, "invalid exact-package manifest contents"));
    }
    Ok(SealedManifest {
        identity,
        tree_digest,
    })
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> ExactPackageCacheError {
    ExactPackageCacheError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

fn invalid(path: &Path, reason: &'static str) -> ExactPackageCacheError {
    ExactPackageCacheError::Invalid {
        path: path.to_path_buf(),
        reason,
    }
}

/// Exact package sealing or validation failure.
#[derive(Debug)]
pub enum ExactPackageCacheError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Invalid {
        path: PathBuf,
        reason: &'static str,
    },
}

impl fmt::Display for ExactPackageCacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
            Self::Invalid { path, reason } => {
                write!(
                    formatter,
                    "invalid exact package {}: {reason}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ExactPackageCacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Invalid { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{seal_exact_package, verify_exact_package};

    static NEXT_TREE: AtomicU64 = AtomicU64::new(0);

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "nocter-package-cache-{}-{}",
                std::process::id(),
                NEXT_TREE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::write(
                path.join("index.nct"),
                b"#package: { name: \"example\", version: \"0.0.0\", }\n",
            )
            .unwrap();
            fs::create_dir(path.join("source")).unwrap();
            fs::write(path.join("source/value.nct"), b"pub const VALUE: i32 = 1\n").unwrap();
            Self(path)
        }

        fn root(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn sealed_tree_verifies_for_its_exact_identity() {
        let tree = TempTree::new();
        let identity = "git-7db21c1000000000000000000000000000000000";
        let sealed = seal_exact_package(tree.root(), identity).unwrap();
        assert_eq!(sealed.as_path(), tree.root());
        assert_eq!(verify_exact_package(tree.root(), identity).unwrap(), sealed);
    }

    #[test]
    fn content_or_identity_change_invalidates_the_root() {
        let tree = TempTree::new();
        let identity = "git-7db21c1000000000000000000000000000000000";
        seal_exact_package(tree.root(), identity).unwrap();
        assert!(
            verify_exact_package(tree.root(), "git-8db21c1000000000000000000000000000000000")
                .is_err()
        );
        fs::write(
            tree.root().join("source/value.nct"),
            b"pub const VALUE: i32 = 2\n",
        )
        .unwrap();
        assert!(verify_exact_package(tree.root(), identity).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_content_is_rejected_without_being_followed() {
        use std::os::unix::fs::symlink;

        let tree = TempTree::new();
        symlink("value.nct", tree.root().join("source/alias.nct")).unwrap();
        assert!(
            seal_exact_package(tree.root(), "git-7db21c1000000000000000000000000000000000")
                .is_err()
        );
    }
}
