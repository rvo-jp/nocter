//! Deterministic selection and physical validation of one active Nocter home.
//!
//! This crate owns installation filesystem policy. It does not read process globals itself and
//! does not parse command arguments, resolve user packages, or invoke compiler stages.

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nocter_content_integrity::{
    ContentDigest, ContentIntegrityError, TreeHashOptions, sha256_file, sha256_regular_tree,
};
use nocter_model::PackageIdentity;
use nocter_package::StandardPackage;

mod compatibility;
mod manifest;

pub use compatibility::{CompilerInstallation, InstallationCompatibilityError};
pub use manifest::{
    ArchiveMetadata, ArtifactMetadata, ImplementedTarget, InstallationManifest, LicenseMetadata,
    ManifestError,
};

/// Explicit process facts needed to select a Nocter home.
#[derive(Clone, Debug)]
pub struct NocterHomeRequest {
    configured_home: Option<OsString>,
    executable: PathBuf,
}

impl NocterHomeRequest {
    #[must_use]
    pub fn new(configured_home: Option<OsString>, executable: impl Into<PathBuf>) -> Self {
        Self {
            configured_home,
            executable: executable.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NocterHomeOrigin {
    Configured,
    Executable,
}

/// Canonical physical installation boundary selected for one command invocation.
#[derive(Clone, Debug)]
pub struct NocterHome {
    root: PathBuf,
    origin: NocterHomeOrigin,
    manifest_path: PathBuf,
    manifest: InstallationManifest,
    compiler: PathBuf,
    standard_root: PathBuf,
    license: PathBuf,
    notice: PathBuf,
}

impl NocterHome {
    /// Selects and validates one physical installation without reading environment variables or
    /// the current process executable implicitly.
    ///
    /// # Errors
    ///
    /// Returns the exact invalid selection, filesystem operation, required entry, or VERSION
    /// format failure.
    pub fn resolve(request: NocterHomeRequest) -> Result<Self, NocterHomeError> {
        let executable = canonicalize("canonicalize compiler executable", &request.executable)?;
        if !executable.is_file() {
            return Err(NocterHomeError::ExecutableNotFile(executable));
        }
        let (selected, origin) = match request.configured_home {
            Some(path) if path.is_empty() => return Err(NocterHomeError::EmptyConfiguredHome),
            Some(path) => (PathBuf::from(path), NocterHomeOrigin::Configured),
            None => {
                let root = executable
                    .parent()
                    .ok_or_else(|| NocterHomeError::ExecutableWithoutParent(executable.clone()))?;
                (root.to_path_buf(), NocterHomeOrigin::Executable)
            }
        };
        let root = canonicalize("canonicalize Nocter home", &selected)?;
        if !root.is_dir() {
            return Err(NocterHomeError::HomeNotDirectory(root));
        }
        let version = required_file(&root, "VERSION")?;
        let manifest_path = required_file(&root, "MANIFEST.json")?;
        let release = read_release(&version)?;
        let manifest_bytes =
            fs::read(&manifest_path).map_err(|error| NocterHomeError::Filesystem {
                operation: "read MANIFEST.json",
                path: manifest_path.clone(),
                error,
            })?;
        let manifest =
            InstallationManifest::decode(&manifest_bytes, &release).map_err(|error| {
                NocterHomeError::Manifest {
                    path: manifest_path.clone(),
                    error,
                }
            })?;
        let compiler = required_relative_file(&root, "compiler.path", manifest.compiler().path())?;
        let installed_compiler_digest =
            sha256_file(&compiler).map_err(|error| NocterHomeError::ContentIntegrity {
                name: "compiler",
                error,
            })?;
        verify_digest(
            "compiler",
            &compiler,
            manifest.compiler().digest(),
            installed_compiler_digest,
        )?;
        let running_compiler_digest = if executable == compiler {
            installed_compiler_digest
        } else {
            sha256_file(&executable).map_err(|error| NocterHomeError::ContentIntegrity {
                name: "running compiler",
                error,
            })?
        };
        if running_compiler_digest != manifest.compiler().digest() {
            return Err(NocterHomeError::CompilerMismatch {
                running: executable,
                installed: compiler,
            });
        }
        let standard_root =
            required_relative_directory(&root, "std.path", manifest.standard().path())?;
        required_file(&standard_root, "index.nct")?;
        let standard_digest = sha256_regular_tree(&standard_root, TreeHashOptions::complete())
            .map_err(|error| NocterHomeError::ContentIntegrity { name: "std", error })?;
        verify_digest(
            "std",
            &standard_root,
            manifest.standard().digest(),
            standard_digest,
        )?;
        let license = required_relative_file(&root, "license.path", manifest.license().path())?;
        let notice = required_relative_file(&root, "license.notice", manifest.license().notice())?;
        Ok(Self {
            root,
            origin,
            manifest_path,
            manifest,
            compiler,
            standard_root,
            license,
            notice,
        })
    }

    /// Closes the host and native default-target relationship for the running compiler.
    ///
    /// # Errors
    ///
    /// Returns a compatibility failure when this home belongs to another compiler host or selects
    /// a non-native default target while cross compilation remains unsupported.
    pub fn for_compiler(
        self,
        compiler_host: &str,
    ) -> Result<CompilerInstallation, InstallationCompatibilityError> {
        CompilerInstallation::validate(self, compiler_host)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub const fn origin(&self) -> NocterHomeOrigin {
        self.origin
    }

    #[must_use]
    pub const fn release(&self) -> &str {
        self.manifest.release()
    }

    #[must_use]
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    #[must_use]
    pub const fn manifest(&self) -> &InstallationManifest {
        &self.manifest
    }

    #[must_use]
    pub fn compiler(&self) -> &Path {
        &self.compiler
    }

    #[must_use]
    pub fn standard_root(&self) -> &Path {
        &self.standard_root
    }

    #[must_use]
    pub fn license(&self) -> &Path {
        &self.license
    }

    #[must_use]
    pub fn notice(&self) -> &Path {
        &self.notice
    }

    /// Creates the standard-package input owned by this exact release installation.
    #[must_use]
    pub fn standard_package(&self) -> StandardPackage {
        StandardPackage::new(
            PackageIdentity::new(format!("toolchain-std-v{}", self.release())),
            self.standard_root.clone(),
            self.release(),
        )
    }
}

fn verify_digest(
    name: &'static str,
    path: &Path,
    expected: ContentDigest,
    actual: ContentDigest,
) -> Result<(), NocterHomeError> {
    if actual == expected {
        Ok(())
    } else {
        Err(NocterHomeError::ArtifactDigestMismatch {
            name,
            path: path.into(),
            expected,
            actual,
        })
    }
}

fn required_relative_file(
    root: &Path,
    name: &'static str,
    relative: &Path,
) -> Result<PathBuf, NocterHomeError> {
    inspect_required_file(root, name, root.join(relative))
}

fn required_file(root: &Path, name: &'static str) -> Result<PathBuf, NocterHomeError> {
    inspect_required_file(root, name, root.join(name))
}

fn inspect_required_file(
    root: &Path,
    name: &'static str,
    path: PathBuf,
) -> Result<PathBuf, NocterHomeError> {
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => {
            canonical_required(root, name, "canonicalize required file", &path)
        }
        Ok(_) => Err(NocterHomeError::RequiredEntryNotFile { name, path }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(NocterHomeError::MissingRequiredEntry { name, path })
        }
        Err(error) => Err(NocterHomeError::Filesystem {
            operation: "inspect required file",
            path,
            error,
        }),
    }
}

fn required_relative_directory(
    root: &Path,
    name: &'static str,
    relative: &Path,
) -> Result<PathBuf, NocterHomeError> {
    inspect_required_directory(root, name, root.join(relative))
}

fn inspect_required_directory(
    root: &Path,
    name: &'static str,
    path: PathBuf,
) -> Result<PathBuf, NocterHomeError> {
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            canonical_required(root, name, "canonicalize required directory", &path)
        }
        Ok(_) => Err(NocterHomeError::RequiredEntryNotDirectory { name, path }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(NocterHomeError::MissingRequiredEntry { name, path })
        }
        Err(error) => Err(NocterHomeError::Filesystem {
            operation: "inspect required directory",
            path,
            error,
        }),
    }
}

fn canonical_required(
    root: &Path,
    name: &'static str,
    operation: &'static str,
    path: &Path,
) -> Result<PathBuf, NocterHomeError> {
    let canonical = canonicalize(operation, path)?;
    if !canonical.starts_with(root) {
        return Err(NocterHomeError::RequiredEntryEscapesHome {
            name,
            path: path.into(),
            target: canonical,
        });
    }
    Ok(canonical)
}

fn read_release(path: &Path) -> Result<Box<str>, NocterHomeError> {
    let bytes = fs::read(path).map_err(|error| NocterHomeError::Filesystem {
        operation: "read VERSION",
        path: path.into(),
        error,
    })?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| NocterHomeError::InvalidVersion { path: path.into() })?;
    let release = text.strip_suffix('\n').unwrap_or(text);
    if release.is_empty() || release.contains(['\n', '\r']) {
        return Err(NocterHomeError::InvalidVersion { path: path.into() });
    }
    Ok(release.into())
}

