//! Front-end source loading, parsing, and compile-unit construction.

mod builtin_surfaces;
mod dependencies;
mod diagnostics;
mod imports;
mod module_discovery;
mod parsing;
mod prelude;

#[cfg(test)]
mod tests;

use crate::analysis::CompileUnit;
use crate::ast::{AstFile, ConformanceMember, Item, Visibility};
use crate::diagnostics::Diagnostic;
use crate::resolve::{ImportKind, ImportSource, ImportSourceMap, PreludeSourceMap};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::target::DEFAULT_TARGET;
use crate::target::primitive::validate_primitive_declaration;
use crate::target::trusted::trusted_declarations_for_module;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

use builtin_surfaces::{
    enqueue_builtin_surface_sources, validate_builtin_conformance_authority,
    validate_builtin_construction_authority, validate_builtin_instance_authority,
};
use dependencies::SourceDependencyTrace;
pub(crate) use dependencies::dependency_path_aliases;
use diagnostics::{
    import_source_diagnostic, invalid_source_import_declaration_diagnostic,
    invalid_visibility_boundary_diagnostic, primitive_outside_nocter_home_diagnostic,
    primitive_registry_diagnostic, public_declaration_outside_module_root_diagnostic,
};
use imports::{
    active_nocter_home, canonicalize_existing, import_access_for_source, import_paths,
    resolve_import_path,
};
use parsing::{parse_package_source_for_check, parse_source_for_check};
use prelude::{should_load_prelude, standard_prelude_path};

pub(crate) use module_discovery::module_segment_candidates;

#[derive(Debug, Clone)]
pub(crate) struct FrontendOptions {
    pub(crate) nocter_home: Option<PathBuf>,
    pub(crate) package_graph: Option<crate::package::PackageGraph>,
    pub(crate) target: String,
}

impl Default for FrontendOptions {
    fn default() -> Self {
        Self {
            nocter_home: None,
            package_graph: None,
            target: DEFAULT_TARGET.to_string(),
        }
    }
}

pub(crate) fn load_compile_unit(
    sources: &mut SourceMap,
    root: SourceId,
    options: &FrontendOptions,
) -> Result<CompileUnit, Vec<Diagnostic>> {
    load_compile_unit_with_trace(sources, root, options).result
}

pub(crate) struct CompileUnitLoad {
    pub(crate) result: Result<CompileUnit, Vec<Diagnostic>>,
    pub(crate) loaded_sources: HashSet<SourceId>,
    pub(crate) dependency_paths: HashSet<PathBuf>,
}

