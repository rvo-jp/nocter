use crate::diagnostics::Diagnostic;
use crate::target::macho::ExecutableImage;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn write_executable_image(
    path: &Path,
    image: &ExecutableImage,
) -> Result<(), Vec<Diagnostic>> {
    let temporary_path = temporary_output_path(path);

    fs::write(&temporary_path, &image.bytes).map_err(|error| {
        vec![Diagnostic::error(
            "E9001",
            format!("failed to write executable `{}`: {error}", path.display()),
        )]
    })?;

    if let Err(diagnostics) = make_executable(&temporary_path) {
        remove_temporary_output(&temporary_path);
        return Err(diagnostics);
    }

    if let Err(error) = fs::rename(&temporary_path, path) {
        remove_temporary_output(&temporary_path);
        return Err(vec![Diagnostic::error(
            "E9001",
            format!("failed to replace executable `{}`: {error}", path.display()),
        )]);
    }

    Ok(())
}

fn temporary_output_path(path: &Path) -> PathBuf {
    let file_name = path.file_name().unwrap_or_else(|| OsStr::new("a.out"));
    let temporary_name = format!(
        ".{}.{}.tmp",
        file_name.to_string_lossy(),
        unique_temporary_suffix()
    );

    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(temporary_name),
        _ => PathBuf::from(temporary_name),
    }
}

fn unique_temporary_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}", std::process::id())
}

fn remove_temporary_output(path: &Path) {
    let _ = fs::remove_file(path);
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

    #[test]
    fn replaces_existing_output_on_success() {
        let root = make_temp_project("replace-image");
        let output = root.join("app");
        fs::write(&output, b"old executable").unwrap();
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
