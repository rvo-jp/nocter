use super::FrontendOptions;
use super::dependencies::SourceDependencyTrace;
use super::diagnostics::{
    ImportPathKind, ambiguous_import_diagnostic, import_load_diagnostic,
    nocter_home_import_diagnostic, package_absolute_import_without_package_diagnostic,
    package_import_escape_diagnostic, relative_import_without_file_path_diagnostic,
    source_import_outside_module_diagnostic, undeclared_dependency_diagnostic,
};
use crate::ast::{AstFile, Block, Expr, Item, ModulePath, Stmt};
use crate::diagnostics::Diagnostic;
use crate::home::resolve_nocter_home;
use crate::resolve::ImportAccess;
use crate::source::{SourceId, SourceMap};
use std::path::{Path, PathBuf};

pub(super) fn import_paths(ast: &AstFile) -> Vec<&ModulePath> {
    let mut paths = Vec::new();
    for item in &ast.items {
        collect_item_import_paths(item, &mut paths);
    }
    paths
}

fn collect_item_import_paths<'a>(item: &'a Item, paths: &mut Vec<&'a ModulePath>) {
    match item {
        Item::Import(item) => paths.push(&item.path),
        Item::FromImport(item) => paths.push(&item.path),
        Item::Function(function) => {
            if let Some(body) = &function.body {
                collect_block_import_paths(body, paths);
            }
        }
        Item::Test(test) => collect_block_import_paths(&test.body, paths),
        Item::Instance(instance) => {
            for method in instance.callables() {
                if let Some(body) = &method.body {
                    collect_block_import_paths(body, paths);
                }
            }
        }
        Item::Destruct(destruct) => collect_block_import_paths(&destruct.body, paths),
        Item::Conformance(conformance) => {
            for member in &conformance.members {
                if let crate::ast::ConformanceMember::Method(method) = member
                    && let Some(body) = &method.body
                {
                    collect_block_import_paths(body, paths);
                }
            }
        }
        Item::Interface(interface) => {
            for method in &interface.methods {
                if let Some(body) = &method.body {
                    collect_block_import_paths(body, paths);
                }
            }
        }
        Item::Construct(construct) => {
            for (_, function) in construct.functions() {
                if let Some(body) = &function.body {
                    collect_block_import_paths(body, paths);
                }
            }
            for (_, literal) in construct.literals() {
                if let Some(body) = &literal.body {
                    collect_block_import_paths(body, paths);
                }
            }
        }
        Item::Primitive(_) | Item::TypeAlias(_) | Item::Struct(_) | Item::Enum(_) => {}
    }
}

fn collect_block_import_paths<'a>(block: &'a Block, paths: &mut Vec<&'a ModulePath>) {
    for statement in &block.statements {
        match statement {
            Stmt::Import(item) => paths.push(&item.path),
            Stmt::FromImport(item) => paths.push(&item.path),
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    collect_expression_import_paths(expression, paths);
                }
            }
            Stmt::Binding(statement) => {
                collect_expression_import_paths(&statement.initializer, paths);
            }
            Stmt::Assignment(statement) => {
                collect_expression_import_paths(&statement.target, paths);
                collect_expression_import_paths(&statement.value, paths);
            }
            Stmt::If(statement) => {
                collect_expression_import_paths(&statement.condition, paths);
                collect_block_import_paths(&statement.then_block, paths);
                if let Some(else_block) = &statement.else_block {
                    collect_block_import_paths(else_block, paths);
                }
            }
            Stmt::IfIs(statement) => {
                collect_expression_import_paths(&statement.expression, paths);
                collect_block_import_paths(&statement.then_block, paths);
                if let Some(else_block) = &statement.else_block {
                    collect_block_import_paths(else_block, paths);
                }
            }
            Stmt::Switch(statement) => {
                collect_expression_import_paths(&statement.expression, paths);
                for arm in &statement.arms {
                    collect_block_import_paths(&arm.body, paths);
                }
                if let Some(wildcard_arm) = &statement.wildcard_arm {
                    collect_block_import_paths(&wildcard_arm.body, paths);
                }
            }
            Stmt::ForRange(statement) => {
                collect_expression_import_paths(&statement.start, paths);
                collect_expression_import_paths(&statement.end, paths);
                collect_block_import_paths(&statement.body, paths);
            }
            Stmt::CollectionFor(statement) => {
                collect_expression_import_paths(&statement.source, paths);
                collect_block_import_paths(&statement.body, paths);
            }
            Stmt::LiteralPackFor(statement) => {
                collect_block_import_paths(&statement.body, paths);
            }
            Stmt::While(statement) => {
                collect_expression_import_paths(&statement.condition, paths);
                collect_block_import_paths(&statement.body, paths);
            }
            Stmt::Loop(statement) => collect_block_import_paths(&statement.body, paths),
            Stmt::Region(statement) => {
                collect_expression_import_paths(&statement.allocator, paths);
                collect_block_import_paths(&statement.body, paths);
            }
            Stmt::Expression(statement) => {
                collect_expression_import_paths(&statement.expression, paths);
            }
            Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
        }
    }

    if let Some(result) = &block.result {
        collect_expression_import_paths(result, paths);
    }
}