pub(crate) fn load_compile_unit_with_trace(
    sources: &mut SourceMap,
    root: SourceId,
    options: &FrontendOptions,
) -> CompileUnitLoad {
    let mut queue = VecDeque::from([root]);
    let mut dependencies = SourceDependencyTrace::default();
    dependencies.record_source(root);
    let mut loaded_sources_by_path = std::collections::HashMap::new();
    let mut import_sources = ImportSourceMap::new();
    let mut prelude_sources = PreludeSourceMap::new();
    let mut resolved_nocter_home = None;
    let mut diagnostics = Vec::new();
    let mut root_ast = None;
    let mut files = Vec::new();
    let mut trusted_declarations = crate::semantics::TrustedDeclarationInputs::default();

    for (path, source) in sources.sources_with_absolute_paths() {
        loaded_sources_by_path.insert(path.to_path_buf(), source);
    }

    diagnostics.extend(enqueue_builtin_surface_sources(
        sources,
        root,
        options,
        &mut resolved_nocter_home,
        &mut loaded_sources_by_path,
        &mut dependencies,
        &mut queue,
    ));

    while let Some(source) = queue.pop_front() {
        let parse_result = if source_is_package_file(sources, source, options) {
            parse_package_source_for_check(sources, source)
        } else {
            parse_source_for_check(sources, source)
        };
        let mut ast = match parse_result {
            Ok(ast) => ast,
            Err(source_diagnostics) => {
                diagnostics.extend(source_diagnostics);
                continue;
            }
        };

        filter_target_items(&mut ast, &options.target);

        diagnostics.extend(validate_public_declarations_in_module_root(
            sources, source, &ast,
        ));

        diagnostics.extend(validate_visibility_boundaries(
            sources,
            source,
            &ast,
            options,
            &resolved_nocter_home,
        ));

        diagnostics.extend(validate_builtin_instance_authority(
            sources,
            source,
            &ast,
            options,
            &mut resolved_nocter_home,
        ));
        diagnostics.extend(validate_builtin_construction_authority(
            sources,
            source,
            &ast,
            options,
            &mut resolved_nocter_home,
        ));

        diagnostics.extend(validate_builtin_conformance_authority(
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

        if let Some(module_path) =
            trusted_module_path(sources, source, options, &mut resolved_nocter_home)
        {
            trusted_declarations.extend(trusted_declarations_for_module(&module_path, &ast));
        }

        if should_load_prelude(sources, source, options, &mut resolved_nocter_home) {
            let path = standard_prelude_path(source);
            match resolve_import_path(
                sources,
                source,
                &path,
                options,
                &mut resolved_nocter_home,
                &mut dependencies,
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
                                    imported,
                                    options,
                                    &resolved_nocter_home,
                                ),
                                kind: ImportKind::Module,
                            },
                        );

                        if dependencies.record_source(imported) {
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
                &mut resolved_nocter_home,
                &mut dependencies,
            ) {
                Ok(path) => path,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };

            let import_kind = if crate::source_layout::is_module_root_source(&canonical) {
                ImportKind::Module
            } else {
                ImportKind::Source
            };
            if import_kind == ImportKind::Source
                && !valid_source_import_declaration(&ast, path.span)
            {
                diagnostics.push(invalid_source_import_declaration_diagnostic(
                    sources, path.span,
                ));
            }
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
                        imported,
                        options,
                        &resolved_nocter_home,
                    ),
                    kind: import_kind,
                },
            );

            if dependencies.record_source(imported) {
                queue.push_back(imported);
            }
        }
    }

    if !diagnostics.is_empty() {
        let (loaded_sources, dependency_paths) = dependencies.into_parts();
        return CompileUnitLoad {
            result: Err(diagnostics),
            loaded_sources,
            dependency_paths,
        };
    }

    let (callable_bodies, callable_body_diagnostics) =
        crate::callable_bodies::CallableBodyIndex::build(sources, &files, &import_sources);
    if !callable_body_diagnostics.is_empty() {
        let (loaded_sources, dependency_paths) = dependencies.into_parts();
        return CompileUnitLoad {
            result: Err(callable_body_diagnostics),
            loaded_sources,
            dependency_paths,
        };
    }

    let Some(root_ast) = root_ast else {
        let (loaded_sources, dependency_paths) = dependencies.into_parts();
        return CompileUnitLoad {
            result: Err(vec![Diagnostic::error(
                "E0200",
                "root source did not produce an AST and did not report a diagnostic",
            )]),
            loaded_sources,
            dependency_paths,
        };
    };

    let nocter_home = options
        .nocter_home
        .as_ref()
        .map(|home| canonicalize_existing(home))
        .or_else(|| {
            resolved_nocter_home
                .as_ref()
                .and_then(|home| home.as_ref().ok())
                .map(|home| canonicalize_existing(home))
        });

    let trusted_modules = files
        .iter()
        .filter(|ast| {
            sources
                .get(ast.span.source)
                .and_then(|source| source.absolute_path())
                .is_none_or(|path| crate::source_layout::is_module_root_source(path))
        })
        .filter_map(|ast| {
            trusted_module_path(sources, ast.span.source, options, &mut resolved_nocter_home)
                .map(|path| (path, ast))
        })
        .collect();
    crate::target::trusted_interpolation::attach_interpolation_runtime(
        &trusted_modules,
        &mut trusted_declarations,
    );
    crate::target::trusted_iteration::attach_iteration_runtime(
        &trusted_modules,
        &mut trusted_declarations,
    );
    let standard_library_root = nocter_home.as_ref().map(|home| home.join("std"));
    let source_scopes = crate::source_scopes::SourceScopeMap::new(
        sources,
        files.iter().map(|file| file.span.source),
        options.package_graph.as_ref(),
        standard_library_root.as_deref(),
    );
    let (loaded_sources, dependency_paths) = dependencies.into_parts();
    CompileUnitLoad {
        result: Ok(CompileUnit::new(
            root_ast,
            files,
            import_sources,
            prelude_sources,
            nocter_home,
        )
        .with_callable_bodies(callable_bodies)
        .with_source_scopes(source_scopes)
        .with_trusted_declarations(trusted_declarations)),
        loaded_sources,
        dependency_paths,
    }
}

