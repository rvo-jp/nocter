//! Deterministic ARM64 Mach-O executable image serialization.
//!
//! This crate consumes only a completed [`nocter_arm64::Arm64Program`]. It owns section virtual
//! addresses, Mach-O load commands, the native entry offset, and the ad-hoc code signature.

mod image;
mod sha256;

pub use image::{MachOError, MachOImage};

#[cfg(test)]
mod tests;
