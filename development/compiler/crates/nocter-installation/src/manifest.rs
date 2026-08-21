use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

use nocter_model::CompilationTarget;

use crate::json::{self, Member, Value};

const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplementedTarget {
    target: CompilationTarget,
    backend: Box<str>,
    executable: Box<str>,
    operating_system: Box<str>,
}

impl ImplementedTarget {
    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub const fn backend(&self) -> &str {
        &self.backend
    }

    #[must_use]
    pub const fn executable(&self) -> &str {
        &self.executable
    }

    #[must_use]
    pub const fn operating_system(&self) -> &str {
        &self.operating_system
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LicenseMetadata {
    id: Box<str>,
    path: PathBuf,
    notice: PathBuf,
}

impl LicenseMetadata {
    #[must_use]
    pub const fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    #[must_use]
    pub fn notice(&self) -> &std::path::Path {
        &self.notice
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveMetadata {
    name: Box<str>,
    root: Box<str>,
}

impl ArchiveMetadata {
    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn root(&self) -> &str {
        &self.root
    }
}

/// Complete validated `nocter.manifest` v1 metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationManifest {
    release: Box<str>,
    host: Box<str>,
    default_target: CompilationTarget,
    compiler_path: PathBuf,
    standard_path: PathBuf,
    license: LicenseMetadata,
    implemented_targets: Box<[ImplementedTarget]>,
    archive: ArchiveMetadata,
}

impl InstallationManifest {
    pub(crate) fn decode(bytes: &[u8], version: &str) -> Result<Self, ManifestError> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(ManifestError::TooLarge {
                bytes: bytes.len(),
                limit: MAX_MANIFEST_BYTES,
            });
        }
        let text = std::str::from_utf8(bytes).map_err(|_| ManifestError::InvalidUtf8)?;
        let root = json::parse(text).map_err(|error| ManifestError::Json {
            offset: error.offset(),
            detail: error.to_string().into_boxed_str(),
        })?;
        decode_root(root, version)
    }

    #[must_use]
    pub const fn release(&self) -> &str {
        &self.release
    }

    #[must_use]
    pub const fn host(&self) -> &str {
        &self.host
    }

    #[must_use]
    pub const fn default_target(&self) -> CompilationTarget {
        self.default_target
    }

    #[must_use]
    pub fn compiler_path(&self) -> &std::path::Path {
        &self.compiler_path
    }

    #[must_use]
    pub fn standard_path(&self) -> &std::path::Path {
        &self.standard_path
    }

    #[must_use]
    pub const fn license(&self) -> &LicenseMetadata {
        &self.license
    }

    #[must_use]
    pub const fn implemented_targets(&self) -> &[ImplementedTarget] {
        &self.implemented_targets
    }

    #[must_use]
    pub const fn archive(&self) -> &ArchiveMetadata {
        &self.archive
    }
}

fn decode_root(value: Value, version: &str) -> Result<InstallationManifest, ManifestError> {
    let mut root = ExactObject::new(
        value,
        "$",
        &[
            "schema",
            "schema_version",
            "release",
            "host",
            "default_target",
            "compiler",
            "std",
            "license",
            "implemented_targets",
            "archive",
        ],
    )?;
    exact_string(root.take("schema")?, "$.schema", "nocter.manifest")?;
    exact_integer(root.take("schema_version")?, "$.schema_version", "1")?;
    let release = string(root.take("release")?, "$.release")?;
    validate_release(&release)?;
    if release.as_ref() != version {
        return Err(ManifestError::VersionMismatch {
            version: version.into(),
            manifest: release,
        });
    }
    let host = metadata_token(root.take("host")?, "$.host")?;
    let default_target = target(root.take("default_target")?, "$.default_target")?;
    let compiler_path = single_path_record(root.take("compiler")?, "$.compiler", "nocter")?;
    let standard_path = single_path_record(root.take("std")?, "$.std", "std")?;
    let license = decode_license(root.take("license")?)?;
    let implemented_targets = decode_targets(root.take("implemented_targets")?)?;
    if !implemented_targets
        .iter()
        .any(|implemented| implemented.target == default_target)
    {
        return Err(ManifestError::DefaultTargetNotImplemented(default_target));
    }
    let archive = decode_archive(root.take("archive")?, &release, &host)?;
    Ok(InstallationManifest {
        release,
        host,
        default_target,
        compiler_path,
        standard_path,
        license,
        implemented_targets: implemented_targets.into_boxed_slice(),
        archive,
    })
}

fn single_path_record(
    value: Value,
    field: &'static str,
    expected: &'static str,
) -> Result<PathBuf, ManifestError> {
    let mut object = ExactObject::new(value, field, &["path"])?;
    let path_field = format!("{field}.path");
    let value = string(object.take("path")?, &path_field)?;
    if value.as_ref() != expected {
        return Err(ManifestError::UnexpectedValue {
            field: path_field.into_boxed_str(),
            expected: expected.into(),
            actual: value,
        });
    }
    portable_relative_path(expected, &path_field)
}

fn decode_license(value: Value) -> Result<LicenseMetadata, ManifestError> {
    let mut object = ExactObject::new(value, "$.license", &["id", "path", "notice"])?;
    let id = string(object.take("id")?, "$.license.id")?;
    if id.as_ref() != "Apache-2.0" {
        return Err(ManifestError::UnexpectedValue {
            field: "$.license.id".into(),
            expected: "Apache-2.0".into(),
            actual: id,
        });
    }
    let path = string(object.take("path")?, "$.license.path")?;
    let notice = string(object.take("notice")?, "$.license.notice")?;
    Ok(LicenseMetadata {
        id,
        path: portable_relative_path(&path, "$.license.path")?,
        notice: portable_relative_path(&notice, "$.license.notice")?,
    })
}

fn decode_targets(value: Value) -> Result<Vec<ImplementedTarget>, ManifestError> {
    let Value::Array(values) = value else {
        return Err(wrong_type("$.implemented_targets", "array", &value));
    };
    let mut targets = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for (index, value) in values.into_iter().enumerate() {
        let base = format!("$.implemented_targets[{index}]");
        let mut object = ExactObject::new(value, &base, &["name", "backend", "executable", "os"])?;
        let target = target(object.take("name")?, &format!("{base}.name"))?;
        if !seen.insert(target) {
            return Err(ManifestError::DuplicateImplementedTarget(target));
        }
        let backend = metadata_token(object.take("backend")?, &format!("{base}.backend"))?;
        let executable = metadata_token(object.take("executable")?, &format!("{base}.executable"))?;
        let operating_system = metadata_token(object.take("os")?, &format!("{base}.os"))?;
        targets.push(ImplementedTarget {
            target,
            backend,
            executable,
            operating_system,
        });
    }
    Ok(targets)
}

fn decode_archive(
    value: Value,
    release: &str,
    host: &str,
) -> Result<ArchiveMetadata, ManifestError> {
    let mut object = ExactObject::new(value, "$.archive", &["name", "root"])?;
    let name = string(object.take("name")?, "$.archive.name")?;
    let expected = format!("nocter-v{release}-{host}.tar.gz");
    if name.as_ref() != expected {
        return Err(ManifestError::UnexpectedValue {
            field: "$.archive.name".into(),
            expected: expected.into_boxed_str(),
            actual: name,
        });
    }
    let root = string(object.take("root")?, "$.archive.root")?;
    if root.as_ref() != ".nocter" {
        return Err(ManifestError::UnexpectedValue {
            field: "$.archive.root".into(),
            expected: ".nocter".into(),
            actual: root,
        });
    }
    Ok(ArchiveMetadata { name, root })
}

fn validate_release(release: &str) -> Result<(), ManifestError> {
    if release.is_empty()
        || !release
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        || !release
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !release
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        || release.contains("..")
    {
        return Err(ManifestError::InvalidRelease(release.into()));
    }
    Ok(())
}

fn target(value: Value, field: &str) -> Result<CompilationTarget, ManifestError> {
    let name = string(value, field)?;
    CompilationTarget::from_name(&name).ok_or_else(|| ManifestError::UnknownTarget {
        field: field.into(),
        name,
    })
}

fn metadata_token(value: Value, field: &str) -> Result<Box<str>, ManifestError> {
    let token = string(value, field)?;
    if token.is_empty()
        || token
            .bytes()
            .any(|byte| !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && byte != b'-')
        || token.starts_with('-')
        || token.ends_with('-')
        || token.contains("--")
    {
        return Err(ManifestError::InvalidMetadataToken {
            field: field.into(),
            value: token,
        });
    }
    Ok(token)
}

fn portable_relative_path(value: &str, field: &str) -> Result<PathBuf, ManifestError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
        || value
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(ManifestError::InvalidRelativePath {
            field: field.into(),
            value: value.into(),
        });
    }
    Ok(value.split('/').collect())
}

