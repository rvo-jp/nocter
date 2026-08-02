use super::documents::OpenDocument;
use super::protocol::{LspPosition, byte_offset_to_lsp_position};
use crate::analysis::FileAnalysis;
use crate::analysis::semantic as analysis_semantic;

#[cfg(test)]
pub(super) use analysis_semantic::SEMANTIC_DECLARATION_MODIFIER;
#[cfg(test)]
pub(super) use analysis_semantic::SEMANTIC_READONLY_MODIFIER;
pub(super) use analysis_semantic::{ClassifiedIdentifier, SemanticTokenKind};

pub(super) const SEMANTIC_TOKEN_TYPES: [&str; 7] = [
    "function",
    "method",
    "variable",
    "parameter",
    "type",
    "property",
    "namespace",
];
pub(super) const SEMANTIC_TOKEN_MODIFIERS: [&str; 2] = ["declaration", "readonly"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemanticToken {
    start: LspPosition,
    length: usize,
    kind: SemanticTokenKind,
    modifiers: u32,
}

pub(super) fn semantic_tokens_for_document(document: &OpenDocument) -> Vec<usize> {
    encode_classified_identifiers(document, classified_identifiers(document))
}

pub(super) fn semantic_tokens_for_file_analysis(
    document: &OpenDocument,
    file: &FileAnalysis,
) -> Vec<usize> {
    encode_classified_identifiers(
        document,
        classified_identifiers_for_file_analysis(document, file),
    )
}

pub(super) fn classified_identifiers_for_file_analysis(
    document: &OpenDocument,
    file: &FileAnalysis,
) -> Vec<ClassifiedIdentifier> {
    analysis_semantic::classified_identifiers_for_file_analysis(&document.text, file)
}

pub(super) fn classified_identifiers(document: &OpenDocument) -> Vec<ClassifiedIdentifier> {
    analysis_semantic::classified_identifiers_for_single_file_text(&document.text)
        .unwrap_or_default()
}

pub(super) const fn semantic_token_kind_index(kind: SemanticTokenKind) -> u32 {
    match kind {
        SemanticTokenKind::Function => 0,
        SemanticTokenKind::Method => 1,
        SemanticTokenKind::Variable => 2,
        SemanticTokenKind::Parameter => 3,
        SemanticTokenKind::Type => 4,
        SemanticTokenKind::Property => 5,
        SemanticTokenKind::Namespace => 6,
    }
}

fn encode_classified_identifiers(
    document: &OpenDocument,
    identifiers: Vec<ClassifiedIdentifier>,
) -> Vec<usize> {
    let semantic_tokens = identifiers
        .into_iter()
        .filter_map(|identifier| {
            let length = utf16_len(&document.text, identifier.start_byte, identifier.end_byte);
            (length > 0).then(|| SemanticToken {
                start: byte_offset_to_lsp_position(&document.text, identifier.start_byte),
                length,
                kind: identifier.kind,
                modifiers: identifier.modifiers,
            })
        })
        .collect::<Vec<_>>();

    encode_semantic_tokens(semantic_tokens)
}

fn encode_semantic_tokens(tokens: Vec<SemanticToken>) -> Vec<usize> {
    let mut tokens = tokens;
    tokens.sort_by(|left, right| {
        left.start
            .line
            .cmp(&right.start.line)
            .then(left.start.character.cmp(&right.start.character))
    });

    let mut data = Vec::with_capacity(tokens.len() * 5);
    let mut previous_line = 0;
    let mut previous_character = 0;

    for token in tokens {
        let delta_line = token.start.line - previous_line;
        let delta_character = if delta_line == 0 {
            token.start.character - previous_character
        } else {
            token.start.character
        };

        data.push(delta_line);
        data.push(delta_character);
        data.push(token.length);
        data.push(semantic_token_kind_index(token.kind) as usize);
        data.push(token.modifiers as usize);

        previous_line = token.start.line;
        previous_character = token.start.character;
    }

    data
}

fn utf16_len(text: &str, start: usize, end: usize) -> usize {
    text.get(start.min(text.len())..end.min(text.len()))
        .map(|text| text.encode_utf16().count())
        .unwrap_or(0)
}
