//! Installed Nocter home resolution and validation.

mod manifest;

use crate::target::{DEFAULT_TARGET, HOST};
use manifest::Manifest;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Component;
use std::path::{Path, PathBuf};

const MANIFEST_SCHEMA: &str = "nocter.manifest";
const MANIFEST_SCHEMA_VERSION: u32 = 1;

pub(crate) fn resolve_nocter_home() -> Result<PathBuf, String> {
    if let Some(home) = env::var_os("NOCTER_HOME") {
        return Ok(PathBuf::from(home));
    }

    let exe = env::current_exe()
        .map_err(|error| format!("failed to resolve running nocter executable: {error}"))?;
    let resolved = exe
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize running nocter executable: {error}"))?;
    resolved
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "running nocter executable has no parent directory".to_string())
}

pub(crate) fn validate_nocter_home(home: &Path) -> Vec<String> {
    let mut errors = Vec::new();

    if !home.is_dir() {
        errors.push(format!(
            "Nocter home is not a directory `{}`",
            home.display()
        ));
        return errors;
    }

    let version = match read_version_file(&home.join("VERSION")) {
        Ok(version) => Some(version),
        Err(error) => {
            errors.push(error);
            None
        }
    };

    let manifest = match manifest::load_manifest(&home.join("MANIFEST.json")) {
        Ok(manifest) => Some(manifest),
        Err(error) => {
            errors.push(error);
            None
        }
    };

    require_dir(home, "std", &mut errors);
    require_file(home, "std/nocter.nct", &mut errors);
    require_file(home, "std/index.nct", &mut errors);
    require_file(home, "LICENSE", &mut errors);
    require_file(home, "NOTICE", &mut errors);

    if home.join("std/nocter.nct").is_file() && home.join("std/index.nct").is_file() {
        let load = crate::package::load_package(&home.join("std"));
        if load.diagnostics.is_empty() {
            if let Some(package) = load.package.as_ref() {
                errors.extend(crate::package::standard_library::validation_errors(
                    package,
                    version.as_deref(),
                ));
            }
        } else {
            errors.extend(load.diagnostics.into_iter().map(|diagnostic| {
                format!(
                    "invalid toolchain standard-library package: {}",
                    diagnostic.message
                )
            }));
        }
    }

    if let (Some(version), Some(manifest)) = (version.as_deref(), manifest.as_ref()) {
        validate_manifest(home, version, manifest, &mut errors);
    }

    errors
}

fn require_dir(home: &Path, relative: &str, errors: &mut Vec<String>) {
    let path = home.join(relative);
    if !path.is_dir() {
        errors.push(format!("missing directory `{}`", path.display()));
    }
}

fn require_file(home: &Path, relative: &str, errors: &mut Vec<String>) {
    let path = home.join(relative);
    if !path.is_file() {
        errors.push(format!("missing file `{}`", path.display()));
    }
}

fn read_version_file(path: &Path) -> Result<String, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read `{}`: {error}", path.display()))?;
    let mut lines = text.lines();
    let Some(version) = lines.next() else {
        return Err(format!("`{}` is empty", path.display()));
    };

    if lines.next().is_some() {
        return Err(format!(
            "`{}` must contain exactly one line",
            path.display()
        ));
    }

    if version.trim() != version {
        return Err(format!(
            "`{}` must not contain leading or trailing whitespace",
            path.display()
        ));
    }

    if !is_valid_release_version(version) {
        return Err(format!(
            "`{}` contains invalid release version `{version}`",
            path.display()
        ));
    }

    Ok(version.to_string())
}

pub(crate) fn read_nocter_home_version(home: &Path) -> Result<String, String> {
    read_version_file(&home.join("VERSION"))
}

fn is_valid_release_version(version: &str) -> bool {
    if version.is_empty() || version.starts_with('v') {
        return false;
    }

    let (core, prerelease) = match version.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease)),
        None => (version, None),
    };

    let mut parts = core.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let Some(patch) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }

    let numeric = [major, minor, patch]
        .into_iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()));
    if !numeric {
        return false;
    }

    match prerelease {
        Some(part) => {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
        }
        None => true,
    }
}

