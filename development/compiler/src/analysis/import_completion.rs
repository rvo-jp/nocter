//! Public symbol candidates for a resolved `use module.` selector.

use super::completion::{CompletionItemInfo, completion_item_for_symbol};
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{AstFile, Item, Visibility};
use crate::resolve::ImportAccess;
use crate::source::ByteSpan;
use std::collections::HashSet;

pub(super) fn import_symbol_items_at_offset(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Vec<CompletionItemInfo>> {
    let path = file.ast.items.iter().find_map(|item| match item {
        Item::FromImport(import) if import.path.span.end < offset && offset <= import.span.end => {
            Some(import.path.span)
        }
        _ => None,
    })?;
    let import_source = analysis.import_sources.get(&path)?;
    let imported = analysis.file_by_source(import_source.source)?;
    let visible = visible_export_spans(&imported.ast, import_source.access);

    let mut items = imported
        .resolved
        .symbols
        .symbols()
        .filter(|symbol| !symbol.is_hidden && visible.contains(&symbol.name_span))
        .map(|symbol| completion_item_for_symbol(symbol, &imported.resolved))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.label.cmp(&right.label));
    Some(items)
}

fn visible_export_spans(ast: &AstFile, access: ImportAccess) -> HashSet<ByteSpan> {
    let visible = |visibility| {
        visibility == Visibility::Public
            || (visibility == Visibility::Nocter && access == ImportAccess::Nocter)
    };
    let mut spans = HashSet::new();
    for item in &ast.items {
        match item {
            Item::Function(item) if visible(item.visibility) => {
                spans.insert(item.name_span);
            }
            Item::Primitive(item) if visible(item.visibility) => {
                spans.insert(item.name_span);
            }
            Item::TypeAlias(item) if visible(item.visibility) => {
                spans.insert(item.name_span);
            }
            Item::Struct(item) if visible(item.visibility) => {
                spans.insert(item.name_span);
            }
            Item::Enum(item) if visible(item.visibility) => {
                spans.insert(item.name_span);
            }
            Item::Interface(item) if visible(item.visibility) => {
                spans.insert(item.name_span);
            }
            Item::FromImport(item) if visible(item.visibility) => {
                spans.extend(item.names.iter().map(|name| name.local_span()));
            }
            Item::Import(_)
            | Item::Impl(_)
            | Item::FromImport(_)
            | Item::Literal(_)
            | Item::Construct(_) => {}
            Item::Function(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_)
            | Item::Interface(_) => {}
        }
    }
    spans
}
