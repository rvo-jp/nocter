//! Front-end source loading, parsing, and compile-unit construction.

use crate::analysis::CompileUnit;
use crate::ast::{AstFile, Item, ModulePath, UseItem};
use crate::diagnostics::Diagnostic;
use crate::home::resolve_nocter_home;
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::{ImportAccess, ImportSource, ImportSourceMap};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::target::DEFAULT_TARGET;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

const STANDARD_PRELUDE_PATH: &str = "std/prelude";

#[derive(Debug, Clone)]
pub(crate) struct FrontendOptions {
    pub(crate) nocter_home: Option<PathBuf>,
    pub(crate) target: String,
}

impl Default for FrontendOptions {
    fn default() -> Self {
        Self {
            nocter_home: None,
            target: DEFAULT_TARGET.to_string(),
        }
    }
}

pub(crate) fn load_compile_unit(
    sources: &mut SourceMap,
    root: SourceId,
    options: &FrontendOptions,
) -> Result<CompileUnit, Vec<Diagnostic>> {
    let mut queue = VecDeque::from([root]);
    let mut queued_sources = HashSet::from([root]);
    let mut loaded_sources_by_path = std::collections::HashMap::new();
    let mut import_sources = ImportSourceMap::new();
    let mut resolved_nocter_home = None;
    let mut diagnostics = Vec::new();
    let mut root_ast = None;
    let mut files = Vec::new();

    if let Some(path) = sources
        .get(root)
        .and_then(|file| file.absolute_path())
        .cloned()
    {
        loaded_sources_by_path.insert(path, root);
    }

    while let Some(source) = queue.pop_front() {
        let mut ast = match parse_source_for_check(sources, source) {
            Ok(ast) => ast,
            Err(source_diagnostics) => {
                diagnostics.extend(source_diagnostics);
                continue;
            }
        };

        if should_synthesize_prelude(sources, source, &ast, options, &mut resolved_nocter_home) {
            synthesize_prelude_use(source, &mut ast);
        }

        if source == root {
            root_ast = Some(ast.clone());
        }
        files.push(ast.clone());

        for path in import_paths(&ast) {
            let canonical = match resolve_import_path(
                sources,
                source,
                path,
                options,
                &mut resolved_nocter_home,
            ) {
                Ok(path) => path,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };

            let imported = match loaded_sources_by_path.get(&canonical).copied() {
                Some(source) => source,
                None => match sources.load_file(&canonical) {
                    Ok(source) => {
                        loaded_sources_by_path.insert(canonical, source);
                        source
                    }
                    Err(error) => {
                        diagnostics.push(import_source_diagnostic(
                            sources,
                            path.span,
                            &path.value,
                            error,
                        ));
                        continue;
                    }
                },
            };

            import_sources.insert(
                path.span,
                ImportSource {
                    source: imported,
                    access: import_access_for_source(
                        sources,
                        source,
                        options,
                        &resolved_nocter_home,
                    ),
                },
            );

            if queued_sources.insert(imported) {
                queue.push_back(imported);
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let Some(root_ast) = root_ast else {
        return Err(vec![Diagnostic::error(
            "E0200",
            "root source did not produce an AST and did not report a diagnostic",
        )]);
    };

    Ok(CompileUnit::new(root_ast, files, import_sources))
}

fn parse_source_for_check(
    sources: &SourceMap,
    source: SourceId,
) -> Result<AstFile, Vec<Diagnostic>> {
    let lexed = lex(sources, source);
    if !lexed.diagnostics.is_empty() {
        return Err(lexed.diagnostics);
    }

    let parsed = parse(sources, source, &lexed.tokens);
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics);
    }

    let Some(ast) = parsed.ast else {
        return Err(vec![Diagnostic::error(
            "E0200",
            "parser did not produce an AST and did not report a diagnostic",
        )]);
    };

    Ok(ast)
}

fn import_paths(ast: &AstFile) -> Vec<&ModulePath> {
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

fn should_synthesize_prelude(
    sources: &SourceMap,
    source: SourceId,
    ast: &AstFile,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> bool {
    if ast.items.iter().any(is_standard_prelude_use) {
        return false;
    }

    let Ok(home) = active_nocter_home(options, resolved_nocter_home) else {
        return true;
    };
    let home = canonicalize_existing(&home);
    let Some(source_path) = sources
        .get(source)
        .and_then(|file| file.absolute_path())
        .map(|path| canonicalize_existing(path))
    else {
        return true;
    };

    !source_path.starts_with(home)
}

fn is_standard_prelude_use(item: &Item) -> bool {
    matches!(item, Item::Use(use_) if use_.path.value == STANDARD_PRELUDE_PATH)
}

fn synthesize_prelude_use(source: SourceId, ast: &mut AstFile) {
    let span = ByteSpan::new(source, 0, 0);
    ast.items.insert(
        0,
        Item::Use(UseItem {
            span,
            path: ModulePath {
                span,
                value: STANDARD_PRELUDE_PATH.to_string(),
                segments: vec!["std".to_string(), "prelude".to_string()],
            },
        }),
    );
}

fn is_relative_module_path(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../")
}

fn resolve_import_path(
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

fn active_nocter_home(
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

fn import_access_for_source(
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

fn canonicalize_existing(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
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

fn relative_import_without_file_path_diagnostic(sources: &SourceMap, span: ByteSpan) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        "relative import cannot be resolved because the importing source has no file path",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help =
        Some("load the root source from a file before resolving relative imports".to_string());
    diagnostic
}

fn import_load_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    import_path: &str,
    candidates: &[PathBuf],
    error: impl std::fmt::Display,
    kind: ImportPathKind,
) -> Diagnostic {
    let searched = candidates
        .iter()
        .map(|path| format!("`{}`", path.display()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!("failed to resolve import `{import_path}`; searched {searched}: {error}"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(match kind {
        ImportPathKind::Relative => {
            "relative imports are resolved from the importing file directory and automatically add `.nct`"
                .to_string()
        }
        ImportPathKind::NonRelative => {
            "non-relative imports are resolved inside the active Nocter home; `std/...` searches the active target overlay before common `std/`"
                .to_string()
        }
    });
    diagnostic
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportPathKind {
    Relative,
    NonRelative,
}

fn nocter_home_import_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    import_path: &str,
    error: impl std::fmt::Display,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!("failed to resolve Nocter home while loading import `{import_path}`: {error}"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "set `NOCTER_HOME` to the active Nocter home, or run the `nocter` binary from inside its installed `.nocter/` directory"
            .to_string(),
    );
    diagnostic
}

fn import_source_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    import_path: &str,
    source_error: Diagnostic,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!(
            "failed to load import `{import_path}`: {}",
            source_error.message
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = source_error.help;
    diagnostic
}
