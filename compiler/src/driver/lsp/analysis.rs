use super::diagnostics::{LspDiagnostic, diagnostics_for_lsp};
use super::documents::OpenDocument;
use crate::analysis::{CompileUnitAnalysis, FileAnalysis, analyze_compile_unit_with_entry};
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::source::{SourceId, SourceMap};
use std::collections::HashMap;

pub(super) struct LspWorkspaceAnalysis {
    pub(super) sources: SourceMap,
    pub(super) analysis: CompileUnitAnalysis,
    root: SourceId,
}

impl LspWorkspaceAnalysis {
    pub(super) fn root_file(&self) -> Option<&FileAnalysis> {
        self.analysis
            .files
            .iter()
            .find(|file| file.ast.span.source == self.root)
    }
}

pub(super) fn workspace_analysis_for_uri(
    uri: &str,
    documents: &HashMap<String, OpenDocument>,
) -> Option<LspWorkspaceAnalysis> {
    let document = documents.get(uri)?;
    let open_documents = sorted_open_documents(documents);

    let mut sources = SourceMap::new();
    let source_by_uri = source_map_for_documents(&mut sources, &open_documents);
    let root = source_by_uri.get(uri).copied()?;
    let options = frontend_options_for_document(document);
    let unit = load_compile_unit(&mut sources, root, &options).ok()?;
    let analysis = analyze_compile_unit_with_entry(&sources, &unit, DEFAULT_ENTRY_NAME);

    Some(LspWorkspaceAnalysis {
        sources,
        analysis,
        root,
    })
}

pub(super) fn diagnostics_for_workspace(
    root_uri: &str,
    documents: &HashMap<String, OpenDocument>,
) -> Vec<(String, Vec<LspDiagnostic>)> {
    let open_documents = sorted_open_documents(documents);

    let mut sources = SourceMap::new();
    let source_by_uri = source_map_for_documents(&mut sources, &open_documents);

    let diagnostics = match source_by_uri.get(root_uri).copied() {
        Some(root) => match documents
            .get(root_uri)
            .map(frontend_options_for_document)
            .map(|options| load_compile_unit(&mut sources, root, &options))
            .unwrap_or_else(|| load_compile_unit(&mut sources, root, &FrontendOptions::default()))
        {
            Ok(unit) => {
                analyze_compile_unit_with_entry(&sources, &unit, DEFAULT_ENTRY_NAME).diagnostics()
            }
            Err(diagnostics) => diagnostics,
        },
        None => Vec::new(),
    };

    open_documents
        .into_iter()
        .map(|document| {
            (
                document.uri.clone(),
                diagnostics_for_lsp(document, diagnostics.clone()),
            )
        })
        .collect()
}

fn sorted_open_documents(documents: &HashMap<String, OpenDocument>) -> Vec<&OpenDocument> {
    let mut open_documents = documents.values().collect::<Vec<_>>();
    open_documents.sort_by(|left, right| left.uri.cmp(&right.uri));
    open_documents
}

fn source_map_for_documents(
    sources: &mut SourceMap,
    documents: &[&OpenDocument],
) -> HashMap<String, SourceId> {
    let mut source_by_uri = HashMap::new();

    for document in documents {
        let source = sources.add_source(
            document.display_path.clone(),
            document.absolute_path.clone(),
            document.text.clone(),
        );
        source_by_uri.insert(document.uri.clone(), source);
    }

    source_by_uri
}

fn frontend_options_for_document(_document: &OpenDocument) -> FrontendOptions {
    FrontendOptions::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_options_do_not_override_nocter_home_from_document_path() {
        let document = OpenDocument {
            uri: "file:///tmp/project/app.nct".to_string(),
            version: Some(1),
            display_path: "/tmp/project/app.nct".to_string(),
            absolute_path: Some(std::path::PathBuf::from("/tmp/project/app.nct")),
            text: "func main(): i32 { return 0 }".to_string(),
        };

        assert!(
            frontend_options_for_document(&document)
                .nocter_home
                .is_none()
        );
    }
}
