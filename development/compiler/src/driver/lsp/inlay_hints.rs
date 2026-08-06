use super::documents::OpenDocument;
use super::protocol::{byte_offset_to_lsp_position, lsp_position_to_byte_offset};
use crate::analysis::inlay_hints::{InlayHintKind, inlay_hints_for_file_analysis};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::source::SourceMap;
use serde_json::{Value, json};

pub(super) fn inlay_hints(
    document: &OpenDocument,
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    params: Option<&Value>,
) -> Vec<Value> {
    let Some(range) = params.and_then(|params| params.get("range")) else {
        return Vec::new();
    };
    let Some(start) = range.get("start") else {
        return Vec::new();
    };
    let Some(end) = range.get("end") else {
        return Vec::new();
    };
    let Some((start_line, start_character, end_line, end_character)) = start
        .get("line")
        .and_then(Value::as_u64)
        .zip(start.get("character").and_then(Value::as_u64))
        .zip(end.get("line").and_then(Value::as_u64))
        .zip(end.get("character").and_then(Value::as_u64))
        .map(
            |(((start_line, start_character), end_line), end_character)| {
                (start_line, start_character, end_line, end_character)
            },
        )
    else {
        return Vec::new();
    };
    let start = lsp_position_to_byte_offset(
        &document.text,
        start_line as usize,
        start_character as usize,
    );
    let end =
        lsp_position_to_byte_offset(&document.text, end_line as usize, end_character as usize);
    inlay_hints_for_file_analysis(sources, analysis, file, start..=end)
        .into_iter()
        .map(|hint| {
            let mut value = json!({
                "position": byte_offset_to_lsp_position(&document.text, hint.offset),
                "label": hint.label,
                "paddingLeft": false,
                "paddingRight": false
            });
            if hint.kind == InlayHintKind::Type {
                value["kind"] = json!(1);
            }
            if let Some(tooltip) = hint.tooltip {
                value["tooltip"] = json!({ "kind": "markdown", "value": tooltip });
            }
            value
        })
        .collect()
}
