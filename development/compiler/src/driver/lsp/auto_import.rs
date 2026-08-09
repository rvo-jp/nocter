use super::completion::{LSP_COMPLETION_ITEM_KIND_FUNCTION, LSP_COMPLETION_ITEM_KIND_STRUCT};
use super::documents::OpenDocument;
use super::protocol::range_for_byte_span;
use crate::analysis::FileAnalysis;
use crate::analysis::package_index::{IndexedExportKind, PackageSemanticIndex};
use crate::analysis::source_edits::plan_top_level_import;
use crate::package::{PackageGraph, SourcePackage};
use crate::source::{ByteSpan, SourceId};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AutoImportCandidate {
    pub(super) name: String,
    pub(super) module_path: String,
    pub(super) kind: IndexedExportKind,
}

pub(super) struct AutoImportContext<'a> {
    pub(super) document: &'a OpenDocument,
    pub(super) file: &'a FileAnalysis,
    pub(super) index: &'a PackageSemanticIndex,
    pub(super) graph: &'a PackageGraph,
}

pub(super) fn auto_import_completion_items(
    context: &AutoImportContext<'_>,
    offset: usize,
    existing: &[Value],
) -> Vec<Value> {
    let prefix = identifier_prefix(&context.document.text, offset);
    if prefix.is_empty() {
        return Vec::new();
    }
    let existing = existing
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect::<HashSet<_>>();
    auto_import_candidates(context, Some(prefix))
        .into_iter()
        .filter(|candidate| !existing.contains(candidate.name.as_str()))
        .filter_map(|candidate| completion_item(context, candidate))
        .collect()
}

pub(super) fn auto_import_candidates(
    context: &AutoImportContext<'_>,
    exact_or_prefix: Option<&str>,
) -> Vec<AutoImportCandidate> {
    let Some(current_path) = context.document.absolute_path.as_deref() else {
        return Vec::new();
    };
    let Some(owner) = context.graph.package_containing(current_path) else {
        return Vec::new();
    };
    let Some(current_module) =
        crate::source_scopes::semantic_module_id(current_path, owner.root(), owner.id().clone())
    else {
        return Vec::new();
    };
    let mut candidates = context
        .index
        .exports()
        .iter()
        .filter(|export| export.absolute_path != current_path)
        .filter(|export| export.visibility.allows(&current_module))
        .filter(|export| exact_or_prefix.is_none_or(|prefix| export.name.starts_with(prefix)))
        .filter_map(|export| {
            Some(AutoImportCandidate {
                name: export.name.clone(),
                module_path: import_module_path(context.graph, owner, &export.absolute_path)?,
                kind: export.kind,
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        (&left.name, &left.module_path).cmp(&(&right.name, &right.module_path))
    });
    candidates.dedup();
    candidates
}

pub(super) fn import_edit_value(
    context: &AutoImportContext<'_>,
    candidate: &AutoImportCandidate,
) -> Option<Value> {
    let edit = plan_top_level_import(
        &context.document.text,
        &context.file.ast,
        &candidate.module_path,
        &candidate.name,
    )?;
    let span = ByteSpan::new(SourceId::new(0), edit.offset, edit.offset);
    Some(json!({
        "range": range_for_byte_span(&context.document.text, span),
        "newText": edit.new_text
    }))
}

fn completion_item(
    context: &AutoImportContext<'_>,
    candidate: AutoImportCandidate,
) -> Option<Value> {
    let edit = import_edit_value(context, &candidate)?;
    let kind = match candidate.kind {
        IndexedExportKind::Function => LSP_COMPLETION_ITEM_KIND_FUNCTION,
        IndexedExportKind::Type => LSP_COMPLETION_ITEM_KIND_STRUCT,
    };
    Some(json!({
        "label": candidate.name,
        "kind": kind,
        "detail": format!("auto import from {}", candidate.module_path),
        "sortText": format!("9_{}_{}", candidate.name, candidate.module_path),
        "additionalTextEdits": [edit]
    }))
}

fn import_module_path(
    graph: &PackageGraph,
    owner: &SourcePackage,
    target_path: &Path,
) -> Option<String> {
    if let Some(target) = graph.package_containing(target_path) {
        let module = module_path_inside_package(target, target_path)?;
        if target.id() == owner.id() {
            return (!module.is_empty()).then(|| format!("/{module}"));
        }
        let dependency = graph.dependency_name(owner.id(), target.id())?;
        return Some(if module.is_empty() {
            dependency.to_string()
        } else {
            format!("{dependency}/{module}")
        });
    }

    None
}

fn module_path_inside_package(package: &SourcePackage, source: &Path) -> Option<String> {
    if source == package.package_file_path() {
        return Some(String::new());
    }
    let relative = source.strip_prefix(package.root()).ok()?;
    normalized_module_path(relative)
}

fn normalized_module_path(relative: &Path) -> Option<String> {
    let path = crate::source_layout::logical_module_path(relative)?;
    let value = path
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()?
        .join("/");
    Some(value)
}

fn identifier_prefix(text: &str, offset: usize) -> &str {
    let offset = offset.min(text.len());
    let start = text[..offset]
        .rfind(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .map(|index| index + 1)
        .unwrap_or(0);
    &text[start..offset]
}
