//! Deterministic physical file and regular-tree content identities.

use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use nocter_hash::Sha256;

const BUFFER_SIZE: usize = 64 * 1024;
const TREE_FORMAT: &[u8] = b"nocter-regular-tree-v1\0";

/// One exact SHA-256 content identity rendered as lowercase hexadecimal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentDigest([u8; 32]);

impl ContentDigest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for ContentDigest {
    type Err = ContentDigestParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != 64 {
            return Err(ContentDigestParseError);
        }
        let mut bytes = [0_u8; 32];
        for (destination, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
            *destination = (hex_nibble(pair[0]).ok_or(ContentDigestParseError)? << 4)
                | hex_nibble(pair[1]).ok_or(ContentDigestParseError)?;
        }
        Ok(Self(bytes))
    }
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentDigestParseError;

impl fmt::Display for ContentDigestParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("digest must contain exactly 64 lowercase hexadecimal characters")
    }
}

impl std::error::Error for ContentDigestParseError {}

/// Root-entry selection for a deterministic regular-tree digest.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TreeHashOptions<'name> {
    excluded_root_entry: Option<&'name OsStr>,
}

impl<'name> TreeHashOptions<'name> {
    #[must_use]
    pub const fn complete() -> Self {
        Self {
            excluded_root_entry: None,
        }
    }

    /// Excludes exactly one direct child of the root from the digest.
    #[must_use]
    pub fn excluding_root_entry(name: &'name OsStr) -> Self {
        Self {
            excluded_root_entry: Some(name),
        }
    }
}

/// Hashes one physical regular file without following a symlink at `path`.
///
/// # Errors
///
/// Returns an I/O or physical-shape failure, including a file length change during the read.
pub fn sha256_file(path: &Path) -> Result<ContentDigest, ContentIntegrityError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| io_error("inspect content file", path, source))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(invalid(path, "content path is not a physical regular file"));
    }
    let mut digest = Sha256::new();
    hash_file_bytes(path, metadata.len(), &mut digest)?;
    Ok(ContentDigest::from_bytes(digest.finish()))
}

/// Hashes one physical tree using normalized Unicode relative paths and deterministic ordering.
///
/// # Errors
///
/// Returns an I/O or physical-shape failure for the root or any retained descendant.
pub fn sha256_regular_tree(
    root: &Path,
    options: TreeHashOptions<'_>,
) -> Result<ContentDigest, ContentIntegrityError> {
    require_physical_directory(root)?;
    let mut digest = Sha256::new();
    digest.update(TREE_FORMAT);
    hash_directory(root, Path::new(""), options, &mut digest)?;
    Ok(ContentDigest::from_bytes(digest.finish()))
}

fn require_physical_directory(path: &Path) -> Result<(), ContentIntegrityError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| io_error("inspect content directory", path, source))?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(invalid(
            path,
            "content tree root is not a physical directory",
        ))
    }
}

