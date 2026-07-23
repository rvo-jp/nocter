use super::diagnostics::{LspDiagnostic, diagnostics_for_lsp};
use super::documents::OpenDocument;
use crate::analysis::{CompileUnitAnalysis, FileAnalysis, analyze_module_compile_unit};
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::source::{SourceId, SourceMap};
use std::collections::HashMap;

pub(super) struct LspWorkspaceAnalysis {
    pub(super) sources: SourceMap,
    pub(super) analysis: CompileUnitAnalysis,
}

impl LspWorkspaceAnalysis {
    pub(super) fn root_file(&self) -> Option<&FileAnalysis> {
        self.analysis.root_file()
    }
}

pub(super) fn workspace_analysis_for_uri(
    uri: &str,
    documents: &HashMap<String, OpenDocument>,
) -> Option<LspWorkspaceAnalysis> {
    let mut workspace = OpenWorkspaceSources::new(documents);

    let root = workspace.source_for_uri(uri)?;
    let options = lsp_frontend_options();
    let unit = load_compile_unit(&mut workspace.sources, root, &options).ok()?;
    let analysis = analyze_module_compile_unit(&workspace.sources, &unit);

    Some(LspWorkspaceAnalysis {
        sources: workspace.sources,
        analysis,
    })
}

pub(super) fn diagnostics_for_workspace(
    root_uri: &str,
    documents: &HashMap<String, OpenDocument>,
) -> Vec<(String, Vec<LspDiagnostic>)> {
    let mut workspace = OpenWorkspaceSources::new(documents);

    let diagnostics = match workspace.source_for_uri(root_uri) {
        Some(root) => {
            let options = lsp_frontend_options();
            match load_compile_unit(&mut workspace.sources, root, &options) {
                Ok(unit) => analyze_module_compile_unit(&workspace.sources, &unit).diagnostics(),
                Err(diagnostics) => diagnostics,
            }
        }
        None => Vec::new(),
    };

    workspace
        .open_documents
        .iter()
        .map(|document| {
            (
                document.uri.clone(),
                diagnostics_for_lsp(
                    document,
                    &workspace.open_documents,
                    &workspace.sources,
                    diagnostics.clone(),
                ),
            )
        })
        .collect()
}

struct OpenWorkspaceSources<'a> {
    open_documents: Vec<&'a OpenDocument>,
    sources: SourceMap,
    source_by_uri: HashMap<String, SourceId>,
}

impl<'a> OpenWorkspaceSources<'a> {
    fn new(documents: &'a HashMap<String, OpenDocument>) -> Self {
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
            open_documents,
            sources,
            source_by_uri,
        }
    }

    fn source_for_uri(&self, uri: &str) -> Option<SourceId> {
        self.source_by_uri.get(uri).copied()
    }
}

fn lsp_frontend_options() -> FrontendOptions {
    FrontendOptions::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_frontend_options_do_not_override_nocter_home_from_document_path() {
        assert!(lsp_frontend_options().nocter_home.is_none());
    }
}
