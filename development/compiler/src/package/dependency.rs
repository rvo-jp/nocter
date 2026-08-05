use crate::source::ByteSpan;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyDeclaration {
    name: String,
    span: ByteSpan,
    source: DependencySource,
}

impl DependencyDeclaration {
    pub(super) fn new(name: String, span: ByteSpan, source: DependencySource) -> Self {
        Self { name, span, source }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn span(&self) -> ByteSpan {
        self.span
    }

    pub fn source(&self) -> &DependencySource {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencySource {
    Git { url: String, revision: String },
    Archive { url: String },
    Path { path: String },
}

impl DependencySource {
    pub(super) fn identity_descriptor(
        &self,
        declaring_root: &Path,
        lock: Option<&DependencyLock>,
    ) -> Option<String> {
        match (self, lock) {
            (Self::Git { url, .. }, Some(DependencyLock::GitCommit(commit))) => {
                Some(format!("git:{url}@{commit}"))
            }
            (Self::Archive { url }, Some(DependencyLock::ArchiveSha256(digest))) => {
                Some(format!("archive:{url}@sha256:{digest}"))
            }
            (Self::Path { path }, None) => Some(format!(
                "path:{}",
                declaring_root.join(path).canonicalize().ok()?.display()
            )),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyLock {
    GitCommit(String),
    ArchiveSha256(String),
}

impl DependencyLock {
    pub fn display(&self) -> String {
        match self {
            Self::GitCommit(commit) => format!("git:{commit}"),
            Self::ArchiveSha256(digest) => format!("sha256:{digest}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedDependency {
    name: String,
    span: ByteSpan,
    resolution: DependencyLock,
}

impl LockedDependency {
    pub(super) fn new(name: String, span: ByteSpan, resolution: DependencyLock) -> Self {
        Self {
            name,
            span,
            resolution,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn resolution(&self) -> &DependencyLock {
        &self.resolution
    }

    pub fn span(&self) -> ByteSpan {
        self.span
    }
}
