use super::FrontendOptions;
use super::diagnostics::{
    ImportPathKind, ambiguous_import_diagnostic, import_load_diagnostic,
    nocter_home_import_diagnostic, relative_import_without_file_path_diagnostic,
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
        Item::Function(function) => collect_block_import_paths(&function.body, paths),
        Item::Literal(literal) => collect_block_import_paths(&literal.body, paths),
        Item::Impl(impl_) => {
            for member in &impl_.members {
                match member {
                    crate::ast::ImplMember::Method(method) => {
                        if let Some(body) = &method.body {
                            collect_block_import_paths(body, paths);
                        }
                    }
                    crate::ast::ImplMember::Drop(drop_) => {
                        collect_block_import_paths(&drop_.body, paths);
                    }
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
    if !source_is_inside_current_nocter_home(sources, source, options, resolved_nocter_home)
        && let Some(root) = source_root
    {
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

fn source_is_inside_current_nocter_home(
    sources: &SourceMap,
    source: SourceId,
    options: &FrontendOptions,
    resolved_nocter_home: &Option<Result<PathBuf, String>>,
) -> bool {
    let Some(home) = current_nocter_home(options, resolved_nocter_home) else {
        return false;
    };
    let Some(source_path) = sources
        .get(source)
        .and_then(|file| file.absolute_path())
        .map(|path| canonicalize_existing(path))
    else {
        return false;
    };

    source_path.starts_with(home)
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
