use nocter_diagnostics::SourceDiagnostic;
use nocter_source_index::SourceOrigin;
use nocter_syntax::SyntaxTree;

use crate::syntax::use_path_node;
use crate::{DiscoveryError, ImportFailure};

pub(crate) fn discovery_diagnostics(
    error: &DiscoveryError,
    trees: &[SyntaxTree],
) -> Result<Box<[SourceDiagnostic]>, DiscoveryError> {
    let DiscoveryError::Import {
        declaration,
        path,
        failure,
    } = error
    else {
        return Ok(Box::new([]));
    };
    let tree = trees
        .iter()
        .find(|tree| tree.source() == declaration.source())
        .ok_or(DiscoveryError::InconsistentSyntax(*declaration))?;
    let path_node = use_path_node(tree, *declaration)
        .ok_or(DiscoveryError::InconsistentSyntax(*declaration))?;
    let origin = SourceOrigin::from_node(tree, path_node)
        .map_err(|_| DiscoveryError::InconsistentSyntax(*declaration))?;
    let message = format!(
        "cannot resolve module path `{path}`: {}",
        import_failure_message(failure)
    );
    Ok(vec![SourceDiagnostic::new(
        "E0263",
        message,
        origin,
        [],
        Some(import_failure_help(failure)),
    )]
    .into_boxed_slice())
}

fn import_failure_message(failure: &ImportFailure) -> Box<str> {
    match failure {
        ImportFailure::UnknownDependency { alias } => {
            format!("dependency alias `{alias}` is not declared").into()
        }
        ImportFailure::OutsidePackage => "the path escapes its package boundary".into(),
        ImportFailure::NotFound => "no source or module exists at that path".into(),
        ImportFailure::Ambiguous { .. } => {
            "both a source file and a directory module exist at that path".into()
        }
        ImportFailure::CrossesPackage { .. } => {
            "a relative source path crosses a package boundary".into()
        }
        ImportFailure::CrossesModule { .. } => {
            "a source import crosses a directory-module boundary".into()
        }
        ImportFailure::InvalidModuleDirectory => {
            "the path does not identify a valid module directory".into()
        }
        ImportFailure::SingleFileLocalImport => {
            "single-file mode cannot load a local source graph".into()
        }
    }
}

const fn import_failure_help(failure: &ImportFailure) -> &'static str {
    match failure {
        ImportFailure::UnknownDependency { .. } => {
            "declare the dependency alias in nocter.nct or correct the first path segment"
        }
        ImportFailure::Ambiguous { .. } => {
            "remove one candidate or choose a path that identifies exactly one source or module"
        }
        ImportFailure::SingleFileLocalImport => {
            "use package mode when source code spans more than one file"
        }
        ImportFailure::OutsidePackage
        | ImportFailure::NotFound
        | ImportFailure::CrossesPackage { .. }
        | ImportFailure::CrossesModule { .. }
        | ImportFailure::InvalidModuleDirectory => {
            "change the module path to one valid source or directory module within its boundary"
        }
    }
}
