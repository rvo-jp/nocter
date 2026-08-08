use super::super::analysis::{
    workspace_analysis_for_path_with_package_graph, workspace_analysis_for_uri_with_package_graph,
};
use super::super::diagnostics::diagnostics_for_lsp;
use super::super::documents::{OpenDocument, WorkspaceRoot};
use super::super::import_completion::package_root_for_document;
use super::invalidation::{SnapshotChange, can_reuse_document, can_reuse_package};
use super::model::{DocumentSnapshot, LspSnapshot, PackageSnapshot};
use crate::analysis::package_index::PackageSemanticIndexBuilder;
use crate::package::{PackageSourceOverlay, load_locked_offline_package_graph_with_overlay};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(in crate::driver::lsp) fn build_snapshot(
    generation: u64,
    documents: &HashMap<String, OpenDocument>,
    workspace_roots: &[WorkspaceRoot],
    previous: Option<&LspSnapshot>,
    change: &SnapshotChange,
) -> LspSnapshot {
    let documents = documents.clone();
    let package_overlay = package_source_overlay(&documents);
    let package_roots = documents
        .values()
        .filter_map(|document| package_root_for_document(document, workspace_roots))
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>();
    let packages = package_roots
        .into_iter()
        .map(|root| {
            let package = previous
                .and_then(|snapshot| snapshot.package(&root))
                .filter(|package| can_reuse_package(package, change))
                .cloned()
                .unwrap_or_else(|| load_package_snapshot(&root, generation, &package_overlay));
            (root, package)
        })
        .collect::<HashMap<_, _>>();

    let mut analyses = HashMap::new();
    for (uri, document) in &documents {
        let package_root =
            package_root_for_document(document, workspace_roots).map(Path::to_path_buf);
        let package = package_root.as_deref().and_then(|root| packages.get(root));
        let package_revision = package.map(|package| package.revision);
        if let Some(previous_document_snapshot) = previous
            .and_then(|snapshot| Some((snapshot.document(uri)?, snapshot.document_snapshot(uri)?)))
            .filter(|(previous_document, previous_snapshot)| {
                can_reuse_document(
                    previous_snapshot,
                    previous_document,
                    document,
                    package_root.as_deref(),
                    package_revision,
                    change,
                )
            })
            .map(|(_, snapshot)| snapshot)
        {
            analyses.insert(
                uri.clone(),
                DocumentSnapshot {
                    analysis: Arc::clone(&previous_document_snapshot.analysis),
                    package_root,
                    package_revision,
                },
            );
            continue;
        }

        let analysis_root = module_analysis_root(document, package_root.as_deref());
        let analysis = analysis_root
            .as_deref()
            .and_then(|root| {
                workspace_analysis_for_path_with_package_graph(
                    root,
                    &documents,
                    package.and_then(|package| package.graph.as_deref()),
                )
            })
            .filter(|analysis| analysis.file_for_document(document).is_some())
            .or_else(|| {
                workspace_analysis_for_uri_with_package_graph(
                    uri,
                    &documents,
                    package.and_then(|package| package.graph.as_deref()),
                )
            })
            .expect("snapshot document must have an analysis source");
        analyses.insert(
            uri.clone(),
            DocumentSnapshot {
                analysis: Arc::new(analysis),
                package_root,
                package_revision,
            },
        );
    }

    let open_documents = sorted_documents(&documents);
    let package_indexes = build_package_indexes(generation, &documents, &analyses, &packages);
    let diagnostics = analyses
        .iter()
        .map(|(uri, document_snapshot)| {
            let document = documents
                .get(uri)
                .expect("document analysis must have one immutable source document");
            let mut raw = document_snapshot.analysis.diagnostics().to_vec();
            if document_snapshot
                .package_root
                .as_deref()
                .is_some_and(|root| {
                    document.absolute_path.as_deref() == Some(&root.join("nocter.nct"))
                })
                && let Some(package) = document_snapshot
                    .package_root
                    .as_deref()
                    .and_then(|root| packages.get(root))
            {
                for diagnostic in &package.diagnostics {
                    if !raw.contains(diagnostic) {
                        raw.push(diagnostic.clone());
                    }
                }
            }
            let diagnostics = diagnostics_for_lsp(
                document,
                &open_documents,
                &document_snapshot.analysis.sources,
                &raw,
            );
            (uri.clone(), diagnostics)
        })
        .collect();

    LspSnapshot::new(
        generation,
        workspace_roots.to_vec(),
        documents,
        analyses,
        packages,
        package_indexes,
        diagnostics,
    )
}