fn collect_expression_import_paths<'a>(expression: &'a Expr, paths: &mut Vec<&'a ModulePath>) {
    match expression {
        Expr::Closure(expression) => collect_block_import_paths(&expression.body, paths),
        Expr::Catch(expression) => {
            collect_expression_import_paths(&expression.expression, paths);
            collect_block_import_paths(&expression.catch_block, paths);
        }
        Expr::Otherwise(expression) => {
            collect_expression_import_paths(&expression.value, paths);
            collect_block_import_paths(&expression.fallback, paths);
        }
        Expr::If(expression) => {
            collect_expression_import_paths(&expression.condition, paths);
            collect_block_import_paths(&expression.then_block, paths);
            if let Some(else_block) = &expression.else_block {
                collect_block_import_paths(else_block, paths);
            }
        }
        Expr::IfIs(expression) => {
            collect_expression_import_paths(&expression.expression, paths);
            collect_block_import_paths(&expression.then_block, paths);
            if let Some(else_block) = &expression.else_block {
                collect_block_import_paths(else_block, paths);
            }
        }
        Expr::Match(expression) => {
            collect_expression_import_paths(&expression.expression, paths);
            for arm in &expression.arms {
                collect_block_import_paths(&arm.body, paths);
            }
            if let Some(wildcard_arm) = &expression.wildcard_arm {
                collect_block_import_paths(&wildcard_arm.body, paths);
            }
        }
        Expr::Propagate(expression) => {
            collect_expression_import_paths(&expression.expression, paths)
        }
        Expr::Force(expression) => collect_expression_import_paths(&expression.expression, paths),
        Expr::Borrow(expression) => collect_expression_import_paths(&expression.expression, paths),
        Expr::Unary(expression) => collect_expression_import_paths(&expression.operand, paths),
        Expr::Binary(expression) => {
            collect_expression_import_paths(&expression.left, paths);
            collect_expression_import_paths(&expression.right, paths);
        }
        Expr::TypeConversion(expression) => {
            collect_expression_import_paths(&expression.expression, paths);
        }
        Expr::Call(expression) => {
            collect_expression_import_paths(&expression.callee, paths);
            for argument in &expression.arguments {
                collect_expression_import_paths(argument, paths);
            }
        }
        Expr::Member(expression) => collect_expression_import_paths(&expression.object, paths),
        Expr::Index(expression) => {
            collect_expression_import_paths(&expression.object, paths);
            collect_expression_import_paths(&expression.index, paths);
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_import_paths(element, paths);
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_import_paths(element, paths);
            }
            if let Some(using) = &expression.using {
                collect_expression_import_paths(&using.allocator, paths);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                collect_expression_import_paths(&using.allocator, paths);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression_import_paths(&field.value, paths);
            }
        }
        Expr::Group(expression) => collect_expression_import_paths(&expression.expression, paths),
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    collect_expression_import_paths(&part.expression, paths);
                }
            }
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

