use nocter_source_index::{SourceOrigin, SyntaxOrigin};
use nocter_syntax::{NodeId, SyntaxTree};

use crate::CompileUnitInput;

/// One source-backed note related to a primary diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticNote {
    message: Box<str>,
    origin: SourceOrigin,
}

impl DiagnosticNote {
    #[must_use]
    pub fn new(message: impl Into<Box<str>>, origin: SourceOrigin) -> Self {
        Self {
            message: message.into(),
            origin,
        }
    }

    #[must_use]
    pub const fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn origin(&self) -> SourceOrigin {
        self.origin
    }
}

/// Common source diagnostic envelope shared by declaration-lowering stages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDiagnostic {
    code: Box<str>,
    message: Box<str>,
    primary: SourceOrigin,
    notes: Box<[DiagnosticNote]>,
    help: Option<Box<str>>,
}

impl SourceDiagnostic {
    #[must_use]
    pub fn new(
        code: impl Into<Box<str>>,
        message: impl Into<Box<str>>,
        primary: SourceOrigin,
        notes: impl Into<Box<[DiagnosticNote]>>,
        help: Option<impl Into<Box<str>>>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            primary,
            notes: notes.into(),
            help: help.map(Into::into),
        }
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
    pub const fn primary(&self) -> SourceOrigin {
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
}

pub(crate) fn origin_from_trees<'syntax>(
    trees: impl IntoIterator<Item = &'syntax SyntaxTree>,
    node: NodeId,
) -> Option<SourceOrigin> {
    origin_from_syntax(trees, SyntaxOrigin::Node(node))
}

pub(crate) fn origin_from_syntax<'syntax>(
    trees: impl IntoIterator<Item = &'syntax SyntaxTree>,
    syntax: SyntaxOrigin,
) -> Option<SourceOrigin> {
    let source = match syntax {
        SyntaxOrigin::Node(node) => node.source(),
        SyntaxOrigin::Token(token) => token.source(),
    };
    let tree = trees.into_iter().find(|tree| tree.source() == source)?;
    match syntax {
        SyntaxOrigin::Node(node) => SourceOrigin::from_node(tree, node).ok(),
        SyntaxOrigin::Token(token) => SourceOrigin::from_token(tree, token).ok(),
    }
}

pub(crate) fn input_trees<'input, 'syntax: 'input>(
    input: &'input CompileUnitInput<'syntax>,
) -> impl Iterator<Item = &'syntax SyntaxTree> + 'input {
    input
        .packages()
        .iter()
        .filter_map(|package| {
            package
                .declaration()
                .map(crate::PackageDeclarationInput::syntax)
        })
        .chain(input.modules().iter().flat_map(|module| {
            module
                .sources()
                .iter()
                .map(crate::ModuleSourceInput::syntax)
        }))
}

pub(crate) fn project_syntax_diagnostic(
    input: &CompileUnitInput<'_>,
    primary: SyntaxOrigin,
    related: Option<SyntaxOrigin>,
    code: &'static str,
    message: &'static str,
    related_message: Option<&'static str>,
    help: &'static str,
) -> Option<SourceDiagnostic> {
    let primary = origin_from_syntax(input_trees(input), primary)?;
    let related = match related {
        Some(syntax) => Some(origin_from_syntax(input_trees(input), syntax)?),
        None => None,
    };
    let notes = related
        .zip(related_message)
        .map(|(origin, message)| DiagnosticNote::new(message, origin))
        .into_iter()
        .collect::<Vec<_>>();
    Some(SourceDiagnostic::new(
        code,
        message,
        primary,
        notes,
        Some(help),
    ))
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};
    use nocter_source_index::SourceOrigin;
    use nocter_syntax::{ParseGoal, parse};

    use super::{DiagnosticNote, SourceDiagnostic};

    #[test]
    fn envelope_keeps_primary_related_and_help_separate() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("index.nct"), b"enum Empty {}\n")
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::ModuleSource);
        let origin = SourceOrigin::from_node(&tree, tree.root_id()).unwrap();
        let diagnostic = SourceDiagnostic::new(
            "E0200",
            "enum must declare at least one variant",
            origin,
            [DiagnosticNote::new("related", origin)],
            Some("add a variant"),
        );

        assert_eq!(diagnostic.code(), "E0200");
        assert_eq!(diagnostic.notes()[0].message(), "related");
        assert_eq!(diagnostic.help(), Some("add a variant"));
    }
}
