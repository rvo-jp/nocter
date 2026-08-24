use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use nocter_hash::sha256;
use nocter_package::{ExactDependencyLock, ExactDependencyLockKind};
use tar::EntryType;

use crate::PackageAcquisitionError;
use crate::http::MAX_ARCHIVE_BYTES;

const MAX_ENTRIES: u64 = 100_000;
const MAX_EXPANDED_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_PATH_COMPONENTS: usize = 64;

pub(crate) fn archive_lock(bytes: &[u8]) -> Result<ExactDependencyLock, PackageAcquisitionError> {
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err(PackageAcquisitionError::invalid_archive(
            "compressed content exceeds 256 MiB",
        ));
    }
    ExactDependencyLock::sha256(&hex(&sha256(bytes)))
        .map_err(|error| PackageAcquisitionError::invalid_archive(error.to_string()))
}

pub(crate) fn verified_archive(
    bytes: &[u8],
    expected: &ExactDependencyLock,
) -> Result<(), PackageAcquisitionError> {
    if expected.kind() != ExactDependencyLockKind::Sha256 {
        return Err(PackageAcquisitionError::invalid_archive(
            "archive source requires a SHA-256 lock",
        ));
    }
    let actual = archive_lock(bytes)?;
    if &actual == expected {
        Ok(())
    } else {
        Err(PackageAcquisitionError::Integrity {
            expected: expected.literal(),
            actual: actual.literal(),
        })
    }
}

pub(crate) fn extract_archive(
    bytes: &[u8],
    destination: &Path,
) -> Result<(), PackageAcquisitionError> {
    require_empty_directory(destination)?;
    let mut archive = tar::Archive::new(GzDecoder::new(bytes));
    let entries = archive
        .entries()
        .map_err(|error| PackageAcquisitionError::invalid_archive(error.to_string()))?;
    let mut paths = BTreeSet::new();
    let mut entry_count = 0_u64;
    let mut expanded = 0_u64;

    for entry in entries {
        entry_count += 1;
        if entry_count > MAX_ENTRIES {
            return Err(PackageAcquisitionError::invalid_archive(
                "entry count exceeds 100,000",
            ));
        }
        let mut entry =
            entry.map_err(|error| PackageAcquisitionError::invalid_archive(error.to_string()))?;
        let relative = normalize_path(entry.path_bytes().as_ref())?;
        if !paths.insert(relative.clone()) {
            return Err(PackageAcquisitionError::invalid_archive(format!(
                "duplicate path {:?}",
                relative.display()
            )));
        }
        let output = destination.join(&relative);
        match entry.header().entry_type() {
            EntryType::Directory => create_directory(&output)?,
            EntryType::Regular => {
                expanded = expanded.checked_add(entry.size()).ok_or_else(|| {
                    PackageAcquisitionError::invalid_archive("expanded size overflow")
                })?;
                if expanded > MAX_EXPANDED_BYTES {
                    return Err(PackageAcquisitionError::invalid_archive(
                        "expanded regular-file data exceeds 1 GiB",
                    ));
                }
                write_regular_file(&mut entry, &output)?;
            }
            _ => {
                return Err(PackageAcquisitionError::invalid_archive(format!(
                    "unsupported entry type at {:?}",
                    relative.display()
                )));
            }
        }
    }
    require_manifest(destination)
}

fn normalize_path(raw: &[u8]) -> Result<PathBuf, PackageAcquisitionError> {
    let authored = std::str::from_utf8(raw)
        .map_err(|_| PackageAcquisitionError::invalid_archive("entry paths must be valid UTF-8"))?;
    if authored.starts_with('/') || authored.starts_with('\\') {
        return Err(PackageAcquisitionError::invalid_archive(
            "absolute entry path",
        ));
    }
    let raw_components: Vec<_> = authored.split('/').collect();
    let mut components = Vec::new();
    for (index, component) in raw_components.iter().enumerate() {
        if component.is_empty() {
            if index + 1 == raw_components.len() {
                continue;
            }
            return Err(PackageAcquisitionError::invalid_archive(
                "empty entry path component",
            ));
        }
        if *component == "." {
            continue;
        }
        if *component == ".." {
            return Err(PackageAcquisitionError::invalid_archive(
                "parent-directory entry path",
            ));
        }
        if component.contains(['\\', '\0']) {
            return Err(PackageAcquisitionError::invalid_archive(
                "non-portable entry path component",
            ));
        }
        components.push(*component);
    }
    if components.is_empty() {
        return Err(PackageAcquisitionError::invalid_archive("empty entry path"));
    }
    if components.len() > MAX_PATH_COMPONENTS {
        return Err(PackageAcquisitionError::invalid_archive(
            "entry path exceeds 64 components",
        ));
    }
    Ok(components.iter().collect())
}

