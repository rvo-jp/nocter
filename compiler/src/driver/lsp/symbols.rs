use super::documents::OpenDocument;
use super::protocol::range_for_byte_span;
use crate::analysis::symbols::{DocumentSymbolInfo, DocumentSymbolKind, document_symbols_for_text};
use serde_json::{Value, json};

const LSP_SYMBOL_KIND_CLASS: u8 = 5;
const LSP_SYMBOL_KIND_METHOD: u8 = 6;
pub(super) const LSP_SYMBOL_KIND_FIELD: u8 = 8;
const LSP_SYMBOL_KIND_ENUM: u8 = 10;
const LSP_SYMBOL_KIND_INTERFACE: u8 = 11;
pub(super) const LSP_SYMBOL_KIND_FUNCTION: u8 = 12;
pub(super) const LSP_SYMBOL_KIND_ENUM_MEMBER: u8 = 22;
pub(super) const LSP_SYMBOL_KIND_STRUCT: u8 = 23;

pub(super) fn document_symbols_for_document(document: &OpenDocument) -> Option<Vec<Value>> {
    document_symbols_for_text(&document.text).map(|symbols| {
        symbols
            .iter()
            .map(|symbol| document_symbol_value(&document.text, symbol))
            .collect()
    })
}

fn document_symbol_value(text: &str, symbol: &DocumentSymbolInfo) -> Value {
    let children = symbol
        .children
        .iter()
        .map(|child| document_symbol_value(text, child))
        .collect::<Vec<_>>();
    let mut value = json!({
        "name": symbol.name,
        "kind": lsp_symbol_kind(symbol.kind),
        "range": range_for_byte_span(text, symbol.range_span),
        "selectionRange": range_for_byte_span(text, symbol.selection_span)
    });

    if !children.is_empty()
        && let Some(object) = value.as_object_mut()
    {
        object.insert("children".to_string(), Value::Array(children));
    }

    value
}

const fn lsp_symbol_kind(kind: DocumentSymbolKind) -> u8 {
    match kind {
        DocumentSymbolKind::Class => LSP_SYMBOL_KIND_CLASS,
        DocumentSymbolKind::Method => LSP_SYMBOL_KIND_METHOD,
        DocumentSymbolKind::Field => LSP_SYMBOL_KIND_FIELD,
        DocumentSymbolKind::Enum => LSP_SYMBOL_KIND_ENUM,
        DocumentSymbolKind::Interface => LSP_SYMBOL_KIND_INTERFACE,
        DocumentSymbolKind::Function => LSP_SYMBOL_KIND_FUNCTION,
        DocumentSymbolKind::EnumMember => LSP_SYMBOL_KIND_ENUM_MEMBER,
        DocumentSymbolKind::Struct => LSP_SYMBOL_KIND_STRUCT,
    }
}
