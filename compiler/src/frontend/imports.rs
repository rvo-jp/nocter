use super::FrontendOptions;
use super::diagnostics::{
    ImportPathKind, ambiguous_import_diagnostic, import_load_diagnostic,
    nocter_home_import_diagnostic, relative_import_without_file_path_diagnostic,
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
    source_root: Option<&Path>,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> Result<PathBuf, Diagnostic> {
    if is_relative_module_path(&path.value) {
        let Some(resolved_path) = resolve_relative_import_path(sources, source, path) else {
            return Err(relative_import_without_file_path_diagnostic(
                sources, path.span,
            ));
        };

        return resolve_module_candidate(resolved_path).map_err(|error| match error {
            ImportResolutionError::Missing { candidates, error } => import_load_diagnostic(
                sources,
                path.span,
                &path.value,
                &candidates,
                error,
                ImportPathKind::Relative,
            ),
            ImportResolutionError::Ambiguous { file, directory } => {
                ambiguous_import_diagnostic(sources, path.span, &path.value, &file, &directory)
            }
        });
    }

    if is_absolute_module_path(&path.value) {
        return resolve_module_candidate(PathBuf::from(&path.value)).map_err(|error| match error {
            ImportResolutionError::Missing { candidates, error } => import_load_diagnostic(
                sources,
                path.span,
                &path.value,
                &candidates,
                error,
                ImportPathKind::Absolute,
            ),
            ImportResolutionError::Ambiguous { file, directory } => {
                ambiguous_import_diagnostic(sources, path.span, &path.value, &file, &directory)
            }
        });
    }

    let mut searched = Vec::new();
    if let Some(root) = source_root {
        match resolve_module_candidate(root.join(&path.value)) {
            Ok(path) => return Ok(path),
            Err(ImportResolutionError::Missing { candidates, .. }) => {
                searched.extend(candidates);
            }
            Err(ImportResolutionError::Ambiguous { file, directory }) => {
                return Err(ambiguous_import_diagnostic(
                    sources,
                    path.span,
                    &path.value,
                    &file,
                    &directory,
                ));
            }
        }
    }

    let home = active_nocter_home(options, resolved_nocter_home).map_err(|message| {
        nocter_home_import_diagnostic(sources, path.span, &path.value, message)
    })?;

    resolve_module_candidate(home.join(&path.value)).map_err(|error| match error {
        ImportResolutionError::Missing { candidates, error } => {
            searched.extend(candidates);
            import_load_diagnostic(
                sources,
                path.span,
                &path.value,
                &searched,
                error,
                ImportPathKind::NonRelative,
            )
        }
        ImportResolutionError::Ambiguous { file, directory } => {
            ambiguous_import_diagnostic(sources, path.span, &path.value, &file, &directory)
        }
    })
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

fn is_absolute_module_path(path: &str) -> bool {
    path.starts_with('/')
}

fn resolve_relative_import_path(
    sources: &SourceMap,
    source: SourceId,
    path: &ModulePath,
) -> Option<PathBuf> {
    let source_file = sources.get(source)?;
    let source_path = source_file.absolute_path()?;
    let source_dir = source_path.parent()?;
    Some(source_dir.join(&path.value))
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

#[derive(Debug)]
enum ImportResolutionError {
    Missing {
        candidates: Vec<PathBuf>,
        error: String,
    },
    Ambiguous {
        file: PathBuf,
        directory: PathBuf,
    },
}

fn resolve_module_candidate(module_path: PathBuf) -> Result<PathBuf, ImportResolutionError> {
    let file = module_path.with_extension("nct");
    let index = module_path.join("index.nct");
    let candidates = vec![file.clone(), index.clone()];
    let file = canonicalize_candidate(file)?;
    let directory = canonicalize_directory_candidate(module_path)?;
    if let (Some(file), Some(directory)) = (&file, directory) {
        return Err(ImportResolutionError::Ambiguous {
            file: file.clone(),
            directory,
        });
    }
    let index = canonicalize_candidate(index)?;

    match (file, index) {
        (Some(file), Some(index)) => Err(ImportResolutionError::Ambiguous {
            file,
            directory: index
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
        }),
        (Some(file), None) => Ok(file),
        (None, Some(index)) => Ok(index),
        (None, None) => Err(ImportResolutionError::Missing {
            candidates,
            error: "file was not found in any import root".to_string(),
        }),
    }
}

fn canonicalize_directory_candidate(
    path: PathBuf,
) -> Result<Option<PathBuf>, ImportResolutionError> {
    match path.canonicalize() {
        Ok(path) if path.is_dir() => Ok(Some(path)),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ImportResolutionError::Missing {
            candidates: vec![path],
            error: error.to_string(),
        }),
    }
}

fn canonicalize_candidate(path: PathBuf) -> Result<Option<PathBuf>, ImportResolutionError> {
    match path.canonicalize() {
        Ok(path) => Ok(Some(path)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ImportResolutionError::Missing {
            candidates: vec![path],
            error: error.to_string(),
        }),
    }
}
