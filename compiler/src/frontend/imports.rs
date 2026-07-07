use super::FrontendOptions;
use super::diagnostics::{
    ImportPathKind, import_load_diagnostic, nocter_home_import_diagnostic,
    relative_import_without_file_path_diagnostic,
};
use crate::ast::{AstFile, Item, ModulePath};
use crate::diagnostics::Diagnostic;
use crate::home::resolve_nocter_home;
use crate::resolve::ImportAccess;
use crate::source::{SourceId, SourceMap};
use std::path::{Path, PathBuf};

pub(super) fn import_paths(ast: &AstFile) -> Vec<&ModulePath> {
    ast.items
        .iter()
        .filter_map(|item| match item {
            Item::Use(item) => Some(&item.path),
            Item::Import(item) => Some(&item.path),
            Item::FromImport(item) => Some(&item.path),
            _ => None,
        })
        .collect()
}

pub(super) fn resolve_import_path(
    sources: &SourceMap,
    source: SourceId,
    path: &ModulePath,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> Result<PathBuf, Diagnostic> {
    if is_relative_module_path(&path.value) {
        let Some(resolved_path) = resolve_relative_import_path(sources, source, path) else {
            return Err(relative_import_without_file_path_diagnostic(
                sources, path.span,
            ));
        };

        return resolved_path.canonicalize().map_err(|error| {
            import_load_diagnostic(
                sources,
                path.span,
                &path.value,
                &[resolved_path],
                error,
                ImportPathKind::Relative,
            )
        });
    }

    let home = active_nocter_home(options, resolved_nocter_home).map_err(|message| {
        nocter_home_import_diagnostic(sources, path.span, &path.value, message)
    })?;
    let candidates = non_relative_import_candidates(&home, &options.target, &path.value);

    for candidate in &candidates {
        if let Ok(canonical) = candidate.canonicalize() {
            return Ok(canonical);
        }
    }

    Err(import_load_diagnostic(
        sources,
        path.span,
        &path.value,
        &candidates,
        "file was not found in any import root",
        ImportPathKind::NonRelative,
    ))
}

pub(super) fn active_nocter_home(
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> Result<PathBuf, String> {
    if let Some(home) = &options.nocter_home {
        return Ok(home.clone());
    }

    if let Some(cached) = resolved_nocter_home {
        return cached.clone();
    }

    let resolved = resolve_nocter_home();
    *resolved_nocter_home = Some(resolved.clone());
    resolved
}

pub(super) fn import_access_for_source(
    sources: &SourceMap,
    source: SourceId,
    options: &FrontendOptions,
    resolved_nocter_home: &Option<Result<PathBuf, String>>,
) -> ImportAccess {
    let Some(home) = current_nocter_home(options, resolved_nocter_home) else {
        return ImportAccess::Public;
    };
    let Some(source_path) = sources
        .get(source)
        .and_then(|file| file.absolute_path())
        .map(|path| canonicalize_existing(path))
    else {
        return ImportAccess::Public;
    };

    if source_path.starts_with(home) {
        ImportAccess::Nocter
    } else {
        ImportAccess::Public
    }
}

pub(super) fn canonicalize_existing(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn is_relative_module_path(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../")
}

fn resolve_relative_import_path(
    sources: &SourceMap,
    source: SourceId,
    path: &ModulePath,
) -> Option<PathBuf> {
    let source_file = sources.get(source)?;
    let source_path = source_file.absolute_path()?;
    let source_dir = source_path.parent()?;
    Some(source_dir.join(format!("{}.nct", path.value)))
}

fn current_nocter_home(
    options: &FrontendOptions,
    resolved_nocter_home: &Option<Result<PathBuf, String>>,
) -> Option<PathBuf> {
    if let Some(home) = &options.nocter_home {
        return Some(canonicalize_existing(home));
    }

    resolved_nocter_home
        .as_ref()
        .and_then(|home| home.as_ref().ok())
        .map(|home| canonicalize_existing(home))
}

fn non_relative_import_candidates(home: &Path, target: &str, import_path: &str) -> Vec<PathBuf> {
    if let Some(std_path) = import_path.strip_prefix("std/") {
        return vec![
            home.join("targets")
                .join(target)
                .join("std")
                .join(format!("{std_path}.nct")),
            home.join("std").join(format!("{std_path}.nct")),
        ];
    }

    vec![home.join(format!("{import_path}.nct"))]
}