fn hash_directory(
    root: &Path,
    relative: &Path,
    options: TreeHashOptions<'_>,
    digest: &mut Sha256,
) -> Result<(), ContentIntegrityError> {
    let directory = root.join(relative);
    let entries = fs::read_dir(&directory)
        .map_err(|source| io_error("read content directory", &directory, source))?;
    let mut entries = entries
        .map(|entry| {
            let entry =
                entry.map_err(|source| io_error("read content entry", &directory, source))?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| invalid(&entry.path(), "content tree entry name is not Unicode"))?;
            Ok(name.to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_unstable();
    for name in entries {
        if relative.as_os_str().is_empty() && options.excluded_root_entry == Some(OsStr::new(&name))
        {
            continue;
        }
        let child_relative = relative.join(&name);
        let child = root.join(&child_relative);
        let metadata = fs::symlink_metadata(&child)
            .map_err(|source| io_error("inspect content entry", &child, source))?;
        let normalized = normalized_relative(&child, &child_relative)?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            hash_record(digest, b'D', normalized.as_bytes(), 0);
            hash_directory(root, &child_relative, options, digest)?;
        } else if metadata.is_file() && !metadata.file_type().is_symlink() {
            hash_record(digest, b'F', normalized.as_bytes(), metadata.len());
            hash_file_bytes(&child, metadata.len(), digest)?;
        } else {
            return Err(invalid(
                &child,
                "content tree contains a symlink or special file",
            ));
        }
    }
    Ok(())
}

fn normalized_relative(path: &Path, relative: &Path) -> Result<String, ContentIntegrityError> {
    let mut normalized = String::new();
    for component in relative.components() {
        let Some(component) = component.as_os_str().to_str() else {
            return Err(invalid(path, "content tree path is not Unicode"));
        };
        if !normalized.is_empty() {
            normalized.push('/');
        }
        normalized.push_str(component);
    }
    Ok(normalized)
}

fn hash_record(digest: &mut Sha256, kind: u8, path: &[u8], byte_length: u64) {
    digest.update(&[kind]);
    digest.update(&(path.len() as u64).to_be_bytes());
    digest.update(path);
    digest.update(&byte_length.to_be_bytes());
}

fn hash_file_bytes(
    path: &Path,
    expected_length: u64,
    digest: &mut Sha256,
) -> Result<(), ContentIntegrityError> {
    let mut file =
        File::open(path).map_err(|source| io_error("open content file", path, source))?;
    let mut buffer = vec![0_u8; BUFFER_SIZE].into_boxed_slice();
    let mut actual_length = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| io_error("read content file", path, source))?;
        if read == 0 {
            break;
        }
        actual_length = actual_length
            .checked_add(read as u64)
            .ok_or_else(|| invalid(path, "content file length overflowed"))?;
        digest.update(&buffer[..read]);
    }
    if actual_length != expected_length {
        return Err(invalid(path, "content file changed while it was hashed"));
    }
    Ok(())
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> ContentIntegrityError {
    ContentIntegrityError::Io {
        operation,
        path: path.into(),
        source,
    }
}

fn invalid(path: &Path, reason: &'static str) -> ContentIntegrityError {
    ContentIntegrityError::Invalid {
        path: path.into(),
        reason,
    }
}

#[derive(Debug)]
pub enum ContentIntegrityError {
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

impl fmt::Display for ContentIntegrityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
            Self::Invalid { path, reason } => {
                write!(formatter, "invalid content at {}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for ContentIntegrityError {
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
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "nocter-content-integrity-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn digest_text_is_exact_lowercase_sha256() {
        let digest = ContentDigest::from_str(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        .unwrap();
        assert_eq!(
            digest.to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(
            ContentDigest::from_str(
                "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
            )
            .is_err()
        );
    }

    #[test]
    fn file_and_tree_hashes_are_content_and_path_sensitive() {
        let tree = TempTree::new();
        fs::write(tree.0.join("a"), b"abc").unwrap();
        fs::create_dir(tree.0.join("nested")).unwrap();
        fs::write(tree.0.join("nested/b"), b"value").unwrap();
        assert_eq!(
            sha256_file(&tree.0.join("a")).unwrap().to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let original = sha256_regular_tree(&tree.0, TreeHashOptions::complete()).unwrap();
        fs::rename(tree.0.join("nested/b"), tree.0.join("nested/c")).unwrap();
        let renamed = sha256_regular_tree(&tree.0, TreeHashOptions::complete()).unwrap();
        assert_ne!(original, renamed);
    }

    #[test]
    fn root_exclusion_is_exact_and_does_not_hide_descendants() {
        let tree = TempTree::new();
        fs::write(tree.0.join("seal"), b"first").unwrap();
        fs::create_dir(tree.0.join("nested")).unwrap();
        fs::write(tree.0.join("nested/seal"), b"retained").unwrap();
        let options = TreeHashOptions::excluding_root_entry(OsStr::new("seal"));
        let original = sha256_regular_tree(&tree.0, options).unwrap();
        fs::write(tree.0.join("seal"), b"second").unwrap();
        assert_eq!(sha256_regular_tree(&tree.0, options).unwrap(), original);
        fs::write(tree.0.join("nested/seal"), b"changed").unwrap();
        assert_ne!(sha256_regular_tree(&tree.0, options).unwrap(), original);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_never_followed() {
        use std::os::unix::fs::symlink;

        let tree = TempTree::new();
        fs::write(tree.0.join("value"), b"value").unwrap();
        symlink("value", tree.0.join("alias")).unwrap();
        assert!(sha256_file(&tree.0.join("alias")).is_err());
        assert!(sha256_regular_tree(&tree.0, TreeHashOptions::complete()).is_err());
    }
}