fn canonicalize(operation: &'static str, path: &Path) -> Result<PathBuf, NocterHomeError> {
    fs::canonicalize(path).map_err(|error| NocterHomeError::Filesystem {
        operation,
        path: path.into(),
        error,
    })
}

#[derive(Debug)]
pub enum NocterHomeError {
    EmptyConfiguredHome,
    ExecutableNotFile(PathBuf),
    ExecutableWithoutParent(PathBuf),
    HomeNotDirectory(PathBuf),
    MissingRequiredEntry {
        name: &'static str,
        path: PathBuf,
    },
    RequiredEntryNotFile {
        name: &'static str,
        path: PathBuf,
    },
    RequiredEntryNotDirectory {
        name: &'static str,
        path: PathBuf,
    },
    RequiredEntryEscapesHome {
        name: &'static str,
        path: PathBuf,
        target: PathBuf,
    },
    CompilerMismatch {
        running: PathBuf,
        installed: PathBuf,
    },
    ArtifactDigestMismatch {
        name: &'static str,
        path: PathBuf,
        expected: ContentDigest,
        actual: ContentDigest,
    },
    ContentIntegrity {
        name: &'static str,
        error: ContentIntegrityError,
    },
    InvalidVersion {
        path: PathBuf,
    },
    Manifest {
        path: PathBuf,
        error: ManifestError,
    },
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        error: io::Error,
    },
}

