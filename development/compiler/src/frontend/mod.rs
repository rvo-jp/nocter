//! Front-end source loading, parsing, and compile-unit construction.

mod diagnostics;
mod imports;
mod module_discovery;
mod parsing;
mod prelude;

#[cfg(test)]
mod tests;

use crate::analysis::CompileUnit;
use crate::ast::{AstFile, ImplMember, Item, Visibility};
use crate::diagnostics::Diagnostic;
use crate::resolve::{ImportSource, ImportSourceMap, PreludeSourceMap};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::target::DEFAULT_TARGET;
use crate::target::primitive::validate_primitive_declaration;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use diagnostics::{
    import_source_diagnostic, nocter_visibility_outside_nocter_home_diagnostic,
    primitive_outside_nocter_home_diagnostic, primitive_registry_diagnostic,
};
use imports::{
    active_nocter_home, canonicalize_existing, import_access_for_source, import_paths,
    resolve_import_path,
};
use parsing::parse_source_for_check;
use prelude::{should_load_prelude, standard_prelude_path};

pub(crate) use module_discovery::module_segment_candidates;

#[derive(Debug, Clone)]
pub(crate) struct FrontendOptions {
    pub(crate) nocter_home: Option<PathBuf>,
    pub(crate) source_root: Option<PathBuf>,
    pub(crate) target: String,
}

