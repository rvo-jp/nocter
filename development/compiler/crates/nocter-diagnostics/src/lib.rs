//! Phase-neutral source diagnostic values.
//!
//! This crate owns only the common diagnostic envelope. Each compiler phase remains responsible
//! for deciding which language rule failed and for projecting its exact source subject into a
//! [`DiagnosticOrigin`].

use nocter_source::{SourceId, Span};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{NodeId, SyntaxOrigin, SyntaxToken};

mod human;
mod json;
mod projection;
mod syntax;

pub use human::{DiagnosticRenderError, render_source_diagnostic};
pub use json::{
    DiagnosticJsonContext, SpanlessDiagnostic, render_source_diagnostics_json,
    render_spanless_diagnostic_json, write_json_string, write_source_diagnostic_items_json,
    write_spanless_diagnostic_json,
};
pub use syntax::{lexical_diagnostic, parse_diagnostic, syntax_diagnostics};

/// Exact source subject selected by the phase that owns a diagnostic rule.
///
/// Semantic phases retain their syntax identity while lexer and parser failures use the span they
/// own before a complete syntax subject exists. Presentation consumes the common source and span
/// projection and never reconstructs either form.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DiagnosticOrigin {
    Syntax(SourceOrigin),
    Span(Span),
}

impl DiagnosticOrigin {
    #[must_use]
    pub const fn source(self) -> SourceId {
        match self {
            Self::Syntax(origin) => origin.source(),
            Self::Span(span) => span.source(),
        }
    }

    #[must_use]
    pub const fn span(self) -> Span {
        match self {
            Self::Syntax(origin) => origin.span(),
            Self::Span(span) => span,
        }
    }

    #[must_use]
    pub const fn syntax(self) -> Option<SyntaxOrigin> {
        match self {
            Self::Syntax(origin) => Some(origin.syntax()),
            Self::Span(_) => None,
        }
    }

    #[must_use]
    pub const fn node(self) -> Option<NodeId> {
        match self {
            Self::Syntax(origin) => origin.node(),
            Self::Span(_) => None,
        }
    }

    #[must_use]
    pub const fn token(self) -> Option<SyntaxToken> {
        match self {
            Self::Syntax(origin) => origin.token(),
            Self::Span(_) => None,
        }
    }
}

impl From<SourceOrigin> for DiagnosticOrigin {
    fn from(origin: SourceOrigin) -> Self {
        Self::Syntax(origin)
    }
}

impl From<Span> for DiagnosticOrigin {
    fn from(span: Span) -> Self {
        Self::Span(span)
    }
}

/// One source-backed note related to a primary diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticNote {
    message: Box<str>,
    origin: DiagnosticOrigin,
}

impl DiagnosticNote {
    #[must_use]
    pub fn new(message: impl Into<Box<str>>, origin: impl Into<DiagnosticOrigin>) -> Self {
        Self {
            message: message.into(),
            origin: origin.into(),
        }
    }

    #[must_use]
    pub const fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn origin(&self) -> DiagnosticOrigin {
        self.origin
    }
}

/// Common source diagnostic envelope shared by compiler phases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDiagnostic {
    code: Box<str>,
    message: Box<str>,
    primary: DiagnosticOrigin,
    notes: Box<[DiagnosticNote]>,
    help: Option<Box<str>>,
    repair: Option<DiagnosticRepair>,
}

/// Compiler-selected semantic repair capability attached at the point a rule fails.
///
/// Consumers use this value instead of inferring repair eligibility from diagnostic codes,
/// rendered messages, or source text. The phase that owns the failed rule also owns the exact
/// authored evidence needed by a repair.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticRepair {
    ImportUnknownName { name: Box<str> },
    ImplementMissingInterfaceMethod,
    AddCallableOutcomeContract,
}

impl SourceDiagnostic {
    #[must_use]
    pub fn new(
        code: impl Into<Box<str>>,
        message: impl Into<Box<str>>,
        primary: impl Into<DiagnosticOrigin>,
        notes: impl Into<Box<[DiagnosticNote]>>,
        help: Option<impl Into<Box<str>>>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            primary: primary.into(),
            notes: notes.into(),
            help: help.map(Into::into),
            repair: None,
        }
    }

    #[must_use]
    pub fn with_repair(mut self, repair: DiagnosticRepair) -> Self {
        self.repair = Some(repair);
        self
    }

    #[must_use]
    pub const fn code(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub const fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn primary(&self) -> DiagnosticOrigin {
        self.primary
    }

    #[must_use]
    pub const fn notes(&self) -> &[DiagnosticNote] {
        &self.notes
    }

    #[must_use]
    pub fn help(&self) -> Option<&str> {
        self.help.as_deref()
    }

    #[must_use]
    pub const fn repair(&self) -> Option<&DiagnosticRepair> {
        self.repair.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};
    use nocter_source_index::SourceOrigin;
    use nocter_syntax::{ParseGoal, parse};

    use super::{DiagnosticNote, DiagnosticRepair, SourceDiagnostic};

    #[test]
    fn envelope_keeps_primary_related_and_help_separate() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("index.nct"), b"enum Empty {}\n")
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        let origin = SourceOrigin::from_node(&tree, tree.root_id()).unwrap();
        let diagnostic = SourceDiagnostic::new(
            "E0200",
            "enum must declare at least one variant",
            origin,
            [DiagnosticNote::new("related", origin)],
            Some("add a variant"),
        )
        .with_repair(DiagnosticRepair::ImplementMissingInterfaceMethod);

        assert_eq!(diagnostic.code(), "E0200");
        assert_eq!(diagnostic.notes()[0].message(), "related");
        assert_eq!(diagnostic.help(), Some("add a variant"));
        assert_eq!(
            diagnostic.repair(),
            Some(&DiagnosticRepair::ImplementMissingInterfaceMethod)
        );
    }
}
