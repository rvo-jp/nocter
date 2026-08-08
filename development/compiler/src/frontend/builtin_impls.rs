//! Compiler-owned loading and declaration authority for built-in type methods.

use super::dependencies::SourceDependencyTrace;
use super::diagnostics::import_source_diagnostic;
use super::imports::resolve_import_path;
use super::{FrontendOptions, trusted_module_path};
use crate::ast::{AstFile, Item, ModulePath, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, SourceId, SourceMap};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

const BUILTIN_IMPLEMENTATION_MODULES: [(&str, &str); 2] =
    [("str", "std/str"), ("[T]", "std/slice")];

pub(super) fn enqueue_builtin_implementation_sources(
    sources: &mut SourceMap,
    root: SourceId,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
    loaded_sources_by_path: &mut HashMap<PathBuf, SourceId>,
    dependencies: &mut SourceDependencyTrace,
    queue: &mut VecDeque<SourceId>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (_, module) in BUILTIN_IMPLEMENTATION_MODULES {
        let path = module_path(root, module);
        let canonical = match resolve_import_path(
            sources,
            root,
            &path,
            options,
            resolved_nocter_home,
            dependencies,
        ) {
            Ok(path) => path,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
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
        if let Some(imported) = imported
            && dependencies.record_source(imported)
        {
            queue.push_back(imported);
        }
    }
    diagnostics
}

pub(super) fn validate_builtin_impl_authority(
    sources: &SourceMap,
    source: SourceId,
    ast: &AstFile,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> Vec<Diagnostic> {
    let actual_module = trusted_module_path(sources, source, options, resolved_nocter_home);
    ast.items
        .iter()
        .filter_map(|item| {
            let Item::Impl(impl_) = item else {
                return None;
            };
            builtin_target(&impl_.target_ty)
                .map(|(owner, module)| (owner, module, impl_.target_ty.span()))
        })
        .filter_map(|(owner, required_module, span)| {
            (actual_module.as_deref() != Some(required_module)).then(|| {
                let mut diagnostic = Diagnostic::error(
                    "E0416",
                    format!(
                        "implementations for built-in type `{owner}` are owned by `{required_module}`"
                    ),
                );
                diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
                diagnostic.help = Some(
                    "define behavior on a project-owned type; built-in type surfaces are supplied by the active Nocter home"
                        .to_string(),
                );
                diagnostic
            })
        })
        .collect()
}

fn builtin_target(target: &TypeExpr) -> Option<(&'static str, &'static str)> {
    match target {
        TypeExpr::Reference(reference) if reference.name == "str" => {
            Some(BUILTIN_IMPLEMENTATION_MODULES[0])
        }
        TypeExpr::View(_) => Some(BUILTIN_IMPLEMENTATION_MODULES[1]),
        _ => None,
    }
}

fn module_path(source: SourceId, value: &str) -> ModulePath {
    let span = ByteSpan::new(source, 0, 0);
    ModulePath {
        span,
        value: value.to_string(),
        segments: value.split('/').map(str::to_string).collect(),
        segment_spans: value.split('/').map(|_| span).collect(),
    }
}
