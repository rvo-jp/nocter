use nocter_diagnostics::SourceDiagnostic;
use nocter_source_index::SourceOrigin;
use nocter_syntax::SyntaxTree;

use crate::include::include_path_node;
use crate::syntax::use_path_node;
use crate::{DiscoveryError, IncludeFailure, UseFailure};

pub(crate) fn discovery_diagnostics(
    error: &DiscoveryError,
    trees: &[SyntaxTree],
) -> Result<Box<[SourceDiagnostic]>, DiscoveryError> {
    let (declaration, path_kind, code, message, help) = match error {
        DiscoveryError::Use {
            declaration,
            path,
            failure,
        } => (
            *declaration,
            nocter_syntax::NodeKind::ModulePath,
            "E0263",
            format!(
                "cannot resolve module path `{path}`: {}",
                use_failure_message(failure)
            ),
            use_failure_help(failure),
        ),
        DiscoveryError::Include {
            declaration,
            path,
            failure,
        } => (
            *declaration,
            nocter_syntax::NodeKind::IncludePath,
            "E0270",
            format!(
                "cannot resolve included source `{path}`: {}",
                include_failure_message(failure)
            ),
            include_failure_help(failure),
        ),
        _ => return Ok(Box::new([])),
    };
    let tree = trees
        .iter()
        .find(|tree| tree.source() == declaration.source())
        .ok_or(DiscoveryError::InconsistentSyntax(declaration))?;
    let path_node = match path_kind {
        nocter_syntax::NodeKind::ModulePath => use_path_node(tree, declaration),
        nocter_syntax::NodeKind::IncludePath => include_path_node(tree, declaration),
        _ => None,
    }
    .ok_or(DiscoveryError::InconsistentSyntax(declaration))?;
    let origin = SourceOrigin::from_node(tree, path_node)
        .map_err(|_| DiscoveryError::InconsistentSyntax(declaration))?;
    Ok(vec![SourceDiagnostic::new(code, message, origin, [], Some(help))].into_boxed_slice())
}

fn use_failure_message(failure: &UseFailure) -> Box<str> {
    match failure {
        UseFailure::UnknownDependency { alias } => {
            format!("dependency alias `{alias}` is not declared").into()
        }
        UseFailure::OutsidePackage => "the path escapes its package boundary".into(),
        UseFailure::NotFound => "no directory module exists at that path".into(),
        UseFailure::CrossesPackage { .. } => "the module path crosses a package boundary".into(),
        UseFailure::InvalidModuleDirectory => {
            "the path does not identify a valid module directory".into()
        }
        UseFailure::SingleFileLocalUse => {
            "single-file mode cannot import a package-local directory module".into()
        }
    }
}

const fn use_failure_help(failure: &UseFailure) -> &'static str {
    match failure {
        UseFailure::UnknownDependency { .. } => {
            "declare the dependency alias in nocter.nct or correct the first path segment"
        }
        UseFailure::SingleFileLocalUse => {
            "use a dependency module, or use `include ./file.nct` for a local source"
        }
        UseFailure::OutsidePackage
        | UseFailure::NotFound
        | UseFailure::CrossesPackage { .. }
        | UseFailure::InvalidModuleDirectory => {
            "change the module path to one valid directory module within its boundary"
        }
    }
}

fn include_failure_message(failure: &IncludeFailure) -> Box<str> {
    match failure {
        IncludeFailure::OutsidePackage => "the path escapes its package boundary".into(),
        IncludeFailure::NotFound => "the exact source file does not exist".into(),
        IncludeFailure::CrossesPackage { .. } => {
            "the source path crosses a package boundary".into()
        }
        IncludeFailure::CrossesModule { .. } => {
            "the source belongs to another directory module".into()
        }
    }
}

const fn include_failure_help(failure: &IncludeFailure) -> &'static str {
    match failure {
        IncludeFailure::CrossesModule { .. } => {
            "use the target directory module through `use`, or include a source owned by this module"
        }
        IncludeFailure::OutsidePackage
        | IncludeFailure::NotFound
        | IncludeFailure::CrossesPackage { .. } => {
            "name one existing source with an exact current-directory-relative `.nct` path"
        }
    }
}
