use std::fmt;

/// One exact dependency selection independent of its authored source location.
///
/// Package declarations retain syntax-bearing [`crate::DependencyLock`] values. Resolution and
/// package-state transactions use this value so a provisional lock can validate a complete graph
/// before it is committed to `nocter.nct`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExactDependencyLock {
    kind: ExactDependencyLockKind,
    value: Box<str>,
}

impl ExactDependencyLock {
    /// Creates an exact Git lock from a 40-digit hexadecimal commit.
    ///
    /// # Errors
    ///
    /// Returns [`ExactDependencyLockError::InvalidGitCommit`] for any other spelling.
    pub fn git(commit: &str) -> Result<Self, ExactDependencyLockError> {
        validate_hex(commit, 40)
            .then(|| Self::validated(ExactDependencyLockKind::Git, commit))
            .ok_or(ExactDependencyLockError::InvalidGitCommit)
    }

    /// Creates an exact archive lock from a 64-digit hexadecimal SHA-256 digest.
    ///
    /// # Errors
    ///
    /// Returns [`ExactDependencyLockError::InvalidArchiveDigest`] for any other spelling.
    pub fn sha256(digest: &str) -> Result<Self, ExactDependencyLockError> {
        validate_hex(digest, 64)
            .then(|| Self::validated(ExactDependencyLockKind::Sha256, digest))
            .ok_or(ExactDependencyLockError::InvalidArchiveDigest)
    }

    pub(crate) fn validated(kind: ExactDependencyLockKind, value: &str) -> Self {
        Self {
            kind,
            value: value.to_ascii_lowercase().into(),
        }
    }

    #[must_use]
    pub const fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub const fn kind(&self) -> ExactDependencyLockKind {
        self.kind
    }

    #[must_use]
    pub fn literal(&self) -> Box<str> {
        format!("{}:{}", self.kind().prefix(), self.value()).into()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExactDependencyLockKind {
    Git,
    Sha256,
}

impl ExactDependencyLockKind {
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Sha256 => "sha256",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactDependencyLockError {
    InvalidGitCommit,
    InvalidArchiveDigest,
}

impl fmt::Display for ExactDependencyLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGitCommit => {
                formatter.write_str("Git lock must be exactly 40 hexadecimal digits")
            }
            Self::InvalidArchiveDigest => {
                formatter.write_str("archive lock must be exactly 64 hexadecimal digits")
            }
        }
    }
}

impl std::error::Error for ExactDependencyLockError {}

fn validate_hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_locks_normalize_case_without_source_identity() {
        let git = ExactDependencyLock::git("7DB21C1000000000000000000000000000000000").unwrap();
        let archive = ExactDependencyLock::sha256(
            "ABCDEF0000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        assert_eq!(
            git.literal().as_ref(),
            "git:7db21c1000000000000000000000000000000000"
        );
        assert_eq!(
            archive.literal().as_ref(),
            "sha256:abcdef0000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(git.kind(), ExactDependencyLockKind::Git);
        assert_eq!(archive.kind(), ExactDependencyLockKind::Sha256);
    }
}
