//! Temporary open-document overlays used only for editor recovery analysis.

use super::analysis::{LspWorkspaceAnalysis, workspace_analysis_for_uri_with_package_graph};
use super::documents::OpenDocument;
use crate::package::PackageGraph;
use std::collections::HashMap;

pub(super) fn workspace_analysis_with_recovered_document(
    uri: &str,
    documents: &HashMap<String, OpenDocument>,
    recovered_text: String,
    package_graph: Option<&PackageGraph>,
) -> Option<LspWorkspaceAnalysis> {
    let mut recovered_documents = documents.clone();
    recovered_documents.get_mut(uri)?.text = recovered_text;
    workspace_analysis_for_uri_with_package_graph(uri, &recovered_documents, package_graph)
}
