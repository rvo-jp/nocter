use super::documents::OpenDocument;
use super::protocol::{
    LspRange, byte_offset_to_lsp_position, lsp_position_to_byte_offset, position_from_params,
    range_for_byte_span,
};
use super::semantic::{SEMANTIC_DECLARATION_MODIFIER, classified_identifiers};
use crate::analysis::hover::{
    HoverInfo, hover_for_file_analysis as analysis_hover_for_file_analysis, hover_for_text,
};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::source::SourceMap;
use serde_json::{Value, json};

pub(super) fn hover_for_document(document: &OpenDocument, params: Option<&Value>) -> Option<Value> {
    let position = position_from_params(params)?;
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
    if let Some(hover) = documented_hover_for_document(document, offset) {
        return Some(hover);
    }

    let identifier = classified_identifiers(document)
        .into_iter()
        .find(|identifier| identifier.start_byte <= offset && offset < identifier.end_byte)?;
    let lexeme = document
        .text
        .get(identifier.start_byte..identifier.end_byte)?;
    let range = LspRange {
        start: byte_offset_to_lsp_position(&document.text, identifier.start_byte),
        end: byte_offset_to_lsp_position(&document.text, identifier.end_byte),
    };
    let declaration = if identifier.modifiers & SEMANTIC_DECLARATION_MODIFIER != 0 {
        " declaration"
    } else {
        ""
    };

    Some(json!({
        "contents": {
            "kind": "markdown",
            "value": format!("```nocter\n{}{} {}\n```", identifier.kind.hover_label(), declaration, lexeme)
        },
        "range": range
    }))
}

pub(super) fn hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Value> {
    let text = sources.get(file.ast.span.source)?.text();
    analysis_hover_for_file_analysis(sources, analysis, file, offset)
        .map(|hover| hover_value(text, &hover))
}

fn documented_hover_for_document(document: &OpenDocument, offset: usize) -> Option<Value> {
    hover_for_text(&document.text, offset).map(|hover| hover_value(&document.text, &hover))
}

fn hover_value(text: &str, hover: &HoverInfo) -> Value {
    json!({
        "contents": {
            "kind": "markdown",
            "value": hover_markdown(&hover.label, hover.documentation.as_deref())
        },
        "range": range_for_byte_span(text, hover.span)
    })
}

fn hover_markdown(label: &str, documentation: Option<&str>) -> String {
    let mut value = format!("```nocter\n{label}\n```");
    if let Some(documentation) = documentation
        && !documentation.trim().is_empty()
    {
        value.push_str("\n\n");
        value.push_str(documentation.trim());
    }
    value
}
