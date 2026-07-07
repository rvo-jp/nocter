//! Mach-O executable writer.

mod codesign;
mod writer;

pub(crate) use writer::{ExecutableImage, write_arm64_macos_executable_with_data};
