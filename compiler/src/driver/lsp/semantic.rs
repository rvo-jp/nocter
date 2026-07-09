use super::documents::OpenDocument;
use super::protocol::{LspPosition, byte_offset_to_lsp_position};
use crate::lexer::{Keyword, Token, TokenKind, lex};
use crate::source::SourceMap;

pub(super) const SEMANTIC_TOKEN_TYPES: [&str; 5] =
    ["function", "variable", "parameter", "type", "property"];
pub(super) const SEMANTIC_TOKEN_MODIFIERS: [&str; 1] = ["declaration"];
pub(super) const SEMANTIC_DECLARATION_MODIFIER: u32 = 1 << 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SemanticTokenKind {
    Function,
    Variable,
    Parameter,
    Type,
    Property,
}

impl SemanticTokenKind {
    pub(super) const fn index(self) -> u32 {
        match self {
            SemanticTokenKind::Function => 0,
            SemanticTokenKind::Variable => 1,
            SemanticTokenKind::Parameter => 2,
            SemanticTokenKind::Type => 3,
            SemanticTokenKind::Property => 4,
        }
    }

    pub(super) const fn hover_label(self) -> &'static str {
        match self {
            SemanticTokenKind::Function => "function",
            SemanticTokenKind::Variable => "variable",
            SemanticTokenKind::Parameter => "parameter",
            SemanticTokenKind::Type => "type",
            SemanticTokenKind::Property => "property",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemanticToken {
    start: LspPosition,
    length: usize,
    kind: SemanticTokenKind,
    modifiers: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ClassifiedIdentifier {
    pub(super) start_byte: usize,
    pub(super) end_byte: usize,
    pub(super) kind: SemanticTokenKind,
    pub(super) modifiers: u32,
}

pub(super) fn semantic_tokens_for_document(document: &OpenDocument) -> Vec<usize> {
    let semantic_tokens = classified_identifiers(document)
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

pub(super) fn classified_identifiers(document: &OpenDocument) -> Vec<ClassifiedIdentifier> {
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let lex_output = lex(&sources, source);
    let tokens = lex_output
        .tokens
        .iter()
        .filter(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
        .collect::<Vec<_>>();

    let mut identifiers = Vec::new();
    let mut pending_declaration = None;

    for (index, token) in tokens.iter().enumerate() {
        let previous = index
            .checked_sub(1)
            .and_then(|index| tokens.get(index))
            .copied();
        let next = tokens.get(index + 1).copied();

        match token.kind {
            TokenKind::Keyword(keyword) => {
                pending_declaration = pending_declaration_for_keyword(keyword);
            }
            TokenKind::Identifier => {
                let modifiers = if pending_declaration.is_some() {
                    SEMANTIC_DECLARATION_MODIFIER
                } else {
                    0
                };
                let kind = pending_declaration
                    .take()
                    .unwrap_or_else(|| classify_identifier(&document.text, token, previous, next));
                if token.span.start < token.span.end {
                    identifiers.push(ClassifiedIdentifier {
                        start_byte: token.span.start,
                        end_byte: token.span.end,
                        kind,
                        modifiers,
                    });
                }
            }
            _ => {
                if !matches!(
                    token.kind,
                    TokenKind::Punctuation("<")
                        | TokenKind::Punctuation(">")
                        | TokenKind::Punctuation(",")
                ) {
                    pending_declaration = None;
                }
            }
        }
    }

    identifiers.sort_by(|left, right| {
        left.start_byte
            .cmp(&right.start_byte)
            .then(left.end_byte.cmp(&right.end_byte))
    });
    identifiers
}

fn pending_declaration_for_keyword(keyword: Keyword) -> Option<SemanticTokenKind> {
    match keyword {
        Keyword::Func | Keyword::Method => Some(SemanticTokenKind::Function),
        Keyword::Type | Keyword::Struct | Keyword::Enum | Keyword::Trait | Keyword::Primitive => {
            Some(SemanticTokenKind::Type)
        }
        Keyword::Let | Keyword::Var => Some(SemanticTokenKind::Variable),
        _ => None,
    }
}

fn classify_identifier(
    text: &str,
    token: &Token,
    previous: Option<&Token>,
    next: Option<&Token>,
) -> SemanticTokenKind {
    if matches!(
        previous.map(|token| token.kind),
        Some(TokenKind::Punctuation("."))
    ) {
        return SemanticTokenKind::Property;
    }

    if matches!(
        next.map(|token| token.kind),
        Some(TokenKind::Punctuation("("))
    ) {
        return SemanticTokenKind::Function;
    }

    if matches!(
        next.map(|token| token.kind),
        Some(TokenKind::Punctuation(":"))
    ) {
        return SemanticTokenKind::Parameter;
    }

    let lexeme = text
        .get(token.span.start..token.span.end)
        .unwrap_or_default();
    if lexeme
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
    {
        return SemanticTokenKind::Type;
    }

    SemanticTokenKind::Variable
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
        data.push(token.kind.index() as usize);
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
