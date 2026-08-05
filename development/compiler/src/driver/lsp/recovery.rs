//! Temporary open-document overlays used only for editor recovery analysis.

use super::analysis::{LspWorkspaceAnalysis, workspace_analysis_for_uri_with_package_root};
use super::documents::OpenDocument;
use std::collections::HashMap;
use std::path::Path;

pub(super) fn workspace_analysis_with_recovered_document(
    uri: &str,
    documents: &HashMap<String, OpenDocument>,
    recovered_text: String,
    package_root: Option<&Path>,
) -> Option<LspWorkspaceAnalysis> {
    let mut recovered_documents = documents.clone();
    recovered_documents.get_mut(uri)?.text = recovered_text;
    workspace_analysis_for_uri_with_package_root(uri, &recovered_documents, package_root)
}
