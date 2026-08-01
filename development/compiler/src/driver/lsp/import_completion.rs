//! Import-path context detection and filesystem-backed module candidates.

use super::completion::completion_values;
use super::documents::{OpenDocument, WorkspaceRoot};
use crate::analysis::completion::{CompletionItemInfo, CompletionItemKind};
use crate::frontend::module_segment_candidates;
use crate::home::resolve_nocter_home;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(super) fn module_completion_items(
    document: &OpenDocument,
    workspace_roots: &[WorkspaceRoot],
    offset: usize,
) -> Option<Vec<Value>> {
    let typed_path = import_path_at_offset(&document.text, offset)?;
    let source_dir = document.absolute_path.as_deref()?.parent()?;
    let (parent, prefix) = typed_path.rsplit_once('/').unwrap_or(("", typed_path));
    let directories = search_directories(source_dir, workspace_roots, parent, typed_path);
    let candidates = directories
        .iter()
        .flat_map(|directory| module_segment_candidates(directory, prefix))
        .collect::<BTreeSet<_>>();
    Some(completion_values(
        candidates
            .into_iter()
            .map(|label| CompletionItemInfo {
                detail: Some("module path segment".to_string()),
                insert_text: Some(label.clone()),
                sort_text: Some(format!("0-{label}")),
                label,
                kind: CompletionItemKind::Module,
                documentation: None,
                declaration_span: None,
            })
            .collect(),
    ))
}

pub(super) fn source_root_for_document<'a>(
    document: &OpenDocument,
    workspace_roots: &'a [WorkspaceRoot],
) -> Option<&'a Path> {
    let document_path = document.absolute_path.as_deref()?;
    workspace_roots
        .iter()
        .filter_map(|root| root.path.as_deref())
        .filter(|root| document_path.starts_with(root))
        .max_by_key(|root| root.components().count())
}

fn search_directories(
    source_dir: &Path,
    workspace_roots: &[WorkspaceRoot],
    parent: &str,
    typed_path: &str,
) -> Vec<PathBuf> {
    if typed_path.starts_with('/') {
        return vec![PathBuf::from("/").join(parent.trim_start_matches('/'))];
    }
    if typed_path.starts_with("./") || typed_path.starts_with("../") {
        return vec![source_dir.join(parent)];
    }

    let mut roots = workspace_roots
        .iter()
        .filter_map(|root| root.path.clone())
        .collect::<Vec<_>>();
    if roots.is_empty() {
        roots.push(source_dir.to_path_buf());
    }
    if let Ok(home) = resolve_nocter_home() {
        roots.push(home);
    }
    roots.into_iter().map(|root| root.join(parent)).collect()
}

fn import_path_at_offset(text: &str, offset: usize) -> Option<&str> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let prefix = text[line_start..offset].trim_start();
    let body = prefix
        .strip_prefix("use ")
        .or_else(|| prefix.strip_prefix("pub use "))
        .or_else(|| prefix.strip_prefix("nocter use "))?;
    if body.is_empty() || body.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Some(body);
    }
    if !body
        .bytes()
        .all(|byte| byte == b'/' || byte == b'.' || byte == b'_' || byte.is_ascii_alphanumeric())
    {
        return None;
    }
    if let Some(dot) = body.rfind('.')
        && dot != 0
        && body.as_bytes().get(dot - 1) != Some(&b'.')
        && body.as_bytes().get(dot + 1) != Some(&b'/')
    {
        return None;
    }
    Some(body)
}

#[cfg(test)]
mod tests {
    use super::import_path_at_offset;

    #[test]
    fn distinguishes_module_paths_from_symbol_selectors() {
        assert_eq!(import_path_at_offset("use std/ve", 10), Some("std/ve"));
        assert_eq!(import_path_at_offset("use ../li", 9), Some("../li"));
        assert_eq!(import_path_at_offset("use std/vec.", 12), None);
    }
}
