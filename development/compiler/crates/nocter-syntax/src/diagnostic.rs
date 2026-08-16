use nocter_source::Span;

use crate::{Keyword, Punctuation, TokenKind};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExpectedSyntax {
    Token(TokenKind),
    Keyword(Keyword),
    Punctuation(Punctuation),
    Contextual(&'static str),
    Name,
    Visibility,
    PackageDirectiveName,
    DirectiveValue,
    StringLiteral,
    ModuleSegment,
    Type,
    Parameter,
    TargetableItem,
    Item,
    DeclarationMember,
    DeclarationTypePattern,
    Receiver,
    Block,
    LiteralShape,
    Predicate,
    Capability,
    Newline,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ParseDiagnosticKind {
    Expected(ExpectedSyntax),
    LateUseDeclaration,
    NestingLimit,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ParseDiagnostic {
    kind: ParseDiagnosticKind,
    span: Span,
}

impl ParseDiagnostic {
    pub(crate) const fn new(kind: ParseDiagnosticKind, span: Span) -> Self {
        Self { kind, span }
    }

    #[must_use]
    pub const fn kind(self) -> ParseDiagnosticKind {
        self.kind
    }

    #[must_use]
    pub const fn span(self) -> Span {
        self.span
    }
}
