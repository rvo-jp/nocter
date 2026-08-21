use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum PackageAcquisitionError {
    Unsupported(Box<str>),
    InvalidUrl {
        url: Box<str>,
        reason: Box<str>,
    },
    Http(reqwest::Error),
    ResponseTooLarge {
        url: Box<str>,
        limit: u64,
    },
    Integrity {
        expected: Box<str>,
        actual: Box<str>,
    },
    InvalidArchive(Box<str>),
    InvalidGit {
        operation: &'static str,
        detail: Box<str>,
    },
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl PackageAcquisitionError {
    pub(crate) fn unsupported(reason: impl Into<Box<str>>) -> Self {
        Self::Unsupported(reason.into())
    }

    pub(crate) fn invalid_url(url: &str, reason: impl Into<Box<str>>) -> Self {
        Self::InvalidUrl {
            url: url.into(),
            reason: reason.into(),
        }
    }

    pub(crate) fn invalid_archive(reason: impl Into<Box<str>>) -> Self {
        Self::InvalidArchive(reason.into())
    }

    pub(crate) fn invalid_git(operation: &'static str, error: impl fmt::Display) -> Self {
        Self::InvalidGit {
            operation,
            detail: error.to_string().into(),
        }
    }

    pub(crate) fn filesystem(
        operation: &'static str,
        path: impl AsRef<Path>,
        source: io::Error,
    ) -> Self {
        Self::Filesystem {
            operation,
            path: path.as_ref().to_owned(),
            source,
        }
    }
}

impl fmt::Display for PackageAcquisitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(reason) => formatter.write_str(reason),
            Self::InvalidUrl { url, reason } => {
                write!(formatter, "unsupported package URL {url:?}: {reason}")
            }
            Self::Http(error) => write!(formatter, "HTTPS package request failed: {error}"),
            Self::ResponseTooLarge { url, limit } => {
                write!(
                    formatter,
                    "package response from {url:?} exceeds {limit} bytes"
                )
            }
            Self::Integrity { expected, actual } => write!(
                formatter,
                "package content does not match its lock: expected {expected}, got {actual}"
            ),
            Self::InvalidArchive(reason) => write!(formatter, "invalid package archive: {reason}"),
            Self::InvalidGit { operation, detail } => {
                write!(formatter, "cannot {operation}: {detail}")
            }
            Self::Filesystem {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for PackageAcquisitionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Http(error) => Some(error),
            Self::Filesystem { source, .. } => Some(source),
            Self::Unsupported(_)
            | Self::InvalidUrl { .. }
            | Self::ResponseTooLarge { .. }
            | Self::Integrity { .. }
            | Self::InvalidArchive(_)
            | Self::InvalidGit { .. } => None,
        }
    }
}

impl From<reqwest::Error> for PackageAcquisitionError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error)
    }
}