pub(super) fn resolve_import_path(
    sources: &SourceMap,
    source: SourceId,
    path: &ModulePath,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
    dependencies: &mut SourceDependencyTrace,
) -> Result<PathBuf, Diagnostic> {
    if is_relative_module_path(&path.value) {
        let Some(resolved_path) = resolve_relative_import_path(sources, source, path) else {
            return Err(relative_import_without_file_path_diagnostic(
                sources, path.span,
            ));
        };

        let resolved =
            resolve_module_candidate(sources, resolved_path, dependencies).map_err(|error| {
                match error {
                    ImportResolutionError::Missing { candidates, error } => import_load_diagnostic(
                        sources,
                        path.span,
                        &path.value,
                        &candidates,
                        error,
                        ImportPathKind::Relative,
                    ),
                    ImportResolutionError::Ambiguous { file, directory } => {
                        ambiguous_import_diagnostic(
                            sources,
                            path.span,
                            &path.value,
                            &file,
                            &directory,
                        )
                    }
                }
            })?;
        if resolved.file_name().is_none_or(|name| name != "index.nct")
            && !source_paths_share_module(sources, source, &resolved)
        {
            return Err(source_import_outside_module_diagnostic(
                sources,
                path.span,
                &path.value,
            ));
        }
        return ensure_inside_package(sources, source, path, options, resolved);
    }

    if is_absolute_module_path(&path.value) {
        let Some(root) = package_root_for_source(sources, source, options) else {
            return Err(package_absolute_import_without_package_diagnostic(
                sources, path.span,
            ));
        };
        let resolved = resolve_directory_module_candidate(
            sources,
            root.join(path.value.trim_start_matches('/')),
            dependencies,
        )
        .map_err(|error| match error {
            ImportResolutionError::Missing { candidates, error } => import_load_diagnostic(
                sources,
                path.span,
                &path.value,
                &candidates,
                error,
                ImportPathKind::PackageAbsolute,
            ),
            ImportResolutionError::Ambiguous { file, directory } => {
                ambiguous_import_diagnostic(sources, path.span, &path.value, &file, &directory)
            }
        })?;
        return ensure_inside_root(sources, path, &root, resolved);
    }

    if let Some((dependency, remainder)) = dependency_import(sources, source, &path.value, options)
    {
        let candidate = if remainder.is_empty() {
            let path = dependency.root_module().source_path().to_path_buf();
            dependencies.record_path(&path);
            path
        } else {
            match resolve_directory_module_candidate(
                sources,
                dependency.root().join(remainder),
                dependencies,
            ) {
                Ok(path) => path,
                Err(ImportResolutionError::Missing { candidates, error }) => {
                    return Err(import_load_diagnostic(
                        sources,
                        path.span,
                        &path.value,
                        &candidates,
                        error,
                        ImportPathKind::NonRelative,
                    ));
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
        };
        return ensure_inside_root(sources, path, dependency.root(), candidate);
    }

    if !path.value.starts_with("std/") && path.value != "std" {
        return Err(undeclared_dependency_diagnostic(
            sources,
            path.span,
            &path.value,
        ));
    }

    let home = active_nocter_home(options, resolved_nocter_home).map_err(|message| {
        nocter_home_import_diagnostic(sources, path.span, &path.value, message)
    })?;

    resolve_directory_module_candidate(sources, home.join(&path.value), dependencies).map_err(
        |error| match error {
            ImportResolutionError::Missing { candidates, error } => import_load_diagnostic(
                sources,
                path.span,
                &path.value,
                &candidates,
                error,
                ImportPathKind::NonRelative,
            ),
            ImportResolutionError::Ambiguous { file, directory } => {
                ambiguous_import_diagnostic(sources, path.span, &path.value, &file, &directory)
            }
        },
    )
}

fn ensure_inside_package(
    sources: &SourceMap,
    source: SourceId,
    import: &ModulePath,
    options: &FrontendOptions,
    resolved: PathBuf,
) -> Result<PathBuf, Diagnostic> {
    let Some(root) = package_root_for_source(sources, source, options) else {
        return Ok(resolved);
    };
    ensure_inside_root(sources, import, &root, resolved)
}

fn ensure_inside_root(
    sources: &SourceMap,
    import: &ModulePath,
    root: &Path,
    resolved: PathBuf,
) -> Result<PathBuf, Diagnostic> {
    let owner = resolved
        .parent()
        .and_then(|directory| {
            directory
                .ancestors()
                .find(|ancestor| ancestor.join("nocter.nct").is_file())
        })
        .map(canonicalize_existing);
    if resolved.starts_with(root)
        && owner
            .as_deref()
            .is_none_or(|owner| owner == canonicalize_existing(root))
    {
        Ok(resolved)
    } else {
        Err(package_import_escape_diagnostic(
            sources,
            import.span,
            &import.value,
        ))
    }
}

fn package_root_for_source(
    sources: &SourceMap,
    source: SourceId,
    options: &FrontendOptions,
) -> Option<PathBuf> {
    let path = sources.get(source)?.absolute_path()?;
    semantic_package_root(path, options)
}

fn dependency_import<'a>(
    sources: &SourceMap,
    source: SourceId,
    value: &'a str,
    options: &'a FrontendOptions,
) -> Option<(&'a crate::package::SourcePackage, &'a str)> {
    let graph = options.package_graph.as_ref()?;
    let source_path = sources.get(source)?.absolute_path()?;
    let owner = graph.package_containing(source_path)?;
    let (name, remainder) = value.split_once('/').unwrap_or((value, ""));
    graph
        .dependency(owner.id(), name)
        .map(|package| (package, remainder))
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
    use_source: SourceId,
    declaration_source: SourceId,
    options: &FrontendOptions,
    resolved_nocter_home: &Option<Result<PathBuf, String>>,
) -> ImportAccess {
    let Some(use_path) = sources
        .get(use_source)
        .and_then(|file| file.absolute_path())
        .map(|path| canonicalize_existing(path))
    else {
        return ImportAccess::Public;
    };
    let Some(declaration_path) = sources
        .get(declaration_source)
        .and_then(|file| file.absolute_path())
        .map(|path| canonicalize_existing(path))
    else {
        return ImportAccess::Public;
    };
    let standard_library_root = options
        .nocter_home
        .as_ref()
        .or_else(|| {
            resolved_nocter_home
                .as_ref()
                .and_then(|result| result.as_ref().ok())
        })
        .map(|home| canonicalize_existing(&home.join("std")));
    let Some(use_root) =
        semantic_package_root_with_std(&use_path, options, standard_library_root.as_deref())
    else {
        return ImportAccess::Public;
    };
    let Some(declaration_root) = semantic_package_root_with_std(
        &declaration_path,
        options,
        standard_library_root.as_deref(),
    ) else {
        return ImportAccess::Public;
    };
    if use_root != declaration_root {
        return ImportAccess::Public;
    }

    let Some(use_module) = semantic_module_components(&use_path, &use_root) else {
        return ImportAccess::Public;
    };
    let Some(declaration_module) = semantic_module_components(&declaration_path, &declaration_root)
    else {
        return ImportAccess::Public;
    };
    let common = use_module
        .iter()
        .zip(&declaration_module)
        .take_while(|(left, right)| left == right)
        .count();
    let required_parent_levels = declaration_module.len().saturating_sub(common);
    let Ok(required_parent_levels) = u16::try_from(required_parent_levels) else {
        return ImportAccess::Public;
    };
    ImportAccess::Package {
        required_parent_levels,
    }
}

pub(super) fn semantic_package_root(path: &Path, options: &FrontendOptions) -> Option<PathBuf> {
    let standard_library_root = options
        .nocter_home
        .as_ref()
        .map(|home| canonicalize_existing(&home.join("std")));
    semantic_package_root_with_std(path, options, standard_library_root.as_deref())
}

fn semantic_package_root_with_std(
    path: &Path,
    options: &FrontendOptions,
    standard_library_root: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(std_root) = standard_library_root.filter(|root| path.starts_with(root)) {
        return Some(std_root.to_path_buf());
    }
    if let Some(package) = options
        .package_graph
        .as_ref()
        .and_then(|graph| graph.package_containing(path))
    {
        return Some(canonicalize_existing(package.root()));
    }
    path.parent()?
        .ancestors()
        .find(|directory| directory.join("nocter.nct").is_file())
        .map(canonicalize_existing)
}

pub(super) fn semantic_module_components(path: &Path, package_root: &Path) -> Option<Vec<String>> {
    crate::source_scopes::semantic_module_components(path, package_root)
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

fn source_paths_share_module(sources: &SourceMap, source: SourceId, imported: &Path) -> bool {
    let Some(source_path) = sources.get(source).and_then(|file| file.absolute_path()) else {
        return false;
    };
    module_root_source(sources, source_path) == module_root_source(sources, imported)
}

fn module_root_source(sources: &SourceMap, source: &Path) -> Option<PathBuf> {
    if crate::source_layout::is_module_root_source(source) {
        return known_or_existing_source_path(sources, source);
    }
    source
        .parent()?
        .ancestors()
        .find_map(|directory| known_or_existing_source_path(sources, &directory.join("index.nct")))
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

fn resolve_module_candidate(
    sources: &SourceMap,
    module_path: PathBuf,
    dependencies: &mut SourceDependencyTrace,
) -> Result<PathBuf, ImportResolutionError> {
    dependencies.record_module_candidates(&module_path);
    let file = module_path.with_extension("nct");
    let index = module_path.join("index.nct");
    let candidates = vec![file.clone(), index.clone()];
    let file = canonicalize_candidate(sources, file)?;
    let index = canonicalize_candidate(sources, index)?;

    match (file, index) {
        (Some(file), Some(index)) => Err(ImportResolutionError::Ambiguous {
            file,
            directory: index
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
        }),
        (Some(file), None) => {
            dependencies.record_path(&file);
            Ok(file)
        }
        (None, Some(index)) => {
            dependencies.record_path(&index);
            Ok(index)
        }
        (None, None) => Err(ImportResolutionError::Missing {
            candidates,
            error: "file was not found in any import root".to_string(),
        }),
    }
}

fn resolve_directory_module_candidate(
    sources: &SourceMap,
    module_path: PathBuf,
    dependencies: &mut SourceDependencyTrace,
) -> Result<PathBuf, ImportResolutionError> {
    dependencies.record_module_candidates(&module_path);
    let index = module_path.join("index.nct");
    let candidates = vec![index.clone()];
    match canonicalize_candidate(sources, index)? {
        Some(index) => {
            dependencies.record_path(&index);
            Ok(index)
        }
        None => Err(ImportResolutionError::Missing {
            candidates,
            error: "directory module root source file was not found".to_string(),
        }),
    }
}

fn canonicalize_candidate(
    sources: &SourceMap,
    path: PathBuf,
) -> Result<Option<PathBuf>, ImportResolutionError> {
    if let Some(path) = known_source_path(sources, &path) {
        return Ok(Some(path));
    }
    match path.canonicalize() {
        Ok(path) => Ok(Some(path)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ImportResolutionError::Missing {
            candidates: vec![path],
            error: error.to_string(),
        }),
    }
}

fn known_or_existing_source_path(sources: &SourceMap, path: &Path) -> Option<PathBuf> {
    known_source_path(sources, path)
        .map(|path| crate::source_layout::canonicalize_with_missing_suffix(&path))
        .or_else(|| path.canonicalize().ok())
}

fn known_source_path(sources: &SourceMap, path: &Path) -> Option<PathBuf> {
    let candidate = crate::source_layout::canonicalize_with_missing_suffix(path);
    sources
        .sources_with_absolute_paths()
        .map(|(path, _)| path)
        .find(|path| crate::source_layout::canonicalize_with_missing_suffix(path) == candidate)
        .map(Path::to_path_buf)
}