impl fmt::Display for NocterHomeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyConfiguredHome => formatter.write_str("NOCTER_HOME is empty"),
            Self::ExecutableNotFile(path) => write!(
                formatter,
                "compiler executable is not a file: {}",
                path.display()
            ),
            Self::ExecutableWithoutParent(path) => write!(
                formatter,
                "compiler executable has no parent directory: {}",
                path.display()
            ),
            Self::HomeNotDirectory(path) => {
                write!(
                    formatter,
                    "Nocter home is not a directory: {}",
                    path.display()
                )
            }
            Self::MissingRequiredEntry { name, path } => write!(
                formatter,
                "Nocter home is missing {name} at {}",
                path.display()
            ),
            Self::RequiredEntryNotFile { name, path } => {
                write!(
                    formatter,
                    "Nocter home {name} is not a file: {}",
                    path.display()
                )
            }
            Self::RequiredEntryNotDirectory { name, path } => write!(
                formatter,
                "Nocter home {name} is not a directory: {}",
                path.display()
            ),
            Self::RequiredEntryEscapesHome { name, path, target } => write!(
                formatter,
                "Nocter home {name} escapes its containing directory: {} resolves to {}",
                path.display(),
                target.display()
            ),
            Self::CompilerMismatch { running, installed } => write!(
                formatter,
                "running compiler {} does not match the compiler in the selected Nocter home {}",
                running.display(),
                installed.display()
            ),
            Self::ArtifactDigestMismatch {
                name,
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "Nocter home {name} digest does not match its manifest at {}: expected {expected}, actual {actual}",
                path.display()
            ),
            Self::ContentIntegrity { name, error } => {
                write!(formatter, "cannot validate Nocter home {name}: {error}")
            }
            Self::InvalidVersion { path } => write!(
                formatter,
                "Nocter home VERSION is not one non-empty UTF-8 line: {}",
                path.display()
            ),
            Self::Manifest { path, error } => {
                write!(
                    formatter,
                    "invalid Nocter manifest {}: {error}",
                    path.display()
                )
            }
            Self::Filesystem {
                operation,
                path,
                error,
            } => write!(formatter, "cannot {operation} {}: {error}", path.display()),
        }
    }
}