fn exact_string(value: Value, field: &str, expected: &'static str) -> Result<(), ManifestError> {
    let actual = string(value, field)?;
    if actual.as_ref() == expected {
        Ok(())
    } else {
        Err(ManifestError::UnexpectedValue {
            field: field.into(),
            expected: expected.into(),
            actual,
        })
    }
}

fn exact_integer(value: Value, field: &str, expected: &'static str) -> Result<(), ManifestError> {
    let Value::Number(actual) = value else {
        return Err(wrong_type(field, "integer", &value));
    };
    if actual.as_ref() == expected {
        Ok(())
    } else {
        Err(ManifestError::UnexpectedValue {
            field: field.into(),
            expected: expected.into(),
            actual,
        })
    }
}

fn string(value: Value, field: &str) -> Result<Box<str>, ManifestError> {
    let Value::String(value) = value else {
        return Err(wrong_type(field, "string", &value));
    };
    Ok(value)
}

fn wrong_type(field: &str, expected: &'static str, value: &Value) -> ManifestError {
    let actual = match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    ManifestError::WrongType {
        field: field.into(),
        expected,
        actual,
    }
}

struct ExactObject {
    field: Box<str>,
    values: BTreeMap<Box<str>, Value>,
}

impl ExactObject {
    fn new(value: Value, field: &str, allowed: &[&str]) -> Result<Self, ManifestError> {
        let Value::Object(members) = value else {
            return Err(wrong_type(field, "object", &value));
        };
        let mut values = BTreeMap::new();
        for Member { name, value } in members {
            let member_field = child_field(field, &name);
            if !allowed.contains(&name.as_ref()) {
                return Err(ManifestError::UnknownField(member_field));
            }
            if values.insert(name, value).is_some() {
                return Err(ManifestError::DuplicateField(member_field));
            }
        }
        Ok(Self {
            field: field.into(),
            values,
        })
    }

