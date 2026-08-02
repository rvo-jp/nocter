use super::documents::OpenDocument;
use crate::analysis::completion::{
    CompletionItemInfo, CompletionItemKind,
    completion_items_for_compile_unit_at_offset as analysis_completion_items_for_compile_unit,
    completion_items_for_text_at_offset,
    keyword_completion_items as analysis_keyword_completion_items,
    literal_shape_completion_items_for_file_analysis_at_offset as analysis_literal_shape_completion_items,
};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::source::SourceMap;
use serde_json::{Value, json};

pub(super) const LSP_COMPLETION_ITEM_KIND_METHOD: u8 = 2;
pub(super) const LSP_COMPLETION_ITEM_KIND_FUNCTION: u8 = 3;
pub(super) const LSP_COMPLETION_ITEM_KIND_CONSTRUCTOR: u8 = 4;
pub(super) const LSP_COMPLETION_ITEM_KIND_FIELD: u8 = 5;
const LSP_COMPLETION_ITEM_KIND_VARIABLE: u8 = 6;
const LSP_COMPLETION_ITEM_KIND_CLASS: u8 = 7;
const LSP_COMPLETION_ITEM_KIND_INTERFACE: u8 = 8;
pub(super) const LSP_COMPLETION_ITEM_KIND_MODULE: u8 = 9;
const LSP_COMPLETION_ITEM_KIND_ENUM: u8 = 13;
const LSP_COMPLETION_ITEM_KIND_KEYWORD: u8 = 14;
pub(super) const LSP_COMPLETION_ITEM_KIND_ENUM_MEMBER: u8 = 20;
pub(super) const LSP_COMPLETION_ITEM_KIND_STRUCT: u8 = 22;

pub(super) fn completion_items_for_file_analysis_at_offset(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Vec<Value> {
    completion_values(analysis_completion_items_for_compile_unit(
        sources, analysis, file, offset,
    ))
}

pub(super) fn literal_shape_completion_items_for_file_analysis_at_offset(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Vec<Value>> {
    analysis_literal_shape_completion_items(file, offset)?;
    Some(completion_items_for_file_analysis_at_offset(
        sources, analysis, file, offset,
    ))
}

pub(super) fn completion_items_for_document_at_offset(
    document: &OpenDocument,
    offset: usize,
) -> Option<Vec<Value>> {
    completion_items_for_text_at_offset(&document.text, offset).map(completion_values)
}

pub(super) fn keyword_completion_items() -> Vec<Value> {
    completion_values(analysis_keyword_completion_items())
}

pub(super) fn completion_values(items: Vec<CompletionItemInfo>) -> Vec<Value> {
    items.iter().map(completion_item).collect()
}

fn completion_item(item: &CompletionItemInfo) -> Value {
    let mut value = json!({
        "label": item.label,
        "kind": lsp_completion_kind(item.kind),
    });

    if let Some(detail) = &item.detail
        && let Some(object) = value.as_object_mut()
    {
        object.insert("detail".to_string(), Value::String(detail.clone()));
    }

    if let Some(documentation) = &item.documentation
        && let Some(object) = value.as_object_mut()
    {
        object.insert(
            "documentation".to_string(),
            json!({ "kind": "markdown", "value": documentation }),
        );
    }

    if let Some(insert_text) = &item.insert_text
        && let Some(object) = value.as_object_mut()
    {
        object.insert("insertText".to_string(), Value::String(insert_text.clone()));
    }

    if let Some(sort_text) = &item.sort_text
        && let Some(object) = value.as_object_mut()
    {
        object.insert("sortText".to_string(), Value::String(sort_text.clone()));
    }

    value
}

const fn lsp_completion_kind(kind: CompletionItemKind) -> u8 {
    match kind {
        CompletionItemKind::Constructor => LSP_COMPLETION_ITEM_KIND_CONSTRUCTOR,
        CompletionItemKind::Function => LSP_COMPLETION_ITEM_KIND_FUNCTION,
        CompletionItemKind::Method => LSP_COMPLETION_ITEM_KIND_METHOD,
        CompletionItemKind::Class => LSP_COMPLETION_ITEM_KIND_CLASS,
        CompletionItemKind::Interface => LSP_COMPLETION_ITEM_KIND_INTERFACE,
        CompletionItemKind::Module => LSP_COMPLETION_ITEM_KIND_MODULE,
        CompletionItemKind::Enum => LSP_COMPLETION_ITEM_KIND_ENUM,
        CompletionItemKind::EnumMember => LSP_COMPLETION_ITEM_KIND_ENUM_MEMBER,
        CompletionItemKind::Field => LSP_COMPLETION_ITEM_KIND_FIELD,
        CompletionItemKind::Keyword => LSP_COMPLETION_ITEM_KIND_KEYWORD,
        CompletionItemKind::Struct => LSP_COMPLETION_ITEM_KIND_STRUCT,
        CompletionItemKind::Variable => LSP_COMPLETION_ITEM_KIND_VARIABLE,
    }
}