fn valid_source_import_declaration(ast: &AstFile, path_span: ByteSpan) -> bool {
    ast.items.iter().any(|item| {
        matches!(
            item,
            Item::Import(import)
                if import.path.span == path_span
                    && import.visibility == Visibility::Private
                    && import.alias_is_default
        )
    })
}

fn trusted_module_path(
    sources: &SourceMap,
    source: SourceId,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> Option<String> {
    let home = active_nocter_home(options, resolved_nocter_home).ok()?;
    let scopes = crate::source_scopes::SourceScopeMap::new(
        sources,
        [source],
        options.package_graph.as_ref(),
        Some(&home.join("std")),
    );
    scopes.standard_library_module_path(source)
}

fn source_is_package_file(
    sources: &SourceMap,
    source: SourceId,
    options: &FrontendOptions,
) -> bool {
    let Some(path) = sources.get(source).and_then(|file| file.absolute_path()) else {
        return false;
    };
    path.file_name().is_some_and(|name| name == "nocter.nct")
        || options
            .package_graph
            .as_ref()
            .is_some_and(|graph| graph.is_package_file(path))
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

fn validate_public_declarations_in_module_root(
    sources: &SourceMap,
    source: SourceId,
    ast: &AstFile,
) -> Vec<Diagnostic> {
    let Some(path) = sources.get(source).and_then(|file| file.absolute_path()) else {
        // Virtual sources used by embedding clients have no filesystem layout
        // from which a module role can be derived. Their caller owns that role.
        return Vec::new();
    };
    if crate::source_layout::is_module_root_source(path) {
        return Vec::new();
    }

    public_declaration_spans(ast)
        .into_iter()
        .map(|span| public_declaration_outside_module_root_diagnostic(sources, span))
        .collect()
}

fn public_declaration_spans(ast: &AstFile) -> Vec<ByteSpan> {
    let mut spans = Vec::new();
    let is_public = |visibility: Visibility| visibility != Visibility::Private;
    for item in &ast.items {
        match item {
            Item::Import(import) if is_public(import.visibility) => spans.push(import.span),
            Item::FromImport(import) if is_public(import.visibility) => spans.push(import.span),
            Item::Function(function) if is_public(function.visibility) => spans.push(function.span),
            Item::Primitive(primitive) if is_public(primitive.visibility) => {
                spans.push(primitive.span)
            }
            Item::TypeAlias(alias) if is_public(alias.visibility) => spans.push(alias.span),
            Item::Struct(struct_) => {
                if is_public(struct_.visibility) {
                    spans.push(struct_.span);
                }
                spans.extend(
                    struct_
                        .fields
                        .iter()
                        .filter(|field| is_public(field.visibility))
                        .map(|field| field.span),
                );
            }
            Item::Enum(enum_) if is_public(enum_.visibility) => spans.push(enum_.span),
            Item::Interface(interface) => {
                if is_public(interface.visibility) {
                    spans.push(interface.span);
                }
                spans.extend(
                    interface
                        .methods
                        .iter()
                        .filter(|method| is_public(method.visibility))
                        .map(|method| method.span),
                );
            }
            Item::Instance(instance) => spans.extend(
                instance
                    .callable_methods()
                    .filter(|method| is_public(method.visibility))
                    .map(|method| method.span),
            ),
            Item::Conformance(conformance) => spans.extend(conformance.members.iter().filter_map(
                |member| match member {
                    ConformanceMember::Method(method) if is_public(method.visibility) => {
                        Some(method.span)
                    }
                    _ => None,
                },
            )),
            Item::Construct(construct) => {
                spans.extend(
                    construct
                        .functions()
                        .filter(|(_, function)| is_public(function.visibility))
                        .map(|(_, function)| function.span),
                );
                spans.extend(
                    construct
                        .literals()
                        .filter(|(_, literal)| is_public(literal.visibility))
                        .map(|(_, literal)| literal.span),
                );
            }
            _ => {}
        }
    }
    spans
}

fn validate_visibility_boundaries(
    sources: &SourceMap,
    source: SourceId,
    ast: &AstFile,
    options: &FrontendOptions,
    resolved_nocter_home: &Option<Result<PathBuf, String>>,
) -> Vec<Diagnostic> {
    let Some(path) = sources.get(source).and_then(|file| file.absolute_path()) else {
        return Vec::new();
    };
    let package_root = imports::semantic_package_root(path, options).or_else(|| {
        resolved_nocter_home
            .as_ref()
            .and_then(|home| home.as_ref().ok())
            .map(|home| canonicalize_existing(&home.join("std")))
            .filter(|root| path.starts_with(root))
    });
    let module_depth = package_root.as_ref().and_then(|root| {
        imports::semantic_module_components(path, root).map(|components| components.len())
    });
    declaration_visibilities(ast)
        .into_iter()
        .filter_map(|(visibility, span)| match visibility {
            Visibility::Package if package_root.is_none() => {
                Some(invalid_visibility_boundary_diagnostic(
                    sources,
                    span,
                    "`pub(/)` requires a package selected through `nocter.nct`",
                ))
            }
            Visibility::ModuleTree(parents)
                if module_depth.is_none_or(|depth| usize::from(parents) > depth) =>
            {
                Some(invalid_visibility_boundary_diagnostic(
                    sources,
                    span,
                    &format!(
                        "`{}` moves above the declaring package root",
                        visibility.source_notation()
                    ),
                ))
            }
            _ => None,
        })
        .collect()
}

fn declaration_visibilities(ast: &AstFile) -> Vec<(Visibility, ByteSpan)> {
    let mut declarations = Vec::new();
    for item in &ast.items {
        match item {
            Item::Import(import) => declarations.push((import.visibility, import.span)),
            Item::FromImport(import) => declarations.push((import.visibility, import.span)),
            Item::Function(function) => declarations.push((function.visibility, function.span)),
            Item::Primitive(primitive) => declarations.push((primitive.visibility, primitive.span)),
            Item::TypeAlias(alias) => declarations.push((alias.visibility, alias.span)),
            Item::Struct(struct_) => {
                declarations.push((struct_.visibility, struct_.span));
                declarations.extend(
                    struct_
                        .fields
                        .iter()
                        .map(|field| (field.visibility, field.span)),
                );
            }
            Item::Enum(enum_) => declarations.push((enum_.visibility, enum_.span)),
            Item::Interface(interface) => {
                declarations.push((interface.visibility, interface.span));
                declarations.extend(
                    interface
                        .methods
                        .iter()
                        .map(|method| (method.visibility, method.span)),
                );
            }
            Item::Instance(instance) => declarations.extend(
                instance
                    .callable_methods()
                    .map(|method| (method.visibility, method.span)),
            ),
            Item::Conformance(conformance) => declarations.extend(
                conformance
                    .members
                    .iter()
                    .filter_map(|member| match member {
                        ConformanceMember::Method(method) => Some((method.visibility, method.span)),
                        ConformanceMember::AssociatedType(_) => None,
                    }),
            ),
            Item::Construct(construct) => declarations.extend(construct.members.iter().map(
                |member| match &member.declaration {
                    crate::ast::ConstructMemberDecl::Function(function) => {
                        (function.visibility, function.span)
                    }
                    crate::ast::ConstructMemberDecl::Literal(literal) => {
                        (literal.visibility, literal.span)
                    }
                },
            )),
            Item::Test(_) => {}
            Item::Destruct(_) => {}
        }
    }
    declarations
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

    let scopes = crate::source_scopes::SourceScopeMap::new(
        sources,
        [source],
        options.package_graph.as_ref(),
        Some(&home.join("std")),
    );
    if !scopes.is_standard_library(source) {
        return primitives
            .into_iter()
            .map(|primitive| {
                primitive_outside_nocter_home_diagnostic(sources, primitive.span, &options.target)
            })
            .collect();
    }
    let module_path = scopes
        .standard_library_module_path(source)
        .expect("standard-library authority includes a module path");

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
