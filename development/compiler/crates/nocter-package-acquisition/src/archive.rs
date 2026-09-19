use std::fs;
use std::io;
use std::path::Path;

use nocter_archive_extraction::{ArchiveLimits, extract_tar_gzip};
use nocter_hash::sha256;
use nocter_package::{ExactDependencyLock, ExactDependencyLockKind};

use crate::PackageAcquisitionError;
use crate::http::MAX_ARCHIVE_BYTES;

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
    extract_tar_gzip(bytes, destination, ArchiveLimits::standard())
        .map_err(PackageAcquisitionError::ArchiveExtraction)?;
    require_manifest(destination)
}

fn require_manifest(destination: &Path) -> Result<(), PackageAcquisitionError> {
    let manifest = destination.join(nocter_language::MODULE_ROOT_FILE_NAME);
    match fs::symlink_metadata(&manifest) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(PackageAcquisitionError::invalid_archive(format!(
            "archive-root {} is not a regular file",
            nocter_language::MODULE_ROOT_FILE_NAME
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(PackageAcquisitionError::invalid_archive(format!(
                "archive root does not contain {}",
                nocter_language::MODULE_ROOT_FILE_NAME
            )))
        }
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
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::EntryType;

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
}
