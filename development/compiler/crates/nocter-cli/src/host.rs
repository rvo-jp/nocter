/// Host identity of the compiler process built for this Rust target.
///
/// This is deliberately separate from the target selected for generated programs.
#[must_use]
pub const fn build_host() -> Option<&'static str> {
    if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
        Some("arm64-darwin")
    } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        Some("x64-linux")
    } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
        Some("arm64-linux")
    } else if cfg!(all(target_arch = "x86_64", target_os = "windows")) {
        Some("x64-windows")
    } else if cfg!(all(target_arch = "aarch64", target_os = "windows")) {
        Some("arm64-windows")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_rust_hosts_project_to_distribution_host_names() {
        if let Some(host) = build_host() {
            assert!(matches!(
                host,
                "arm64-darwin" | "x64-linux" | "arm64-linux" | "x64-windows" | "arm64-windows"
            ));
        }
    }
}
