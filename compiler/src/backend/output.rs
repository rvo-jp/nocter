use crate::diagnostics::Diagnostic;
use crate::target::macho::ExecutableImage;
use std::fs;
use std::path::Path;

pub(crate) fn write_executable_image(
    path: &Path,
    image: &ExecutableImage,
) -> Result<(), Vec<Diagnostic>> {
    fs::write(path, &image.bytes).map_err(|error| {
        vec![Diagnostic::error(
            "E9001",
            format!("failed to write executable `{}`: {error}", path.display()),
        )]
    })?;

    make_executable(path)?;

    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), Vec<Diagnostic>> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = fs::Permissions::from_mode(0o755);
    fs::set_permissions(path, permissions).map_err(|error| {
        vec![Diagnostic::error(
            "E9001",
            format!(
                "failed to mark executable `{}` as executable: {error}",
                path.display()
            ),
        )]
    })
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), Vec<Diagnostic>> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_executable_image_bytes() {
        let root = make_temp_project("write-image");
        let output = root.join("app");
        let image = ExecutableImage {
            bytes: vec![0xcf, 0xfa, 0xed, 0xfe],
        };

        write_executable_image(&output, &image).unwrap();

        assert_eq!(fs::read(&output).unwrap(), image.bytes);
    }

    #[cfg(unix)]
    #[test]
    fn marks_output_as_executable() {
        use std::os::unix::fs::PermissionsExt;

        let root = make_temp_project("write-mode");
        let output = root.join("app");
        let image = ExecutableImage {
            bytes: vec![0xcf, 0xfa, 0xed, 0xfe],
        };

        write_executable_image(&output, &image).unwrap();

        assert_eq!(
            fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }

    fn make_temp_project(name: &str) -> std::path::PathBuf {
        let unique = format!(
            "nocter-output-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        fs::create_dir_all(&root).unwrap();
        root
    }
}
