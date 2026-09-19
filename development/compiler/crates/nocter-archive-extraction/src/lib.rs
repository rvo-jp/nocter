//! Bounded physical extraction of gzip-compressed tar archives.
//!
//! This crate owns archive-entry and destination-filesystem safety. It does not interpret package
//! manifests, release metadata, content trust, or publication transactions.

use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use tar::EntryType;

/// Explicit resource limits for one archive extraction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveLimits {
    compressed_bytes: u64,
    entries: u64,
    expanded_bytes: u64,
    path_components: usize,
}

impl ArchiveLimits {
    #[must_use]
    pub const fn new(
        compressed_bytes: u64,
        entries: u64,
        expanded_bytes: u64,
        path_components: usize,
    ) -> Self {
        Self {
            compressed_bytes,
            entries,
            expanded_bytes,
            path_components,
        }
    }

    #[must_use]
    pub const fn standard() -> Self {
        Self::new(256 * 1024 * 1024, 100_000, 1024 * 1024 * 1024, 64)
    }
}

/// Physical measurements observed while extracting one accepted archive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveSummary {
    entries: u64,
    expanded_bytes: u64,
}

impl ArchiveSummary {
    #[must_use]
    pub const fn entries(self) -> u64 {
        self.entries
    }

    #[must_use]
    pub const fn expanded_bytes(self) -> u64 {
        self.expanded_bytes
    }
}

/// Extracts one gzip-compressed tar stream into an empty physical staging directory.
///
/// # Errors
///
/// Returns an archive-shape, resource-limit, or destination-filesystem failure. Partial staging
/// content may remain on failure; callers own and must discard the unpublished staging directory.
pub fn extract_tar_gzip(
    bytes: &[u8],
    destination: &Path,
    limits: ArchiveLimits,
) -> Result<ArchiveSummary, ArchiveExtractionError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limits.compressed_bytes {
        return Err(invalid(format!(
            "compressed content exceeds {} bytes",
            limits.compressed_bytes
        )));
    }
    require_empty_directory(destination)?;
    let mut archive = tar::Archive::new(GzDecoder::new(bytes));
    let entries = archive
        .entries()
        .map_err(|error| invalid(error.to_string()))?;
    let mut paths = BTreeSet::new();
    let mut entry_count = 0_u64;
    let mut expanded_bytes = 0_u64;

    for entry in entries {
        entry_count = entry_count
            .checked_add(1)
            .ok_or_else(|| invalid("entry count overflow"))?;
        if entry_count > limits.entries {
            return Err(invalid(format!("entry count exceeds {}", limits.entries)));
        }
        let mut entry = entry.map_err(|error| invalid(error.to_string()))?;
        let relative = normalize_path(entry.path_bytes().as_ref(), limits.path_components)?;
        if !paths.insert(relative.clone()) {
            return Err(invalid(format!("duplicate path {:?}", relative.display())));
        }
        let output = destination.join(&relative);
        match entry.header().entry_type() {
            EntryType::Directory => create_directory(&output)?,
            EntryType::Regular => {
                expanded_bytes = expanded_bytes
                    .checked_add(entry.size())
                    .ok_or_else(|| invalid("expanded size overflow"))?;
                if expanded_bytes > limits.expanded_bytes {
                    return Err(invalid(format!(
                        "expanded regular-file data exceeds {} bytes",
                        limits.expanded_bytes
                    )));
                }
                write_regular_file(&mut entry, &output)?;
            }
            _ => {
                return Err(invalid(format!(
                    "unsupported entry type at {:?}",
                    relative.display()
                )));
            }
        }
    }
    Ok(ArchiveSummary {
        entries: entry_count,
        expanded_bytes,
    })
}

fn normalize_path(
    raw: &[u8],
    maximum_components: usize,
) -> Result<PathBuf, ArchiveExtractionError> {
    let authored =
        std::str::from_utf8(raw).map_err(|_| invalid("entry paths must be valid UTF-8"))?;
    if authored.starts_with('/') || authored.starts_with('\\') {
        return Err(invalid("absolute entry path"));
    }
    let raw_components: Vec<_> = authored.split('/').collect();
    let mut components = Vec::new();
    for (index, component) in raw_components.iter().enumerate() {
        if component.is_empty() {
            if index + 1 == raw_components.len() {
                continue;
            }
            return Err(invalid("empty entry path component"));
        }
        if *component == "." {
            continue;
        }
        if *component == ".." {
            return Err(invalid("parent-directory entry path"));
        }
        if component.contains(['\\', '\0', ':']) {
            return Err(invalid("non-portable entry path component"));
        }
        components.push(*component);
    }
    if components.is_empty() {
        return Err(invalid("empty entry path"));
    }
    if components.len() > maximum_components {
        return Err(invalid(format!(
            "entry path exceeds {maximum_components} components"
        )));
    }
    Ok(components.iter().collect())
}