    fn take(&mut self, name: &'static str) -> Result<Value, ManifestError> {
        self.values
            .remove(name)
            .ok_or_else(|| ManifestError::MissingField(child_field(&self.field, name)))
    }
}

fn child_field(parent: &str, name: &str) -> Box<str> {
    format!("{parent}.{name}").into_boxed_str()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestError {
    TooLarge {
        bytes: usize,
        limit: usize,
    },
    InvalidUtf8,
    Json {
        offset: usize,
        detail: Box<str>,
    },
    UnknownField(Box<str>),
    DuplicateField(Box<str>),
    MissingField(Box<str>),
    WrongType {
        field: Box<str>,
        expected: &'static str,
        actual: &'static str,
    },
    UnexpectedValue {
        field: Box<str>,
        expected: Box<str>,
        actual: Box<str>,
    },
    VersionMismatch {
        version: Box<str>,
        manifest: Box<str>,
    },
    InvalidRelease(Box<str>),
    UnknownTarget {
        field: Box<str>,
        name: Box<str>,
    },
    DuplicateImplementedTarget(CompilationTarget),
    DefaultTargetNotImplemented(CompilationTarget),
    InvalidMetadataToken {
        field: Box<str>,
        value: Box<str>,
    },
    InvalidRelativePath {
        field: Box<str>,
        value: Box<str>,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { bytes, limit } => {
                write!(formatter, "manifest is {bytes} bytes; the limit is {limit}")
            }
            Self::InvalidUtf8 => formatter.write_str("manifest is not UTF-8"),
            Self::Json { detail, .. } => write!(formatter, "invalid JSON: {detail}"),
            Self::UnknownField(field) => write!(formatter, "unknown manifest field {field}"),
            Self::DuplicateField(field) => write!(formatter, "duplicate manifest field {field}"),
            Self::MissingField(field) => write!(formatter, "missing manifest field {field}"),
            Self::WrongType {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "manifest field {field} must be {expected}, not {actual}"
            ),
            Self::UnexpectedValue {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "manifest field {field} must be `{expected}`, not `{actual}`"
            ),
            Self::VersionMismatch { version, manifest } => write!(
                formatter,
                "VERSION `{version}` does not match manifest release `{manifest}`"
            ),
            Self::InvalidRelease(release) => write!(
                formatter,
                "manifest release is not a portable release token: `{release}`"
            ),
            Self::UnknownTarget { field, name } => {
                write!(
                    formatter,
                    "manifest field {field} names unknown target `{name}`"
                )
            }
            Self::DuplicateImplementedTarget(target) => {
                write!(
                    formatter,
                    "implemented target `{target}` is listed more than once"
                )
            }
            Self::DefaultTargetNotImplemented(target) => write!(
                formatter,
                "default target `{target}` is not listed as implemented"
            ),
            Self::InvalidMetadataToken { field, value } => write!(
                formatter,
                "manifest field {field} is not a lowercase metadata token: `{value}`"
            ),
            Self::InvalidRelativePath { field, value } => write!(
                formatter,
                "manifest field {field} is not a portable relative path: `{value}`"
            ),
        }
    }
}

