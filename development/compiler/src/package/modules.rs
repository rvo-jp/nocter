use super::{ModuleId, ModuleKey, NormalizedModulePath, PackageId, ResolvedModule};
use std::path::{Component, Path, PathBuf};

pub(super) fn resolve_package_root_module(
    root: &Path,
    package: PackageId,
) -> Result<ResolvedModule, String> {
    let (_, source_path) = resolve(root, ".")?;
    Ok(ResolvedModule::new(
        ModuleId::new(package, ModuleKey::PackageRoot),
        source_path,
    ))
}

pub(super) fn resolve_explicit_module(
    root: &Path,
    package: PackageId,
    logical: &str,
) -> Result<ResolvedModule, String> {
    let (normalized, source_path) = resolve(root, logical)?;
    Ok(ResolvedModule::new(
        ModuleId::new(package, ModuleKey::Path(normalized)),
        source_path,
    ))
}

pub(crate) fn resolve_explicit_module_path(root: &Path, logical: &str) -> Result<PathBuf, String> {
    resolve(root, logical).map(|(_, path)| path)
}

fn resolve(root: &Path, logical: &str) -> Result<(NormalizedModulePath, PathBuf), String> {
    let canonical_root = root.canonicalize().map_err(|error| {
        format!(
            "package root `{}` could not be canonicalized: {error}",
            root.display()
        )
    })?;
    let relative = validate_logical_path(logical)?;
    let selected = if let Some(relative) = relative {
        let base = canonical_root.join(relative);
        let index = base.join("index.nct");
        let selected = index
            .is_file()
            .then_some(index)
            .ok_or_else(|| format!("target module `{logical}` does not exist at `index.nct`"))?;
        canonical_module_path(&canonical_root, logical, selected)?
    } else {
        let index = canonical_root.join("index.nct");
        if !index.is_file() {
            return Err("target module `.` does not exist at `index.nct`".to_string());
        }
        canonical_module_path(&canonical_root, logical, index)?
    };
    let normalized = normalized_module_path(&canonical_root, &selected)?;
    Ok((normalized, selected))
}

fn validate_logical_path(logical: &str) -> Result<Option<PathBuf>, String> {
    if logical == "." {
        return Ok(None);
    }
    let Some(relative) = logical.strip_prefix("./") else {
        return Err(
            "target module must be `.` or a package-relative module path beginning with `./`"
                .to_string(),
        );
    };
    if relative.is_empty() || logical.ends_with(".nct") {
        return Err(
            "target module must name a logical directory module without a `.nct` suffix"
                .to_string(),
        );
    }
    if Path::new(relative)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("target module cannot escape the package root".to_string());
    }
    Ok(Some(PathBuf::from(relative)))
}

fn canonical_module_path(
    canonical_root: &Path,
    logical: &str,
    selected: PathBuf,
) -> Result<PathBuf, String> {
    let canonical = selected.canonicalize().map_err(|error| {
        format!(
            "target module `{logical}` could not be canonicalized at `{}`: {error}",
            selected.display()
        )
    })?;
    if !canonical.starts_with(canonical_root) {
        return Err(format!(
            "target module `{logical}` escapes the package root through `{}`",
            selected.display()
        ));
    }
    let owner = canonical.parent().and_then(|directory| {
        directory
            .ancestors()
            .find(|ancestor| ancestor.join("nocter.nct").is_file())
    });
    if owner.is_some_and(|owner| owner != canonical_root) {
        return Err(format!(
            "target module `{logical}` crosses into the nested package at `{}`",
            owner.expect("checked package owner").display()
        ));
    }
    Ok(canonical)
}

fn normalized_module_path(
    canonical_root: &Path,
    source_path: &Path,
) -> Result<NormalizedModulePath, String> {
    let relative = source_path
        .strip_prefix(canonical_root)
        .expect("validated module source is inside its canonical package root");
    let logical = crate::source_layout::logical_module_path(relative).ok_or_else(|| {
        format!(
            "target module resolves to a non-Nocter source `{}`",
            source_path.display()
        )
    })?;
    if logical.as_os_str().is_empty() {
        return Ok(NormalizedModulePath::new(".".to_string()));
    }
    let segments = logical
        .components()
        .map(|component| {
            component.as_os_str().to_str().ok_or_else(|| {
                format!(
                    "target module resolves to the non-UTF-8 module path `{}`",
                    source_path.display()
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NormalizedModulePath::new(format!(
        "./{}",
        segments.join("/")
    )))
}
