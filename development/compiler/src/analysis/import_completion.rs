//! Public symbol candidates for a resolved `use module.` selector.

use super::completion::{CompletionItemInfo, completion_item_for_symbol};
use super::{CompileUnitAnalysis, FileAnalysis};
use std::collections::HashSet;

pub(super) fn import_symbol_items_at_offset(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Vec<CompletionItemInfo>> {
    let import = file.syntax.from_import_selector_at(offset)?;
    let import_source = analysis.import_sources.get(&import.path.span)?;
    let imported = analysis.file_by_source(import_source.source)?;
    let visible = imported
        .syntax
        .visible_export_anchors(import_source.access)
        .collect::<HashSet<_>>();

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