impl Default for FrontendOptions {
    fn default() -> Self {
        Self {
            nocter_home: None,
            source_root: None,
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
    let mut prelude_sources = PreludeSourceMap::new();
    let mut resolved_nocter_home = None;
    let source_root = active_source_root(sources, root, options);
    let mut diagnostics = Vec::new();
    let mut root_ast = None;
    let mut files = Vec::new();

    for (path, source) in sources.sources_with_absolute_paths() {
        loaded_sources_by_path.insert(path.to_path_buf(), source);
    }

    while let Some(source) = queue.pop_front() {
        let mut ast = match parse_source_for_check(sources, source) {
            Ok(ast) => ast,
            Err(source_diagnostics) => {
                diagnostics.extend(source_diagnostics);
                continue;
            }
        };

        filter_target_items(&mut ast, &options.target);

        diagnostics.extend(validate_nocter_visibility_declarations(
            sources,
            source,
            &ast,
            options,
            &mut resolved_nocter_home,
        ));

        diagnostics.extend(validate_primitive_declarations(
            sources,
            source,
            &ast,
            options,
            &mut resolved_nocter_home,
        ));

        if should_load_prelude(sources, source, options, &mut resolved_nocter_home) {
            let path = standard_prelude_path(source);
            match resolve_import_path(
                sources,
                source,
                &path,
                options,
                None,
                &mut resolved_nocter_home,
            ) {
                Ok(canonical) => {
                    let imported = match loaded_sources_by_path.get(&canonical).copied() {
                        Some(source) => Some(source),
                        None => match sources.load_file(&canonical) {
                            Ok(source) => {
                                loaded_sources_by_path.insert(canonical, source);
                                Some(source)
                            }
                            Err(error) => {
                                diagnostics.push(import_source_diagnostic(
                                    sources,
                                    path.span,
                                    &path.value,
                                    error,
                                ));
                                None
                            }
                        },
                    };

                    if let Some(imported) = imported {
                        prelude_sources.insert(
                            source,
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
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                }
            }
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
                source_root.as_deref(),
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

    let nocter_home = resolved_nocter_home
        .as_ref()
        .and_then(|home| home.as_ref().ok())
        .map(|home| canonicalize_existing(home));

    Ok(CompileUnit::new(
        root_ast,
        files,
        import_sources,
        prelude_sources,
        nocter_home,
    ))
}

fn active_source_root(
    sources: &SourceMap,
    root: SourceId,
    options: &FrontendOptions,
) -> Option<PathBuf> {
    options.source_root.clone().or_else(|| {
        sources
            .get(root)
            .and_then(|file| file.absolute_path())
            .and_then(|path| path.parent())
            .map(canonicalize_existing)
    })
}

fn filter_target_items(ast: &mut AstFile, target: &str) {
    ast.items.retain(|item| match item {
        Item::Function(function) => function
            .target
            .as_ref()
            .is_none_or(|directive| directive.target == target),
        Item::Primitive(primitive) => primitive
            .target
            .as_ref()
            .is_none_or(|directive| directive.target == target),
        Item::TypeAlias(alias) => alias
            .target_directive
            .as_ref()
            .is_none_or(|directive| directive.target == target),
        Item::Struct(struct_) => struct_
            .target
            .as_ref()
            .is_none_or(|directive| directive.target == target),
        Item::Enum(enum_) => enum_
            .target
            .as_ref()
            .is_none_or(|directive| directive.target == target),
        Item::Interface(interface) => interface
            .target
            .as_ref()
            .is_none_or(|directive| directive.target == target),
        _ => true,
    });
}

fn validate_nocter_visibility_declarations(
    sources: &SourceMap,
    source: SourceId,
    ast: &AstFile,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> Vec<Diagnostic> {
    let spans = nocter_visibility_declaration_spans(ast);
    if spans.is_empty()
        || source_is_inside_active_nocter_home(sources, source, options, resolved_nocter_home)
    {
        return Vec::new();
    }

    spans
        .into_iter()
        .map(|span| nocter_visibility_outside_nocter_home_diagnostic(sources, span))
        .collect()
}

fn nocter_visibility_declaration_spans(ast: &AstFile) -> Vec<ByteSpan> {
    let mut spans = Vec::new();

    for item in &ast.items {
        match item {
            Item::Function(function) if function.visibility == Visibility::Nocter => {
                spans.push(function.span);
            }
            Item::TypeAlias(alias) if alias.visibility == Visibility::Nocter => {
                spans.push(alias.span);
            }
            Item::Struct(struct_) => {
                if struct_.visibility == Visibility::Nocter {
                    spans.push(struct_.span);
                }
                spans.extend(
                    struct_
                        .fields
                        .iter()
                        .filter(|field| field.visibility == Visibility::Nocter)
                        .map(|field| field.span),
                );
            }
            Item::Enum(enum_) if enum_.visibility == Visibility::Nocter => {
                spans.push(enum_.span);
            }
            Item::Interface(interface) if interface.visibility == Visibility::Nocter => {
                spans.push(interface.span);
            }
            Item::Impl(impl_) => {
                spans.extend(impl_.members.iter().filter_map(|member| match member {
                    ImplMember::Method(method) if method.visibility == Visibility::Nocter => {
                        Some(method.span)
                    }
                    _ => None,
                }));
            }
            _ => {}
        }
    }

    spans
}

fn source_is_inside_active_nocter_home(
    sources: &SourceMap,
    source: SourceId,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> bool {
    let Ok(home) = active_nocter_home(options, resolved_nocter_home) else {
        return false;
    };

    let Some(source_path) = sources
        .get(source)
        .and_then(|file| file.absolute_path())
        .map(|path| canonicalize_existing(path))
    else {
        return false;
    };

    source_path.starts_with(canonicalize_existing(&home))
}

fn validate_primitive_declarations(
    sources: &SourceMap,
    source: SourceId,
    ast: &AstFile,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> Vec<Diagnostic> {
    let primitives = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Primitive(primitive) => Some(primitive),
            _ => None,
        })
        .collect::<Vec<_>>();

    if primitives.is_empty() {
        return Vec::new();
    }

    let Ok(home) = active_nocter_home(options, resolved_nocter_home) else {
        return primitives
            .into_iter()
            .map(|primitive| {
                primitive_outside_nocter_home_diagnostic(sources, primitive.span, &options.target)
            })
            .collect();
    };

    let Some(source_path) = sources
        .get(source)
        .and_then(|file| file.absolute_path())
        .map(|path| canonicalize_existing(path))
    else {
        return primitives
            .into_iter()
            .map(|primitive| {
                primitive_outside_nocter_home_diagnostic(sources, primitive.span, &options.target)
            })
            .collect();
    };

    let Some(module_path) = primitive_module_path(&source_path, &home, &options.target) else {
        return primitives
            .into_iter()
            .map(|primitive| {
                primitive_outside_nocter_home_diagnostic(sources, primitive.span, &options.target)
            })
            .collect();
    };

    primitives
        .into_iter()
        .filter_map(|primitive| {
            validate_primitive_declaration(&module_path, &options.target, primitive)
                .err()
                .map(|error| {
                    primitive_registry_diagnostic(
                        sources,
                        primitive.span,
                        error.message,
                        error.help,
                    )
                })
        })
        .collect()
}

fn primitive_module_path(source_path: &Path, home: &Path, _target: &str) -> Option<String> {
    let home = canonicalize_existing(home);
    let common_std = home.join("std");

    source_path
        .strip_prefix(common_std)
        .ok()
        .and_then(std_relative_module_path)
}

fn std_relative_module_path(relative_path: &Path) -> Option<String> {
    let mut segments = relative_path
        .iter()
        .map(|segment| segment.to_str().map(str::to_string))
        .collect::<Option<Vec<_>>>()?;
    let file = segments.last_mut()?;
    let stem = file.strip_suffix(".nct")?;
    *file = stem.to_string();

    Some(format!("std/{}", segments.join("/")))
}
