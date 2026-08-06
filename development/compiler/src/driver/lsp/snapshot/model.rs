use super::super::analysis::LspWorkspaceAnalysis;
use super::super::diagnostics::LspDiagnostic;
use super::super::documents::{OpenDocument, WorkspaceRoot};
use crate::analysis::package_index::PackageSemanticIndex;
use crate::diagnostics::Diagnostic;
use crate::package::PackageGraph;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(in crate::driver::lsp) struct LspSnapshot {
    generation: u64,
    workspace_roots: Vec<WorkspaceRoot>,
    documents: HashMap<String, OpenDocument>,
    document_analyses: HashMap<String, DocumentSnapshot>,
    packages: HashMap<PathBuf, PackageSnapshot>,
    package_indexes: HashMap<PathBuf, Arc<PackageSemanticIndex>>,
    diagnostics: HashMap<String, Vec<LspDiagnostic>>,
}

impl LspSnapshot {
    pub(in crate::driver::lsp) fn generation(&self) -> u64 {
        self.generation
    }

    pub(in crate::driver::lsp) fn matches_inputs(
        &self,
        documents: &HashMap<String, OpenDocument>,
        workspace_roots: &[WorkspaceRoot],
    ) -> bool {
        self.documents == *documents && self.workspace_roots == workspace_roots
    }

    pub(in crate::driver::lsp) fn document(&self, uri: &str) -> Option<&OpenDocument> {
        self.documents.get(uri)
    }

    pub(in crate::driver::lsp) fn documents(&self) -> &HashMap<String, OpenDocument> {
        &self.documents
    }

    pub(in crate::driver::lsp) fn analysis(&self, uri: &str) -> Option<&LspWorkspaceAnalysis> {
        self.document_analyses
            .get(uri)
            .map(|document| document.analysis.as_ref())
    }

    pub(in crate::driver::lsp) fn package_root(&self, uri: &str) -> Option<&Path> {
        self.document_analyses.get(uri)?.package_root.as_deref()
    }

    pub(in crate::driver::lsp) fn package_graph(&self, uri: &str) -> Option<&PackageGraph> {
        let root = self.package_root(uri)?;
        self.packages.get(root)?.graph.as_deref()
    }

    pub(in crate::driver::lsp) fn package_index(&self, uri: &str) -> Option<&PackageSemanticIndex> {
        let root = self.package_root(uri)?;
        let index = self.package_indexes.get(root).map(AsRef::as_ref)?;
        debug_assert_eq!(index.generation(), self.generation);
        Some(index)
    }

    pub(in crate::driver::lsp) fn document_uris(&self) -> Vec<&str> {
        let mut uris = self
            .documents
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        uris.sort_unstable();
        uris
    }

    pub(in crate::driver::lsp) fn diagnostics_for_uri(&self, uri: &str) -> &[LspDiagnostic] {
        self.diagnostics.get(uri).map(Vec::as_slice).unwrap_or(&[])
    }

    pub(in crate::driver::lsp) fn package(&self, root: &Path) -> Option<&PackageSnapshot> {
        self.packages.get(root)
    }

    pub(in crate::driver::lsp) fn document_snapshot(&self, uri: &str) -> Option<&DocumentSnapshot> {
        self.document_analyses.get(uri)
    }

    pub(in crate::driver::lsp) fn new(
        generation: u64,
        workspace_roots: Vec<WorkspaceRoot>,
        documents: HashMap<String, OpenDocument>,
        document_analyses: HashMap<String, DocumentSnapshot>,
        packages: HashMap<PathBuf, PackageSnapshot>,
        package_indexes: HashMap<PathBuf, Arc<PackageSemanticIndex>>,
        diagnostics: HashMap<String, Vec<LspDiagnostic>>,
    ) -> Self {
        Self {
            generation,
            workspace_roots,
            documents,
            document_analyses,
            packages,
            package_indexes,
            diagnostics,
        }
    }
}

pub(in crate::driver::lsp) struct DocumentSnapshot {
    pub(in crate::driver::lsp) analysis: Arc<LspWorkspaceAnalysis>,
    pub(in crate::driver::lsp) package_root: Option<PathBuf>,
    pub(in crate::driver::lsp) package_revision: Option<u64>,
}

#[derive(Clone)]
pub(in crate::driver::lsp) struct PackageSnapshot {
    pub(in crate::driver::lsp) graph: Option<Arc<PackageGraph>>,
    pub(in crate::driver::lsp) diagnostics: Vec<Diagnostic>,
    pub(in crate::driver::lsp) package_files: HashSet<PathBuf>,
    pub(in crate::driver::lsp) revision: u64,
}
