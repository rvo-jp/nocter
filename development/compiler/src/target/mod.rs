pub mod arm64;
pub mod macho;
pub mod primitive;
pub(crate) mod trusted;
mod trusted_io;

pub const HOST: &str = "arm64-darwin";
pub const DEFAULT_TARGET: &str = HOST;

pub const RESERVED_TARGETS: &[&str] = &["x64-linux", "arm64-linux", "x64-windows", "arm64-windows"];

pub fn validate_requested_target(target: &str) -> Result<(), String> {
    if target == DEFAULT_TARGET {
        return Ok(());
    }

    if RESERVED_TARGETS.contains(&target) {
        return Err(format!(
            "target `{target}` is recognized but not implemented"
        ));
    }

    Err(format!("target `{target}` is not recognized"))
}