fn validate_manifest(home: &Path, version: &str, manifest: &Manifest, errors: &mut Vec<String>) {
    if manifest.schema != MANIFEST_SCHEMA {
        errors.push(format!(
            "MANIFEST.json schema must be `{MANIFEST_SCHEMA}`, got `{}`",
            manifest.schema
        ));
    }

    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        errors.push(format!(
            "MANIFEST.json schema_version must be {MANIFEST_SCHEMA_VERSION}, got {}",
            manifest.schema_version
        ));
    }

    if manifest.release != version {
        errors.push(format!(
            "MANIFEST.json release `{}` does not match VERSION `{version}`",
            manifest.release
        ));
    }

    if manifest.host != HOST {
        errors.push(format!(
            "MANIFEST.json host must be `{HOST}`, got `{}`",
            manifest.host
        ));
    }

    if manifest.default_target != DEFAULT_TARGET {
        errors.push(format!(
            "MANIFEST.json default_target must be `{DEFAULT_TARGET}`, got `{}`",
            manifest.default_target
        ));
    }

    if manifest.compiler.path.as_path() != Path::new("nocter") {
        errors.push("MANIFEST.json compiler.path must be `nocter`".to_string());
    }
    validate_relative_path("compiler.path", &manifest.compiler.path, errors);

    if manifest.std.path.as_path() != Path::new("std") {
        errors.push("MANIFEST.json std.path must be `std`".to_string());
    }
    validate_relative_path("std.path", &manifest.std.path, errors);
    if !home.join(&manifest.std.path).is_dir() {
        errors.push(format!(
            "std.path directory is missing `{}`",
            home.join(&manifest.std.path).display()
        ));
    }

    if manifest.license.id != "Apache-2.0" {
        errors.push(format!(
            "MANIFEST.json license.id must be `Apache-2.0`, got `{}`",
            manifest.license.id
        ));
    }
    if manifest.license.path.as_path() != Path::new("LICENSE") {
        errors.push("MANIFEST.json license.path must be `LICENSE`".to_string());
    }
    validate_relative_path("license.path", &manifest.license.path, errors);
    if !home.join(&manifest.license.path).is_file() {
        errors.push(format!(
            "license.path file is missing `{}`",
            home.join(&manifest.license.path).display()
        ));
    }
    if manifest.license.notice.as_path() != Path::new("NOTICE") {
        errors.push("MANIFEST.json license.notice must be `NOTICE`".to_string());
    }
    validate_relative_path("license.notice", &manifest.license.notice, errors);
    if !home.join(&manifest.license.notice).is_file() {
        errors.push(format!(
            "license.notice file is missing `{}`",
            home.join(&manifest.license.notice).display()
        ));
    }

    let mut names = HashSet::new();
    for target in &manifest.implemented_targets {
        if !names.insert(target.name.as_str()) {
            errors.push(format!("duplicate implemented target `{}`", target.name));
        }

        if target.name != HOST {
            errors.push(format!(
                "this compiler supports only implemented target `{HOST}`, got `{}`",
                target.name
            ));
        }

        if target.name == HOST {
            if target.backend != "arm64" {
                errors.push(format!(
                    "target `{HOST}` backend must be `arm64`, got `{}`",
                    target.backend
                ));
            }
            if target.executable != "macho" {
                errors.push(format!(
                    "target `{HOST}` executable must be `macho`, got `{}`",
                    target.executable
                ));
            }
            if target.os != "darwin" {
                errors.push(format!(
                    "target `{HOST}` os must be `darwin`, got `{}`",
                    target.os
                ));
            }
        }
    }

    if !names.contains(manifest.default_target.as_str()) {
        errors.push(format!(
            "default_target `{}` is not listed in implemented_targets",
            manifest.default_target
        ));
    }

    if manifest.archive.name != format!("nocter-v{version}-{HOST}.tar.gz") {
        errors.push(format!(
            "archive.name must be `nocter-v{version}-{HOST}.tar.gz`, got `{}`",
            manifest.archive.name
        ));
    }

    if manifest.archive.root.as_path() != Path::new(".nocter") {
        errors.push("archive.root must be `.nocter`".to_string());
    }
    validate_relative_path("archive.root", &manifest.archive.root, errors);
}

fn validate_relative_path(label: &str, path: &Path, errors: &mut Vec<String>) {
    if path.as_os_str().is_empty() {
        errors.push(format!("MANIFEST.json {label} must not be empty"));
        return;
    }

    if path.is_absolute() {
        errors.push(format!("MANIFEST.json {label} must be relative"));
        return;
    }

    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        errors.push(format!("MANIFEST.json {label} must not contain `..`"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    fn make_temp_home(name: &str) -> PathBuf {
        let unique = format!(
            "nocter-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = env::temp_dir().join(unique);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn rejects_invalid_version_prefix() {
        assert!(!is_valid_release_version("v0.1.0"));
    }

    #[test]
    fn accepts_prerelease_version() {
        assert!(is_valid_release_version("0.1.0-dev"));
    }

    #[test]
    fn validates_nocter_home_shape() {
        let root = make_temp_home("home-shape");
        fs::create_dir_all(root.join("std")).unwrap();
        fs::write(
            root.join("std/nocter.nct"),
            "#name: \"std\"\n#version: \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(root.join("std/index.nct"), "//! Standard library.\n").unwrap();
        fs::write(root.join("VERSION"), "0.1.0\n").unwrap();
        fs::write(root.join("LICENSE"), "Apache License\n").unwrap();
        fs::write(root.join("NOTICE"), "Nocter\nCopyright 2026 Rvo JP\n").unwrap();
        fs::write(
            root.join("MANIFEST.json"),
            r#"{
  "schema": "nocter.manifest",
  "schema_version": 1,
  "release": "0.1.0",
  "host": "arm64-darwin",
  "default_target": "arm64-darwin",
  "compiler": {
    "path": "nocter"
  },
  "std": {
    "path": "std"
  },
  "license": {
    "id": "Apache-2.0",
    "path": "LICENSE",
    "notice": "NOTICE"
  },
  "implemented_targets": [
    {
      "name": "arm64-darwin",
      "backend": "arm64",
      "executable": "macho",
      "os": "darwin"
    }
  ],
  "archive": {
    "name": "nocter-v0.1.0-arm64-darwin.tar.gz",
    "root": ".nocter"
  }
}
"#,
        )
        .unwrap();

        let errors = validate_nocter_home(&root);
        assert!(errors.is_empty(), "{errors:?}");

        fs::write(
            root.join("std/nocter.nct"),
            "#name: \"std\"\n#version: \"0.2.0\"\n",
        )
        .unwrap();
        let errors = validate_nocter_home(&root);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("does not match Nocter home version")),
            "{errors:?}"
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