fn module_analysis_root(document: &OpenDocument, package_root: Option<&Path>) -> Option<PathBuf> {
    let path = document.absolute_path.as_deref()?;
    if path.file_name().is_some_and(|name| name == "index.nct") {
        return Some(path.to_path_buf());
    }

    let mut directory = path.parent()?;
    loop {
        let index = directory.join("index.nct");
        if index.is_file() {
            return Some(index);
        }
        if package_root.is_none_or(|root| directory == root) {
            break;
        }
        let Some(parent) = directory.parent() else {
            break;
        };
        if package_root.is_some_and(|root| !parent.starts_with(root)) {
            break;
        }
        directory = parent;
    }
    None
}

fn build_package_indexes(
    generation: u64,
    documents: &HashMap<String, OpenDocument>,
    analyses: &HashMap<String, DocumentSnapshot>,
    packages: &HashMap<std::path::PathBuf, PackageSnapshot>,
) -> HashMap<std::path::PathBuf, Arc<crate::analysis::package_index::PackageSemanticIndex>> {
    packages
        .iter()
        .map(|(root, package)| {
            let graph = package.graph.as_deref();
            let mut builder = PackageSemanticIndexBuilder::new(generation, graph);

            for snapshot in analyses
                .values()
                .filter(|snapshot| snapshot.package_root.as_deref() == Some(root.as_path()))
            {
                if let Some(analysis) = snapshot.analysis.semantic() {
                    builder.add_analysis(&snapshot.analysis.sources, analysis);
                }
            }

            let roots = graph
                .into_iter()
                .flat_map(|graph| graph.packages())
                .flat_map(|package| {
                    std::iter::once(package.root_module().source_path())
                        .chain(
                            package
                                .executables()
                                .iter()
                                .map(|target| target.module().source_path()),
                        )
                        .chain(
                            package
                                .tests()
                                .iter()
                                .map(|target| target.module().source_path()),
                        )
                })
                .collect::<BTreeSet<_>>();
            for entry in roots {
                let Some(analysis) =
                    workspace_analysis_for_path_with_package_graph(entry, documents, graph)
                else {
                    continue;
                };
                if let Some(semantic) = analysis.semantic() {
                    builder.add_analysis(&analysis.sources, semantic);
                }
            }

            (root.clone(), Arc::new(builder.finish()))
        })
        .collect()
}

fn load_package_snapshot(
    root: &Path,
    generation: u64,
    overlay: &PackageSourceOverlay,
) -> PackageSnapshot {
    let load = load_locked_offline_package_graph_with_overlay(root, overlay);
    let package_files = load
        .package_files
        .into_iter()
        .flat_map(crate::frontend::dependency_path_aliases)
        .collect();
    PackageSnapshot {
        graph: load.graph.map(Arc::new),
        diagnostics: load.diagnostics,
        package_files,
        revision: generation,
    }
}

fn package_source_overlay(documents: &HashMap<String, OpenDocument>) -> PackageSourceOverlay {
    let mut overlay = PackageSourceOverlay::default();
    for document in documents.values() {
        let Some(path) = document.absolute_path.as_deref() else {
            continue;
        };
        if path.file_name().is_some_and(|name| name == "nocter.nct") {
            overlay.insert(path, document.text.clone());
        }
    }
    overlay
}

fn sorted_documents(documents: &HashMap<String, OpenDocument>) -> Vec<&OpenDocument> {
    let mut documents = documents.values().collect::<Vec<_>>();
    documents.sort_by(|left, right| left.uri.cmp(&right.uri));
    documents
}