fn require_empty_directory(destination: &Path) -> Result<(), ArchiveExtractionError> {
    let metadata = fs::symlink_metadata(destination)
        .map_err(|error| filesystem("inspect archive destination", destination, error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid("destination is not a physical directory"));
    }
    let mut entries = fs::read_dir(destination)
        .map_err(|error| filesystem("read archive destination", destination, error))?;
    if entries.next().is_some() {
        return Err(invalid("destination is not empty"));
    }
    Ok(())
}

fn create_directory(path: &Path) -> Result<(), ArchiveExtractionError> {
    fs::create_dir_all(path).map_err(|error| filesystem("create archive directory", path, error))
}

fn write_regular_file<R: io::Read>(
    entry: &mut tar::Entry<'_, R>,
    output: &Path,
) -> Result<(), ArchiveExtractionError> {
    let parent = output
        .parent()
        .ok_or_else(|| invalid("regular file has no parent directory"))?;
    create_directory(parent)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| filesystem("create archive file", output, error))?;
    let expected = entry.size();
    let copied = io::copy(entry, &mut file)
        .map_err(|error| filesystem("write archive file", output, error))?;
    if copied != expected {
        return Err(invalid(format!(
            "regular file {:?} declared {expected} bytes but yielded {copied}",
            output.display()
        )));
    }
    set_file_mode(&file, output, entry.header().mode().unwrap_or(0))
}

#[cfg(unix)]
fn set_file_mode(
    file: &fs::File,
    path: &Path,
    authored_mode: u32,
) -> Result<(), ArchiveExtractionError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if authored_mode & 0o111 == 0 {
        0o644
    } else {
        0o755
    };
    file.set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|error| filesystem("set archive file mode", path, error))
}

#[cfg(not(unix))]
fn set_file_mode(
    _file: &fs::File,
    _path: &Path,
    _authored_mode: u32,
) -> Result<(), ArchiveExtractionError> {
    Ok(())
}

fn invalid(reason: impl Into<Box<str>>) -> ArchiveExtractionError {
    ArchiveExtractionError::InvalidArchive(reason.into())
}

fn filesystem(operation: &'static str, path: &Path, source: io::Error) -> ArchiveExtractionError {
    ArchiveExtractionError::Filesystem {
        operation,
        path: path.into(),
        source,
    }
}

#[derive(Debug)]
pub enum ArchiveExtractionError {
    InvalidArchive(Box<str>),
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for ArchiveExtractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArchive(reason) => write!(formatter, "invalid archive: {reason}"),
            Self::Filesystem {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for ArchiveExtractionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filesystem { source, .. } => Some(source),
            Self::InvalidArchive(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "nocter-archive-extraction-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn archive(paths: &[(&str, &[u8])]) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut archive = tar::Builder::new(encoder);
        for (path, bytes) in paths {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive.append_data(&mut header, path, *bytes).unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn materializes_regular_files_and_reports_exact_measurements() {
        let bytes = archive(&[("root/a", b"one"), ("root/b", b"two")]);
        let destination = TempDirectory::new();

        let summary = extract_tar_gzip(&bytes, &destination.0, ArchiveLimits::standard()).unwrap();

        assert_eq!(summary.entries(), 2);
        assert_eq!(summary.expanded_bytes(), 6);
        assert_eq!(fs::read(destination.0.join("root/a")).unwrap(), b"one");
        assert_eq!(fs::read(destination.0.join("root/b")).unwrap(), b"two");
    }

    #[test]
    fn rejects_links_duplicate_normalized_paths_and_unsafe_paths() {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut linked = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(EntryType::Symlink);
        header.set_size(0);
        linked.append_link(&mut header, "alias", "target").unwrap();
        let bytes = linked.into_inner().unwrap().finish().unwrap();
        assert!(
            extract_tar_gzip(&bytes, &TempDirectory::new().0, ArchiveLimits::standard()).is_err()
        );

        let bytes = archive(&[("file", b"one"), ("./file", b"two")]);
        let error = extract_tar_gzip(&bytes, &TempDirectory::new().0, ArchiveLimits::standard())
            .unwrap_err();
        assert!(error.to_string().contains("duplicate path"));

        for path in ["/absolute", "../parent", "folder\\file", "C:/drive"] {
            assert!(normalize_path(path.as_bytes(), 64).is_err(), "{path}");
        }
    }

    #[test]
    fn rejects_resource_limit_and_nonempty_destination() {
        let bytes = archive(&[("a", b"one"), ("b", b"two")]);
        let destination = TempDirectory::new();
        let limits = ArchiveLimits::new(bytes.len() as u64, 1, 1024, 64);
        assert!(extract_tar_gzip(&bytes, &destination.0, limits).is_err());

        let destination = TempDirectory::new();
        fs::write(destination.0.join("existing"), b"content").unwrap();
        assert!(extract_tar_gzip(&bytes, &destination.0, ArchiveLimits::standard()).is_err());
    }
}
