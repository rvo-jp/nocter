use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub schema_version: u32,
    pub release: String,
    pub host: String,
    pub default_target: String,
    pub compiler: Compiler,
    pub std: StandardLibrary,
    pub implemented_targets: Vec<ImplementedTarget>,
    pub archive: Archive,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compiler {
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandardLibrary {
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImplementedTarget {
    pub name: String,
    pub std_path: PathBuf,
    pub backend: String,
    pub executable: String,
    pub os: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    pub name: String,
    pub root: PathBuf,
}

pub fn load_manifest(path: &Path) -> Result<Manifest, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read `{}`: {error}", path.display()))?;

    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse `{}`: {error}", path.display()))
}
