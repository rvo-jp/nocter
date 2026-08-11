//! Compiler-owned loading and declaration authority for built-in type source surfaces.

use super::dependencies::SourceDependencyTrace;
use super::diagnostics::import_source_diagnostic;
use super::imports::resolve_import_path;
use super::{FrontendOptions, trusted_module_path};
use crate::ast::{AstFile, Item, ModulePath};
use crate::builtin_types::BuiltinTypeOwner;
use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, SourceId, SourceMap};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

pub(super) fn enqueue_builtin_surface_sources(
    sources: &mut SourceMap,
    root: SourceId,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
    loaded_sources_by_path: &mut HashMap<PathBuf, SourceId>,
    dependencies: &mut SourceDependencyTrace,
    queue: &mut VecDeque<SourceId>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut loaded_modules = HashSet::new();
    for module in BuiltinTypeOwner::ALL
        .into_iter()
        .map(BuiltinTypeOwner::source_authority)
        .filter(|authority| authority.implicitly_loaded)
        .map(|authority| authority.module)
        .filter(|module| loaded_modules.insert(*module))
    {
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

pub(super) fn validate_builtin_conformance_authority(
    sources: &SourceMap,
    source: SourceId,
    ast: &AstFile,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> Vec<Diagnostic> {
    let is_standard_library =
        trusted_module_path(sources, source, options, resolved_nocter_home).is_some();

    ast.items
        .iter()
        .filter_map(|item| {
            let Item::Conformance(conformance) = item else {
                return None;
            };
            BuiltinTypeOwner::from_conformance_target(&conformance.target_ty)
                .map(|owner| (owner, conformance.target_ty.span()))
        })
        .filter(|(owner, _)| !owner.source_authority().conformance || !is_standard_library)
        .map(|(owner, span)| {
            let mut diagnostic = Diagnostic::error(
                "E0416",
                format!(
                    "conformances for built-in type `{}` are owned by the standard library package",
                    owner.canonical_name()
                ),
            );
            diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
            diagnostic.help = Some(
                "define the conformance for a project-owned type; built-in conformances are supplied by the active Nocter home"
                    .to_string(),
            );
            diagnostic
        })
        .collect()
}

pub(super) fn validate_builtin_instance_authority(
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
            let Item::Instance(instance) = item else {
                return None;
            };
            BuiltinTypeOwner::from_instance_target(&instance.target_ty).map(|owner| {
                let authority = owner.source_authority();
                (
                    owner,
                    authority,
                    instance.target_ty.span(),
                )
            })
        })
        .filter(|(_, authority, _)| {
            !authority.instance || actual_module.as_deref() != Some(authority.module)
        })
        .map(|(owner, authority, span)| {
            let owner_name = owner.canonical_name();
            let reason = if authority.instance {
                format!("instances for built-in type `{owner_name}` are owned by `{}`", authority.module)
            } else {
                format!("built-in type `{owner_name}` does not accept instance declarations")
            };
            let mut diagnostic = Diagnostic::error(
                "E0416",
                reason,
            );
            diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
            diagnostic.help = Some(
                "define behavior on a project-owned type; built-in type surfaces are supplied by the active Nocter home"
                    .to_string(),
            );
            diagnostic
        })
        .collect()
}

pub(super) fn validate_builtin_construction_authority(
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
            match item {
                Item::Construct(construct) => {
                    BuiltinTypeOwner::from_construction_target(&construct.target).map(|owner| {
                        (owner, owner.source_authority(), construct.target.span(), false)
                    })
                }
                Item::Function(function) => function.owner.as_ref().and_then(|function_owner| {
                    BuiltinTypeOwner::from_reference_name(&function_owner.name).map(|owner| {
                        (owner, owner.source_authority(), function_owner.name_span, true)
                    })
                }),
                _ => None,
            }
        })
        .filter(|(_, authority, _, detached)| {
            *detached
                || !authority.construction
                || actual_module.as_deref() != Some(authority.module)
        })
        .map(|(owner, authority, span, detached)| {
            let owner_name = owner.canonical_name();
            let reason = if detached {
                format!(
                    "associated construction for built-in type `{owner_name}` must be declared inside its authorized `construct` surface"
                )
            } else if authority.construction {
                format!(
                    "construction for built-in type `{owner_name}` is owned by `{}`",
                    authority.module
                )
            } else {
                format!("built-in type `{owner_name}` does not accept construct declarations")
            };
            let mut diagnostic = Diagnostic::error("E0416", reason);
            diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
            diagnostic.help = Some(
                "define construction for a project-owned type; built-in type surfaces are supplied by the active Nocter home"
                    .to_string(),
            );
            diagnostic
        })
        .collect()
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
