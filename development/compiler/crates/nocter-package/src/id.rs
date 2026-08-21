use std::fmt;
use std::path::{Path, PathBuf};

use nocter_hash::sha256;
use nocter_model::PackageIdentity;

use crate::{DependencyLock, ExactDependencyLock, ExactDependencyLockKind};

const GIT_PREFIX: &str = "git-";
const ARCHIVE_PREFIX: &str = "sha256-";
const PATH_PREFIX: &str = "path-";

/// Canonical, Windows-safe identity for one exact package selection.
///
/// The same spelling is used as the resolved [`PackageIdentity`] and as the package-store
/// directory name. Display names and source locations are deliberately excluded.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageId(Box<str>);

impl PackageId {
    /// Creates the identity selected by a decoded exact dependency lock.
    ///
    /// # Errors
    ///
    /// Returns an error if a lock was constructed outside the declaration decoder with malformed
    /// exact data.
    pub fn from_lock(lock: &DependencyLock) -> Result<Self, PackageIdError> {
        Self::from_exact_lock(&lock.exact())
    }

    /// Creates the package identity selected by one source-independent exact lock.
    ///
    /// # Errors
    ///
    /// Returns an error only if an exact lock was constructed outside its validated constructors.
    pub fn from_exact_lock(lock: &ExactDependencyLock) -> Result<Self, PackageIdError> {
        match lock.kind() {
            ExactDependencyLockKind::Git => Self::from_git_commit(lock.value()),
            ExactDependencyLockKind::Sha256 => Self::from_archive_digest(lock.value()),
        }
    }

    /// Creates a Git package identity from one exact 40-hex commit.
    ///
    /// # Errors
    ///
    /// Returns an error unless `commit` is exactly 40 ASCII hexadecimal digits.
    pub fn from_git_commit(commit: &str) -> Result<Self, PackageIdError> {
        from_hex(GIT_PREFIX, commit, 40, PackageIdError::InvalidGitCommit)
    }

    /// Creates an archive package identity from one exact SHA-256 content digest.
    ///
    /// # Errors
    ///
    /// Returns an error unless `digest` is exactly 64 ASCII hexadecimal digits.
    pub fn from_archive_digest(digest: &str) -> Result<Self, PackageIdError> {
        from_hex(
            ARCHIVE_PREFIX,
            digest,
            64,
            PackageIdError::InvalidArchiveDigest,
        )
    }

    /// Creates a mutable path-package identity from its canonical absolute path.
    ///
    /// # Errors
    ///
    /// Returns an error if the supplied path is not absolute or has no Unicode representation.
    /// Filesystem canonicalization remains the caller's responsibility so identity generation is
    /// pure and cannot silently select a different package.
    pub fn from_canonical_path(path: &Path) -> Result<Self, PackageIdError> {
        if !path.is_absolute() {
            return Err(PackageIdError::PathNotAbsolute(path.into()));
        }
        let text = path
            .to_str()
            .ok_or_else(|| PackageIdError::NonUnicodePath(path.into()))?;
        let mut value = String::with_capacity(PATH_PREFIX.len() + 64);
        value.push_str(PATH_PREFIX);
        push_lower_hex(&mut value, &sha256(text.as_bytes()));
        Ok(Self(value.into()))
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn package_identity(&self) -> PackageIdentity {
        PackageIdentity::new(self.0.clone())
    }
}

impl From<PackageId> for PackageIdentity {
    fn from(value: PackageId) -> Self {
        Self::new(value.0)
    }
}

fn from_hex(
    prefix: &str,
    hex: &str,
    length: usize,
    error: PackageIdError,
) -> Result<PackageId, PackageIdError> {
    if hex.len() != length || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(error);
    }
    let mut value = String::with_capacity(prefix.len() + length);
    value.push_str(prefix);
    value.extend(hex.chars().map(|character| character.to_ascii_lowercase()));
    Ok(PackageId(value.into()))
}

fn push_lower_hex(output: &mut String, bytes: &[u8]) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageIdError {
    InvalidGitCommit,
    InvalidArchiveDigest,
    PathNotAbsolute(PathBuf),
    NonUnicodePath(PathBuf),
}

impl fmt::Display for PackageIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGitCommit => write!(formatter, "invalid exact Git commit"),
            Self::InvalidArchiveDigest => write!(formatter, "invalid archive SHA-256 digest"),
            Self::PathNotAbsolute(path) => {
                write!(
                    formatter,
                    "package path is not absolute: {}",
                    path.display()
                )
            }
            Self::NonUnicodePath(path) => write!(
                formatter,
                "canonical package path is not Unicode: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for PackageIdError {}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{PackageId, PackageIdError};

    #[test]
    fn exact_remote_ids_are_normalized_and_windows_safe() {
        let git = PackageId::from_git_commit("7DB21C1000000000000000000000000000000000")
            .expect("valid git commit");
        assert_eq!(git.as_str(), "git-7db21c1000000000000000000000000000000000");
        let archive = PackageId::from_archive_digest(
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
        )
        .expect("valid archive digest");
        assert_eq!(
            archive.as_str(),
            "sha256-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert!(
            git.as_str()
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        );
        assert!(
            archive
                .as_str()
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        );
    }

    #[test]
    fn malformed_exact_values_are_rejected() {
        assert_eq!(
            PackageId::from_git_commit("abc"),
            Err(PackageIdError::InvalidGitCommit)
        );
        assert_eq!(
            PackageId::from_archive_digest(
                "z123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            ),
            Err(PackageIdError::InvalidArchiveDigest)
        );
    }

    #[test]
    fn path_id_uses_the_canonical_path_bytes() {
        let package = PackageId::from_canonical_path(Path::new("/work/project"))
            .expect("absolute Unicode path");
        assert_eq!(package.as_str().len(), "path-".len() + 64);
        assert_eq!(
            package,
            PackageId::from_canonical_path(Path::new("/work/project")).unwrap()
        );
        assert_ne!(
            package,
            PackageId::from_canonical_path(Path::new("/work/other")).unwrap()
        );
        assert!(matches!(
            PackageId::from_canonical_path(Path::new("relative")),
            Err(PackageIdError::PathNotAbsolute(_))
        ));
    }
}
