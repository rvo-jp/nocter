use std::fs;
use std::io;
use std::path::Path;

/// Writes a test fixture after creating its containing directory.
///
/// Directory-defined modules make fixture paths intentionally deeper than the
/// old file-module layout. Keeping directory creation here prevents individual
/// tests from encoding incidental filesystem setup.
pub(crate) fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