impl std::error::Error for NocterHomeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filesystem { error, .. } => Some(error),
            Self::Manifest { error, .. } => Some(error),
            Self::ContentIntegrity { error, .. } => Some(error),
            Self::EmptyConfiguredHome
            | Self::ExecutableNotFile(_)
            | Self::ExecutableWithoutParent(_)
            | Self::HomeNotDirectory(_)
            | Self::MissingRequiredEntry { .. }
            | Self::RequiredEntryNotFile { .. }
            | Self::RequiredEntryNotDirectory { .. }
            | Self::RequiredEntryEscapesHome { .. }
            | Self::CompilerMismatch { .. }
            | Self::ArtifactDigestMismatch { .. }
            | Self::InvalidVersion { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-installation-{}-{serial}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn installation(&self, relative: &str, version: &[u8]) -> PathBuf {
            let root = self.0.join(relative);
            fs::create_dir_all(root.join("std")).unwrap();
            fs::write(root.join("VERSION"), version).unwrap();
            let release = std::str::from_utf8(version)
                .unwrap_or("0.14.0")
                .trim_end_matches('\n');
            fs::write(root.join("nocter"), b"compiler").unwrap();
            fs::write(root.join("LICENSE"), b"license").unwrap();
            fs::write(root.join("NOTICE"), b"notice").unwrap();
            fs::write(
                root.join("std/index.nct"),
                b"#package: { name: \"std\", version: \"0.0.0\", }\n",
            )
            .unwrap();
            let compiler_digest = sha256_file(&root.join("nocter")).unwrap();
            let standard_digest =
                sha256_regular_tree(&root.join("std"), TreeHashOptions::complete()).unwrap();
            fs::write(
                root.join("MANIFEST.json"),
                manifest(release, compiler_digest, standard_digest),
            )
            .unwrap();
            root
        }
    }

    fn manifest(
        release: &str,
        compiler_digest: ContentDigest,
        standard_digest: ContentDigest,
    ) -> String {
        format!(
            r#"{{
                "schema": "nocter.manifest",
                "schema_version": 2,
                "release": "{release}",
                "host": "arm64-darwin",
                "default_target": "arm64-darwin",
                "compiler": {{
                    "path": "nocter",
                    "sha256": "{compiler_digest}"
                }},
                "std": {{
                    "path": "std",
                    "tree_sha256": "{standard_digest}"
                }},
                "license": {{
                    "id": "Apache-2.0",
                    "path": "LICENSE",
                    "notice": "NOTICE"
                }},
                "implemented_targets": [{{
                    "name": "arm64-darwin",
                    "backend": "arm64",
                    "executable": "macho",
                    "os": "darwin"
                }}],
                "archive": {{
                    "name": "nocter-v{release}-arm64-darwin.tar.gz",
                    "root": ".nocter"
                }}
            }}"#
        )
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn configured_home_has_priority_and_produces_release_owned_standard_identity() {
        let tree = TempTree::new();
        let configured = tree.installation("configured", b"0.14.0\n");
        let other = tree.installation("other", b"9.9.9\n");
        let home = NocterHome::resolve(NocterHomeRequest::new(
            Some(configured.clone().into_os_string()),
            other.join("nocter"),
        ))
        .unwrap();

        assert_eq!(home.origin(), NocterHomeOrigin::Configured);
        assert_eq!(home.root(), fs::canonicalize(configured).unwrap());
        assert_eq!(home.release(), "0.14.0");
        assert_eq!(
            home.manifest().default_target(),
            nocter_model::CompilationTarget::Arm64Darwin
        );
        assert_eq!(
            home.standard_package().identity().as_str(),
            "toolchain-std-v0.14.0"
        );
    }

    #[test]
    fn configured_home_requires_the_running_compiler_to_match_its_bundled_compiler() {
        let tree = TempTree::new();
        let configured = tree.installation("configured", b"0.14.0\n");
        let other = tree.installation("other", b"9.9.9\n");
        fs::write(other.join("nocter"), b"another compiler").unwrap();

        let error = NocterHome::resolve(NocterHomeRequest::new(
            Some(configured.clone().into_os_string()),
            other.join("nocter"),
        ))
        .unwrap_err();

        assert!(matches!(error, NocterHomeError::CompilerMismatch { .. }));
    }

    #[test]
    fn manifest_digest_closes_compiler_and_standard_content_identity() {
        let tree = TempTree::new();
        let changed_compiler = tree.installation("changed-compiler", b"0.14.0\n");
        fs::write(changed_compiler.join("nocter"), b"changed compiler").unwrap();
        assert!(matches!(
            NocterHome::resolve(NocterHomeRequest::new(
                None,
                changed_compiler.join("nocter")
            )),
            Err(NocterHomeError::ArtifactDigestMismatch {
                name: "compiler",
                ..
            })
        ));

        let changed_standard = tree.installation("changed-standard", b"0.14.0\n");
        fs::write(
            changed_standard.join("std/index.nct"),
            b"#package: { name: \"std\", version: \"99.0.0\", }\n",
        )
        .unwrap();
        assert!(matches!(
            NocterHome::resolve(NocterHomeRequest::new(
                None,
                changed_standard.join("nocter")
            )),
            Err(NocterHomeError::ArtifactDigestMismatch { name: "std", .. })
        ));
    }

    #[test]
    fn compiler_compatibility_closes_host_and_native_target_identity() {
        let tree = TempTree::new();
        let compatible = tree.installation("compatible", b"0.14.0\n");
        let installation = NocterHome::resolve(NocterHomeRequest::new(
            Some(compatible.clone().into_os_string()),
            compatible.join("nocter"),
        ))
        .unwrap()
        .for_compiler("arm64-darwin")
        .unwrap();
        assert_eq!(installation.manifest().host(), "arm64-darwin");

        let wrong_host = tree.installation("wrong-host", b"0.14.0\n");
        let error = NocterHome::resolve(NocterHomeRequest::new(
            Some(wrong_host.clone().into_os_string()),
            wrong_host.join("nocter"),
        ))
        .unwrap()
        .for_compiler("x64-linux")
        .unwrap_err();
        assert!(matches!(
            error,
            InstallationCompatibilityError::HostMismatch { .. }
        ));

        let wrong_target = tree.installation("wrong-target", b"0.14.0\n");
        let manifest_path = wrong_target.join("MANIFEST.json");
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap()
            .replace(
                "\"default_target\": \"arm64-darwin\"",
                "\"default_target\": \"x64-linux\"",
            )
            .replace("\"name\": \"arm64-darwin\"", "\"name\": \"x64-linux\"");
        fs::write(manifest_path, manifest).unwrap();
        let error = NocterHome::resolve(NocterHomeRequest::new(
            Some(wrong_target.clone().into_os_string()),
            wrong_target.join("nocter"),
        ))
        .unwrap()
        .for_compiler("arm64-darwin")
        .unwrap_err();
        assert!(matches!(
            error,
            InstallationCompatibilityError::NativeDefaultTargetMismatch { .. }
        ));
    }

    #[test]
    fn executable_fallback_uses_the_real_executable_parent() {
        let tree = TempTree::new();
        let installation = tree.installation("home", b"0.14.0");
        let home =
            NocterHome::resolve(NocterHomeRequest::new(None, installation.join("nocter"))).unwrap();

        assert_eq!(home.origin(), NocterHomeOrigin::Executable);
        assert_eq!(home.root(), fs::canonicalize(installation).unwrap());
    }

    #[test]
    fn physical_layout_and_version_are_rejected_before_metadata_decoding() {
        let tree = TempTree::new();
        let invalid = tree.installation("invalid", b"0.14.0\nextra\n");
        assert!(matches!(
            NocterHome::resolve(NocterHomeRequest::new(
                Some(invalid.clone().into_os_string()),
                invalid.join("nocter")
            )),
            Err(NocterHomeError::InvalidVersion { .. })
        ));

        let missing = tree.installation("missing", b"0.14.0\n");
        fs::remove_file(missing.join("MANIFEST.json")).unwrap();
        assert!(matches!(
            NocterHome::resolve(NocterHomeRequest::new(
                Some(missing.clone().into_os_string()),
                missing.join("nocter")
            )),
            Err(NocterHomeError::MissingRequiredEntry {
                name: "MANIFEST.json",
                ..
            })
        ));
    }

    #[test]
    fn metadata_and_its_declared_files_are_one_validation_boundary() {
        let tree = TempTree::new();
        let invalid = tree.installation("invalid-manifest", b"0.14.0\n");
        fs::write(invalid.join("MANIFEST.json"), b"{}").unwrap();
        assert!(matches!(
            NocterHome::resolve(NocterHomeRequest::new(
                Some(invalid.clone().into_os_string()),
                invalid.join("nocter")
            )),
            Err(NocterHomeError::Manifest { .. })
        ));

        let missing_license = tree.installation("missing-license", b"0.14.0\n");
        fs::remove_file(missing_license.join("LICENSE")).unwrap();
        assert!(matches!(
            NocterHome::resolve(NocterHomeRequest::new(
                Some(missing_license.clone().into_os_string()),
                missing_license.join("nocter")
            )),
            Err(NocterHomeError::MissingRequiredEntry {
                name: "license.path",
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn required_entries_cannot_escape_the_selected_home() {
        use std::os::unix::fs::symlink;

        let tree = TempTree::new();
        let installation = tree.installation("home", b"0.14.0\n");
        let outside = tree.0.join("outside-manifest.json");
        fs::write(&outside, b"{}").unwrap();
        fs::remove_file(installation.join("MANIFEST.json")).unwrap();
        symlink(outside, installation.join("MANIFEST.json")).unwrap();

        assert!(matches!(
            NocterHome::resolve(NocterHomeRequest::new(
                Some(installation.clone().into_os_string()),
                installation.join("nocter")
            )),
            Err(NocterHomeError::RequiredEntryEscapesHome {
                name: "MANIFEST.json",
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn standard_tree_root_must_be_physical_even_when_a_symlink_stays_inside_home() {
        use std::os::unix::fs::symlink;

        let tree = TempTree::new();
        let installation = tree.installation("home-with-linked-std", b"0.14.0\n");
        fs::rename(installation.join("std"), installation.join("std-real")).unwrap();
        symlink("std-real", installation.join("std")).unwrap();

        assert!(matches!(
            NocterHome::resolve(NocterHomeRequest::new(None, installation.join("nocter"))),
            Err(NocterHomeError::RequiredEntryNotDirectory {
                name: "std.path",
                ..
            })
        ));
    }
}
