#[cfg(test)]
use super::diagnostics::{LspDiagnostic, diagnostics_for_lsp};
use super::documents::OpenDocument;
use crate::analysis::{CompileUnitAnalysis, FileAnalysis, analyze_module_compile_unit};
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::package::PackageGraph;
use crate::source::{SourceId, SourceMap};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub(super) struct LspWorkspaceAnalysis {
    pub(super) sources: SourceMap,
    analysis: Option<CompileUnitAnalysis>,
    diagnostics: Vec<Diagnostic>,
    source_paths: HashSet<std::path::PathBuf>,
}

impl LspWorkspaceAnalysis {
    pub(super) fn root_file(&self) -> Option<&FileAnalysis> {
        self.analysis.as_ref()?.root_file()
    }

    pub(super) fn semantic(&self) -> Option<&CompileUnitAnalysis> {
        self.analysis.as_ref()
    }

    pub(super) fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub(super) fn depends_on(&self, path: &Path) -> bool {
        self.source_paths.contains(path)
    }
}

#[cfg(test)]
pub(super) fn workspace_analysis_for_uri(
    uri: &str,
    documents: &HashMap<String, OpenDocument>,
) -> Option<LspWorkspaceAnalysis> {
    workspace_analysis_for_uri_with_package_root(uri, documents, None)
}

#[cfg(test)]
pub(super) fn workspace_analysis_for_uri_with_package_root(
    uri: &str,
    documents: &HashMap<String, OpenDocument>,
    package_root: Option<&Path>,
) -> Option<LspWorkspaceAnalysis> {
    let package_graph = package_root.and_then(locked_offline_package_graph);
    workspace_analysis_for_uri_with_package_graph(uri, documents, package_graph.as_ref())
}

pub(super) fn workspace_analysis_for_uri_with_package_graph(
    uri: &str,
    documents: &HashMap<String, OpenDocument>,
    package_graph: Option<&PackageGraph>,
) -> Option<LspWorkspaceAnalysis> {
    let mut workspace = OpenWorkspaceSources::new(documents);

    let root = workspace.source_for_uri(uri)?;
    let options = lsp_frontend_options(package_graph);
    let (analysis, diagnostics) = match load_compile_unit(&mut workspace.sources, root, &options) {
        Ok(unit) => {
            let analysis = analyze_module_compile_unit(&workspace.sources, &unit);
            let diagnostics = analysis.diagnostics();
            (Some(analysis), diagnostics)
        }
        Err(diagnostics) => (None, diagnostics),
    };
    let active_sources = analysis
        .as_ref()
        .map(|analysis| {
            analysis
                .files
                .iter()
                .map(|file| file.ast.span.source)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_else(|| HashSet::from([root]));
    let source_paths = workspace
        .sources
        .sources_with_absolute_paths()
        .filter(|(_, source)| active_sources.contains(source))
        .map(|(path, _)| path.to_path_buf())
        .collect();

    Some(LspWorkspaceAnalysis {
        sources: workspace.sources,
        analysis,
        diagnostics,
        source_paths,
    })
}

#[cfg(test)]
pub(super) fn diagnostics_for_workspace(
    root_uri: &str,
    documents: &HashMap<String, OpenDocument>,
) -> Vec<(String, Vec<LspDiagnostic>)> {
    diagnostics_for_workspace_with_package_root(root_uri, documents, None)
}

#[cfg(test)]
pub(super) fn diagnostics_for_workspace_with_package_root(
    root_uri: &str,
    documents: &HashMap<String, OpenDocument>,
    package_root: Option<&Path>,
) -> Vec<(String, Vec<LspDiagnostic>)> {
    let mut open_documents = documents.values().collect::<Vec<_>>();
    open_documents.sort_by(|left, right| left.uri.cmp(&right.uri));
    let mut workspace = OpenWorkspaceSources::new(documents);

    let diagnostics = match workspace.source_for_uri(root_uri) {
        Some(root) => {
            let package_graph = package_root.and_then(locked_offline_package_graph);
            let options = lsp_frontend_options(package_graph.as_ref());
            match load_compile_unit(&mut workspace.sources, root, &options) {
                Ok(unit) => analyze_module_compile_unit(&workspace.sources, &unit).diagnostics(),
                Err(diagnostics) => diagnostics,
            }
        }
        None => Vec::new(),
    };

    open_documents
        .iter()
        .map(|document| {
            (
                document.uri.clone(),
                diagnostics_for_lsp(document, &open_documents, &workspace.sources, &diagnostics),
            )
        })
        .collect()
}

struct OpenWorkspaceSources {
    sources: SourceMap,
    source_by_uri: HashMap<String, SourceId>,
}

impl OpenWorkspaceSources {
    fn new(documents: &HashMap<String, OpenDocument>) -> Self {
        let mut open_documents = documents.values().collect::<Vec<_>>();
        open_documents.sort_by(|left, right| left.uri.cmp(&right.uri));

        let mut sources = SourceMap::new();
        let mut source_by_uri = HashMap::new();
        for document in &open_documents {
            let source = sources.add_source(
                document.display_path.clone(),
                document.absolute_path.clone(),
                document.text.clone(),
            );
            source_by_uri.insert(document.uri.clone(), source);
        }

        Self {
            sources,
            source_by_uri,
        }
    }

    fn source_for_uri(&self, uri: &str) -> Option<SourceId> {
        self.source_by_uri.get(uri).copied()
    }
}

fn lsp_frontend_options(package_graph: Option<&PackageGraph>) -> FrontendOptions {
    FrontendOptions {
        package_graph: package_graph.cloned(),
        ..FrontendOptions::default()
    }
}

#[cfg(test)]
fn locked_offline_package_graph(root: &Path) -> Option<PackageGraph> {
    crate::package::load_package_graph(
        root,
        crate::package::PackageGraphOptions {
            locked: true,
            offline: true,
        },
    )
    .graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_frontend_options_do_not_override_nocter_home_from_document_path() {
        assert!(lsp_frontend_options(None).nocter_home.is_none());
    }
}
