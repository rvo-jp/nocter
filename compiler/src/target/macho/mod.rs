//! Mach-O executable writer.

mod writer;

pub(crate) use writer::{ExecutableImage, write_arm64_macos_executable};
