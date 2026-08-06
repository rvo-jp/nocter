use super::{ModuleId, ModuleKey, NormalizedModulePath, PackageId, ResolvedModule};
use std::path::{Component, Path, PathBuf};

pub(super) fn package_root_module(
    package: PackageId,
    package_file_path: PathBuf,
) -> ResolvedModule {
    ResolvedModule::new(
        ModuleId::new(package, ModuleKey::PackageRoot),
        package_file_path,
    )
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
        let file = base.with_extension("nct");
        let index = base.join("index.nct");
        let selected = match (file.is_file(), index.is_file()) {
            (true, true) => {
                return Err(format!(
                    "target entry `{logical}` is ambiguous because both `{}` and `{}` exist",
                    file.display(),
                    index.display()
                ));
            }
            (true, false) => file,
            (false, true) => index,
            (false, false) => return Err(format!("target entry `{logical}` does not exist")),
        };
        canonical_module_path(&canonical_root, logical, selected)?
    } else {
        let index = canonical_root.join("index.nct");
        if !index.is_file() {
            return Err("target entry `.` does not exist at `index.nct`".to_string());
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
            "target entry must be `.` or a package-relative module path beginning with `./`"
                .to_string(),
        );
    };
    if relative.is_empty() || logical.ends_with(".nct") {
        return Err("target entry must name a logical module without a `.nct` suffix".to_string());
    }
    if Path::new(relative)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("target entry cannot escape the package root".to_string());
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
            "target entry `{logical}` could not be canonicalized at `{}`: {error}",
            selected.display()
        )
    })?;
    if !canonical.starts_with(canonical_root) {
        return Err(format!(
            "target entry `{logical}` escapes the package root through `{}`",
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
            "target entry `{logical}` crosses into the nested package at `{}`",
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
    let logical = if relative.file_name().is_some_and(|name| name == "index.nct") {
        relative
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf()
    } else {
        relative.with_extension("")
    };
    if logical.as_os_str().is_empty() {
        return Ok(NormalizedModulePath::new(".".to_string()));
    }
    let segments = logical
        .components()
        .map(|component| {
            component.as_os_str().to_str().ok_or_else(|| {
                format!(
                    "target entry resolves to the non-UTF-8 module path `{}`",
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