fn require_empty_directory(destination: &Path) -> Result<(), PackageAcquisitionError> {
    let metadata = fs::symlink_metadata(destination).map_err(|error| {
        PackageAcquisitionError::filesystem("inspect archive destination", destination, error)
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(PackageAcquisitionError::invalid_archive(
            "destination is not a physical directory",
        ));
    }
    let mut entries = fs::read_dir(destination).map_err(|error| {
        PackageAcquisitionError::filesystem("read archive destination", destination, error)
    })?;
    if entries.next().is_some() {
        return Err(PackageAcquisitionError::invalid_archive(
            "destination is not empty",
        ));
    }
    Ok(())
}

fn create_directory(path: &Path) -> Result<(), PackageAcquisitionError> {
    match fs::create_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) => Err(PackageAcquisitionError::filesystem(
            "create archive directory",
            path,
            error,
        )),
    }
}

fn write_regular_file<R: io::Read>(
    entry: &mut tar::Entry<'_, R>,
    output: &Path,
) -> Result<(), PackageAcquisitionError> {
    let parent = output.parent().ok_or_else(|| {
        PackageAcquisitionError::invalid_archive("regular file has no parent directory")
    })?;
    create_directory(parent)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| {
            PackageAcquisitionError::filesystem("create archive file", output, error)
        })?;
    io::copy(entry, &mut file).map_err(|error| {
        PackageAcquisitionError::filesystem("write archive file", output, error)
    })?;
    set_file_mode(&file, output, entry.header().mode().unwrap_or(0))
}

#[cfg(unix)]
fn set_file_mode(
    file: &fs::File,
    path: &Path,
    authored_mode: u32,
) -> Result<(), PackageAcquisitionError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if authored_mode & 0o111 == 0 {
        0o644
    } else {
        0o755
    };
    file.set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|error| PackageAcquisitionError::filesystem("set archive file mode", path, error))
}

#[cfg(not(unix))]
fn set_file_mode(
    _file: &fs::File,
    _path: &Path,
    _authored_mode: u32,
) -> Result<(), PackageAcquisitionError> {
    Ok(())
}

fn require_manifest(destination: &Path) -> Result<(), PackageAcquisitionError> {
    let manifest = destination.join("index.nct");
    match fs::symlink_metadata(&manifest) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(PackageAcquisitionError::invalid_archive(
            "archive-root index.nct is not a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(
            PackageAcquisitionError::invalid_archive("archive root does not contain index.nct"),
        ),
        Err(error) => Err(PackageAcquisitionError::filesystem(
            "inspect archive manifest",
            manifest,
            error,
        )),
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
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
                "nocter-archive-test-{}-{}",
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

    fn package_archive(path: &str) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut archive = tar::Builder::new(encoder);
        append_file(&mut archive, path);
        archive.into_inner().unwrap().finish().unwrap()
    }

    fn append_file(archive: &mut tar::Builder<GzEncoder<Vec<u8>>>, path: &str) {
        let bytes = b"#package: { name: \"fixture\", version: \"0.0.0\", }\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append_data(&mut header, path, &bytes[..]).unwrap();
    }

    #[test]
    fn verifies_compressed_bytes_and_extracts_root_manifest() {
        let bytes = package_archive("index.nct");
        let lock = archive_lock(&bytes).unwrap();
        let destination = TempDirectory::new();
        verified_archive(&bytes, &lock).unwrap();
        extract_archive(&bytes, &destination.0).unwrap();
        assert_eq!(
            fs::read_to_string(destination.0.join("index.nct")).unwrap(),
            "#package: { name: \"fixture\", version: \"0.0.0\", }\n"
        );
    }

    #[test]
    fn does_not_strip_an_enclosing_directory() {
        let bytes = package_archive("package/index.nct");
        let destination = TempDirectory::new();
        let error = extract_archive(&bytes, &destination.0).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("archive root does not contain index.nct")
        );
    }

    #[test]
    fn rejects_a_lock_for_different_compressed_bytes() {
        let original = package_archive("index.nct");
        let mut changed = original.clone();
        changed[0] ^= 1;
        let lock = archive_lock(&original).unwrap();
        let error = verified_archive(&changed, &lock).unwrap_err();
        assert!(matches!(error, PackageAcquisitionError::Integrity { .. }));
    }

    #[test]
    fn rejects_links_and_duplicate_normalized_paths() {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut linked = tar::Builder::new(encoder);
        append_file(&mut linked, "index.nct");
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(EntryType::Symlink);
        header.set_size(0);
        linked
            .append_link(&mut header, "alias", "index.nct")
            .unwrap();
        let bytes = linked.into_inner().unwrap().finish().unwrap();
        let destination = TempDirectory::new();
        assert!(extract_archive(&bytes, &destination.0).is_err());

        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut duplicate = tar::Builder::new(encoder);
        append_file(&mut duplicate, "index.nct");
        append_file(&mut duplicate, "./index.nct");
        let bytes = duplicate.into_inner().unwrap().finish().unwrap();
        let destination = TempDirectory::new();
        let error = extract_archive(&bytes, &destination.0).unwrap_err();
        assert!(error.to_string().contains("duplicate path"));
    }

    #[test]
    fn rejects_absolute_parent_and_backslash_paths_before_writing() {
        assert!(normalize_path(b"/index.nct").is_err());
        assert!(normalize_path(b"../index.nct").is_err());
        assert!(normalize_path(b"folder\\index.nct").is_err());
    }
}