impl std::error::Error for ManifestError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> String {
        r#"{
            "schema": "nocter.manifest",
            "schema_version": 1,
            "release": "0.14.0",
            "host": "arm64-darwin",
            "default_target": "arm64-darwin",
            "compiler": { "path": "nocter" },
            "std": { "path": "std" },
            "license": {
                "id": "Apache-2.0",
                "path": "LICENSE",
                "notice": "NOTICE"
            },
            "implemented_targets": [{
                "name": "arm64-darwin",
                "backend": "arm64",
                "executable": "macho",
                "os": "darwin"
            }],
            "archive": {
                "name": "nocter-v0.14.0-arm64-darwin.tar.gz",
                "root": ".nocter"
            }
        }"#
        .into()
    }

    #[test]
    fn decodes_the_complete_exact_v1_schema() {
        let manifest = InstallationManifest::decode(valid_manifest().as_bytes(), "0.14.0").unwrap();
        assert_eq!(manifest.release(), "0.14.0");
        assert_eq!(manifest.host(), "arm64-darwin");
        assert_eq!(manifest.default_target(), CompilationTarget::Arm64Darwin);
        assert_eq!(manifest.compiler_path(), std::path::Path::new("nocter"));
        assert_eq!(manifest.standard_path(), std::path::Path::new("std"));
        assert_eq!(manifest.implemented_targets().len(), 1);
        assert_eq!(
            manifest.archive().name(),
            "nocter-v0.14.0-arm64-darwin.tar.gz"
        );
        assert_eq!(manifest.archive().root(), ".nocter");
        assert_eq!(manifest.license().id(), "Apache-2.0");
    }

    #[test]
    fn rejects_duplicate_unknown_and_missing_fields_without_overwrite() {
        let duplicate = valid_manifest().replacen(
            r#""schema": "nocter.manifest","#,
            r#""schema": "nocter.manifest", "schema": "nocter.manifest","#,
            1,
        );
        assert!(matches!(
            InstallationManifest::decode(duplicate.as_bytes(), "0.14.0"),
            Err(ManifestError::DuplicateField(field)) if field.as_ref() == "$.schema"
        ));

        let unknown = valid_manifest().replacen(
            r#""schema_version": 1,"#,
            r#""schema_version": 1, "future": true,"#,
            1,
        );
        assert!(matches!(
            InstallationManifest::decode(unknown.as_bytes(), "0.14.0"),
            Err(ManifestError::UnknownField(field)) if field.as_ref() == "$.future"
        ));

        let missing = valid_manifest().replacen(r#""schema_version": 1,"#, "", 1);
        assert!(matches!(
            InstallationManifest::decode(missing.as_bytes(), "0.14.0"),
            Err(ManifestError::MissingField(field)) if field.as_ref() == "$.schema_version"
        ));
    }

    #[test]
    fn rejects_cross_field_and_portability_inconsistency() {
        let mismatch = valid_manifest().replace(r#""release": "0.14.0""#, r#""release": "0.13.0""#);
        assert!(matches!(
            InstallationManifest::decode(mismatch.as_bytes(), "0.14.0"),
            Err(ManifestError::VersionMismatch { .. })
        ));

        let traversal = valid_manifest().replace(r#""path": "LICENSE""#, r#""path": "../LICENSE""#);
        assert!(matches!(
            InstallationManifest::decode(traversal.as_bytes(), "0.14.0"),
            Err(ManifestError::InvalidRelativePath { field, .. })
                if field.as_ref() == "$.license.path"
        ));

        let absent_default = valid_manifest().replace(
            r#""default_target": "arm64-darwin""#,
            r#""default_target": "x64-linux""#,
        );
        assert!(matches!(
            InstallationManifest::decode(absent_default.as_bytes(), "0.14.0"),
            Err(ManifestError::DefaultTargetNotImplemented(
                CompilationTarget::X64Linux
            ))
        ));

        let unsafe_release = valid_manifest()
            .replace(r#""release": "0.14.0""#, r#""release": "../0.14.0""#)
            .replace(
                "nocter-v0.14.0-arm64-darwin.tar.gz",
                "nocter-v../0.14.0-arm64-darwin.tar.gz",
            );
        assert!(matches!(
            InstallationManifest::decode(unsafe_release.as_bytes(), "../0.14.0"),
            Err(ManifestError::InvalidRelease(_))
        ));
    }
}
