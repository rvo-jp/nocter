//! One grouped prefix attached to one subject; no continuing visibility mode.
use super::{
    ContextualSpelling, ExpectedSyntax, NodeKind, ParseDiagnosticKind, Parser, Punctuation, root,
};

/// Returns whether a target gate occurred. Field target gates remain outside this bounded goal.
pub(super) fn prefix(parser: &mut Parser<'_>, target_allowed: bool) -> bool {
    let mut seen = [false; 2];
    while parser.at_punctuation(Punctuation::Hash) {
        let marker = parser.start();
        parser.bump();
        let directive = if parser.at_contextual(ContextualSpelling::Visibility) {
            Some((0, NodeKind::Visibility))
        } else if parser.at_contextual(ContextualSpelling::Target) {
            if !target_allowed {
                parser.diagnostic(ParseDiagnosticKind::Expected(ExpectedSyntax::Contextual(
                    "visibility",
                )));
            }
            Some((1, NodeKind::TargetDirective))
        } else {
            None
        };
        let kind = if let Some((position, kind)) = directive {
            if seen[position] {
                parser.diagnostic(ParseDiagnosticKind::DuplicateItemDirective);
            }
            seen[position] = true;
            parser.bump();
            kind
        } else {
            parser.error_token(ExpectedSyntax::Contextual("visibility or target"));
            NodeKind::Error
        };
        parser.expect_punctuation(Punctuation::Colon);
        root::string_literal(parser);
        parser.complete(marker, kind);
        if parser.eat_newlines() == 0 {
            parser.missing(ExpectedSyntax::Newline);
        }
    }
    seen[1]
}
