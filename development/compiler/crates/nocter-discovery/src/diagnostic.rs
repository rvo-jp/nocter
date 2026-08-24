use nocter_diagnostics::SourceDiagnostic;
use nocter_source_index::SourceOrigin;
use nocter_syntax::SyntaxTree;

use crate::source_visibility::source_visibility_path_node;
use crate::syntax::use_path_node;
use crate::{DiscoveryError, SourceVisibilityFailure, UseFailure};

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
        DiscoveryError::SourceVisibility {
            declaration,
            path,
            failure,
        } => (
            *declaration,
            nocter_syntax::NodeKind::SourceVisibilityPath,
            "E0270",
            format!(
                "cannot resolve visible source `{path}`: {}",
                source_visibility_failure_message(failure)
            ),
            source_visibility_failure_help(failure),
        ),
        _ => return Ok(Box::new([])),
    };
    let tree = trees
        .iter()
        .find(|tree| tree.source() == declaration.source())
        .ok_or(DiscoveryError::InconsistentSyntax(declaration))?;
    let path_node = match path_kind {
        nocter_syntax::NodeKind::ModulePath => use_path_node(tree, declaration),
        nocter_syntax::NodeKind::SourceVisibilityPath => {
            source_visibility_path_node(tree, declaration)
        }
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
            "declare the dependency alias in the package index or correct the first path segment"
        }
        UseFailure::SingleFileLocalUse => {
            "use a dependency module, or use `see ./file.nct` for a local source"
        }
        UseFailure::OutsidePackage
        | UseFailure::NotFound
        | UseFailure::CrossesPackage { .. }
        | UseFailure::InvalidModuleDirectory => {
            "change the module path to one valid directory module within its boundary"
        }
    }
}

fn source_visibility_failure_message(failure: &SourceVisibilityFailure) -> Box<str> {
    match failure {
        SourceVisibilityFailure::OutsidePackage => "the path escapes its package boundary".into(),
        SourceVisibilityFailure::NotFound => "the exact source file does not exist".into(),
        SourceVisibilityFailure::CrossesPackage { .. } => {
            "the source path crosses a package boundary".into()
        }
        SourceVisibilityFailure::CrossesModule { .. } => {
            "the source belongs to another directory module".into()
        }
    }
}

const fn source_visibility_failure_help(failure: &SourceVisibilityFailure) -> &'static str {
    match failure {
        SourceVisibilityFailure::CrossesModule { .. } => {
            "use the target directory module through `use`, or see a source owned by this module"
        }
        SourceVisibilityFailure::OutsidePackage
        | SourceVisibilityFailure::NotFound
        | SourceVisibilityFailure::CrossesPackage { .. } => {
            "name one existing source with an exact current-directory-relative `.nct` path"
        }
    }
}
