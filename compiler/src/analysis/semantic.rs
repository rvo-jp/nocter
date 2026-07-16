//! Semantic identifier classification shared by editor tooling.

use super::FileAnalysis;
use crate::lexer::{Keyword, Token, TokenKind, lex};
use crate::resolve::is_builtin_type_name;
use crate::source::{ByteSpan, SourceMap};

pub(crate) const SEMANTIC_DECLARATION_MODIFIER: u32 = 1 << 0;
pub(crate) const SEMANTIC_READONLY_MODIFIER: u32 = 1 << 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticTokenKind {
    Function,
    Method,
    Variable,
    Parameter,
    Type,
    Property,
}

impl SemanticTokenKind {
    pub(crate) const fn hover_label(self) -> &'static str {
        match self {
            SemanticTokenKind::Function => "function",
            SemanticTokenKind::Method => "method",
            SemanticTokenKind::Variable => "variable",
            SemanticTokenKind::Parameter => "parameter",
            SemanticTokenKind::Type => "type",
            SemanticTokenKind::Property => "property",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassifiedIdentifier {
    pub(crate) start_byte: usize,
    pub(crate) end_byte: usize,
    pub(crate) kind: SemanticTokenKind,
    pub(crate) modifiers: u32,
}

pub(crate) fn classified_identifiers_for_file_analysis(
    text: &str,
    file: &FileAnalysis,
) -> Vec<ClassifiedIdentifier> {
    let mut identifiers = classified_identifiers_for_text(text);
    apply_typecheck_semantic_facts(&mut identifiers, file);
    identifiers
}

pub(crate) fn classified_identifiers_for_text(text: &str) -> Vec<ClassifiedIdentifier> {
    let mut sources = SourceMap::new();
    let source = sources.add_source("semantic.nct", None, text.to_string());
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
                if let Some(kind) = semantic_token_kind_for_keyword(keyword) {
                    pending_declaration = None;
                    push_identifier(&mut identifiers, token, kind, 0);
                } else {
                    pending_declaration = pending_declaration_for_keyword(keyword);
                }
            }
            TokenKind::Identifier => {
                let is_method_declaration_name = is_method_declaration_name(&tokens, index);
                let is_associated_function_owner =
                    is_associated_function_owner(&tokens, index, pending_declaration);
                let is_associated_function_call_name =
                    is_associated_function_call_name(text, &tokens, index);
                let modifiers = if (pending_declaration.is_some() && !is_associated_function_owner)
                    || is_method_declaration_name
                {
                    SEMANTIC_DECLARATION_MODIFIER
                } else {
                    0
                };
                let kind = if is_associated_function_owner {
                    SemanticTokenKind::Type
                } else if is_method_declaration_name {
                    pending_declaration = None;
                    SemanticTokenKind::Method
                } else if is_associated_function_call_name {
                    pending_declaration = None;
                    SemanticTokenKind::Function
                } else {
                    pending_declaration
                        .take()
                        .unwrap_or_else(|| classify_identifier(text, token, previous, next))
                };
                push_identifier(&mut identifiers, token, kind, modifiers);
            }
            _ => {
                if !matches!(
                    token.kind,
                    TokenKind::Punctuation("<")
                        | TokenKind::Punctuation(">")
                        | TokenKind::Punctuation(",")
                        | TokenKind::Punctuation(".")
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

fn push_identifier(
    identifiers: &mut Vec<ClassifiedIdentifier>,
    token: &Token,
    kind: SemanticTokenKind,
    modifiers: u32,
) {
    if token.span.start < token.span.end {
        identifiers.push(ClassifiedIdentifier {
            start_byte: token.span.start,
            end_byte: token.span.end,
            kind,
            modifiers,
        });
    }
}

fn apply_typecheck_semantic_facts(identifiers: &mut [ClassifiedIdentifier], file: &FileAnalysis) {
    for identifier in identifiers {
        let span = ByteSpan::new(
            file.ast.span.source,
            identifier.start_byte,
            identifier.end_byte,
        );
        if file.typecheck_facts.method_call_target(span).is_some() {
            identifier.kind = SemanticTokenKind::Method;
            continue;
        }
        if file
            .typecheck_facts
            .type_reference_spans()
            .any(|reference_span| reference_span == span)
        {
            identifier.kind = SemanticTokenKind::Type;
        }
        if matches!(
            identifier.kind,
            SemanticTokenKind::Variable | SemanticTokenKind::Parameter
        ) && file.typecheck_facts.binding_is_readonly(span) == Some(true)
        {
            identifier.modifiers |= SEMANTIC_READONLY_MODIFIER;
        }
    }
}

fn semantic_token_kind_for_keyword(keyword: Keyword) -> Option<SemanticTokenKind> {
    match keyword {
        Keyword::Void | Keyword::Never => Some(SemanticTokenKind::Type),
        _ => None,
    }
}

fn pending_declaration_for_keyword(keyword: Keyword) -> Option<SemanticTokenKind> {
    match keyword {
        Keyword::Func | Keyword::Primitive => Some(SemanticTokenKind::Function),
        Keyword::Method => None,
        Keyword::Type | Keyword::Struct | Keyword::Enum | Keyword::Trait => {
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
        if matches!(
            next.map(|token| token.kind),
            Some(TokenKind::Punctuation("("))
        ) {
            return SemanticTokenKind::Method;
        }
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
    if is_fallback_type_name(lexeme) {
        return SemanticTokenKind::Type;
    }

    if lexeme
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
    {
        return SemanticTokenKind::Type;
    }

    SemanticTokenKind::Variable
}

fn is_fallback_type_name(lexeme: &str) -> bool {
    is_builtin_type_name(lexeme) || matches!(lexeme, "error" | "void" | "never")
}

fn is_associated_function_owner(
    tokens: &[&Token],
    index: usize,
    pending_declaration: Option<SemanticTokenKind>,
) -> bool {
    pending_declaration == Some(SemanticTokenKind::Function)
        && matches!(
            tokens.get(index + 1),
            Some(token) if matches!(token.kind, TokenKind::Punctuation("."))
        )
}

fn is_associated_function_call_name(text: &str, tokens: &[&Token], index: usize) -> bool {
    if !matches!(
        index.checked_sub(1).and_then(|index| tokens.get(index)),
        Some(token) if matches!(token.kind, TokenKind::Punctuation("."))
    ) || !matches!(
        tokens.get(index + 1),
        Some(token) if matches!(token.kind, TokenKind::Punctuation("("))
    ) {
        return false;
    }

    let Some(object) = index.checked_sub(2).and_then(|index| tokens.get(index)) else {
        return false;
    };
    if !matches!(object.kind, TokenKind::Identifier) {
        return false;
    }

    text.get(object.span.start..object.span.end)
        .and_then(|lexeme| lexeme.chars().next())
        .is_some_and(|first| first.is_ascii_uppercase())
}

fn is_method_declaration_name(tokens: &[&Token], index: usize) -> bool {
    if !matches!(
        index.checked_sub(1).and_then(|index| tokens.get(index)),
        Some(token) if matches!(token.kind, TokenKind::Punctuation("."))
    ) || !matches!(
        tokens.get(index + 1),
        Some(token) if matches!(token.kind, TokenKind::Punctuation("("))
    ) {
        return false;
    }

    let Some(close_receiver) = index.checked_sub(2).and_then(|index| tokens.get(index)) else {
        return false;
    };
    if !matches!(close_receiver.kind, TokenKind::Punctuation(")")) {
        return false;
    }

    let mut depth = 0usize;
    let mut cursor = index - 2;
    loop {
        match tokens[cursor].kind {
            TokenKind::Punctuation(")") => depth += 1,
            TokenKind::Punctuation("(") => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return cursor
                        .checked_sub(1)
                        .and_then(|index| tokens.get(index))
                        .is_some_and(|token| {
                            matches!(token.kind, TokenKind::Keyword(Keyword::Method))
                        });
                }
            }
            _ => {}
        }

        let Some(next_cursor) = cursor.checked_sub(1) else {
            return false;
        };
        cursor = next_cursor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{CompileUnit, analyze_compile_unit_as_modules};
    use crate::lexer::lex;
    use crate::parser::parse;
    use std::collections::HashMap;

    #[test]
    fn fallback_classifies_builtin_types() {
        let text = "func main(path: &str): void! {\n    let byte: u8 = 0 as u8\n    return\n}\n";
        let identifiers = classified_identifiers_for_text(text);

        for name in ["str", "void", "u8"] {
            assert!(
                identifiers_for_lexeme(text, &identifiers, name)
                    .iter()
                    .any(|identifier| identifier.kind == SemanticTokenKind::Type),
                "expected `{name}` to be classified as a type"
            );
        }
    }

    #[test]
    fn fallback_classifies_associated_function_owner_as_type() {
        let text =
            "struct Point {\n}\n\nfunc Point.origin(): Point {\n    return Point.origin()\n}\n";
        let identifiers = classified_identifiers_for_text(text);

        assert!(
            identifiers_for_lexeme(text, &identifiers, "Point")
                .iter()
                .all(|identifier| identifier.kind == SemanticTokenKind::Type),
            "expected associated function owner names to be classified as types"
        );

        let origin_identifiers = identifiers_for_lexeme(text, &identifiers, "origin");
        assert!(
            origin_identifiers.iter().any(|identifier| {
                identifier.kind == SemanticTokenKind::Function
                    && identifier.modifiers & SEMANTIC_DECLARATION_MODIFIER != 0
            }),
            "expected associated function declaration name to be a function declaration"
        );
        assert!(
            origin_identifiers.iter().any(|identifier| {
                identifier.kind == SemanticTokenKind::Function
                    && identifier.modifiers & SEMANTIC_DECLARATION_MODIFIER == 0
            }),
            "expected associated function call name to be a function"
        );
    }

    #[test]
    fn analysis_classification_uses_typecheck_facts() {
        let text = "func main(path: &str): i32 {\n    let alpha = 1\n    var beta = 2\n    return alpha + beta\n}\n";
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let source = sources.get(file.ast.span.source).expect("expected source");
        let identifiers = classified_identifiers_for_file_analysis(source.text(), file);

        assert!(
            identifiers_for_lexeme(text, &identifiers, "alpha")
                .iter()
                .all(|identifier| identifier.modifiers & SEMANTIC_READONLY_MODIFIER != 0),
            "expected `alpha` to be marked readonly"
        );
        assert!(
            identifiers_for_lexeme(text, &identifiers, "beta")
                .iter()
                .all(|identifier| identifier.modifiers & SEMANTIC_READONLY_MODIFIER == 0),
            "expected `beta` to remain mutable"
        );
    }

    fn identifiers_for_lexeme<'a>(
        text: &str,
        identifiers: &'a [ClassifiedIdentifier],
        lexeme: &str,
    ) -> Vec<&'a ClassifiedIdentifier> {
        identifiers
            .iter()
            .filter(|identifier| text[identifier.start_byte..identifier.end_byte] == *lexeme)
            .collect()
    }

    fn analyze_text(text: &str) -> (SourceMap, crate::analysis::CompileUnitAnalysis) {
        let mut sources = SourceMap::new();
        let source = sources.add_source("test.nct", None, text.to_string());
        let lex_output = lex(&sources, source);
        assert!(
            lex_output.diagnostics.is_empty(),
            "unexpected lex diagnostics: {:?}",
            lex_output.diagnostics
        );
        let ast = parse(&sources, source, &lex_output.tokens)
            .ast
            .expect("expected ast");
        let unit = CompileUnit::new(ast.clone(), vec![ast], HashMap::new());
        let analysis = analyze_compile_unit_as_modules(&sources, &unit);

        (sources, analysis)
    }
}
