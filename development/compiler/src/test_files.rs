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

/// Writes the files that make a test home a versioned toolchain home.
///
/// Tests may add only the standard-library modules relevant to their scenario,
/// but package loading always relies on the same root contract as an installed
/// toolchain.
pub(crate) fn write_standard_package(home: impl AsRef<Path>) -> io::Result<()> {
    let home = home.as_ref();
    let standard_library = home.join("std");
    fs::create_dir_all(&standard_library)?;
    write(home.join("VERSION"), env!("CARGO_PKG_VERSION"))?;
    write(
        standard_library.join("nocter.nct"),
        format!(
            "#name: \"std\"\n#version: \"{}\"\n",
            env!("CARGO_PKG_VERSION")
        ),
    )?;
    write(
        standard_library.join("index.nct"),
        "//! Test standard library.\n",
    )
}
