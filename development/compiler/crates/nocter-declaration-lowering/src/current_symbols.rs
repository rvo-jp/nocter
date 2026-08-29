use std::fmt;

use nocter_compile_input::CompileUnitInput;
use nocter_declarations::{AcceptedDeclarationProgram, BodyAnalysisDeclarationProgram};
use nocter_source::{SourceId, SourceMap};
use nocter_syntax::{Keyword, NodeKind, SyntaxElement, TokenKind};

use crate::{ReusableDeclarations, SurfaceSource};

/// Deterministic, current-generation spellings required only while checking bodies.
///
/// Declaration lowering owns the stable symbol prefix. This value is deliberately separate so a
/// body edit cannot invalidate or renumber reusable declaration identities.
#[derive(Debug)]
pub(crate) struct CurrentCheckingSymbols {
    spellings: Box<[Box<str>]>,
}

impl CurrentCheckingSymbols {
    pub(crate) fn from_sources(
        source_map: &SourceMap,
        sources: &[SurfaceSource<'_>],
    ) -> Result<Self, CurrentSymbolError> {
        let mut spellings = Vec::new();
        for source in sources {
            collect_source_body_spellings(source_map, source, &mut spellings)?;
        }
        spellings.sort_unstable();
        spellings.dedup();
        Ok(Self {
            spellings: spellings.into_boxed_slice(),
        })
    }

    pub(crate) fn from_input(input: &CompileUnitInput<'_>) -> Result<Self, CurrentSymbolError> {
        let sources = crate::current_projection::canonical_sources(input);
        Self::from_sources(input.sources(), &sources)
    }

    pub(crate) fn extend_accepted(
        &self,
        program: AcceptedDeclarationProgram,
    ) -> AcceptedDeclarationProgram {
        program.with_checking_symbols(self.spellings.iter())
    }

    pub(crate) fn extend_body_analysis(
        &self,
        program: BodyAnalysisDeclarationProgram,
    ) -> BodyAnalysisDeclarationProgram {
        program.with_checking_symbols(self.spellings.iter())
    }
}

impl ReusableDeclarations {
    /// Opens a checking branch with the exact body-symbol suffix of `input`.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when the current source and syntax domains disagree.
    pub fn checking_branch_for(
        &self,
        input: &CompileUnitInput<'_>,
    ) -> Result<AcceptedDeclarationProgram, CurrentSymbolError> {
        let symbols = CurrentCheckingSymbols::from_input(input)?;
        Ok(symbols.extend_accepted(self.checking_branch()))
    }
}

fn collect_source_body_spellings(
    source_map: &SourceMap,
    source: &SurfaceSource<'_>,
    spellings: &mut Vec<Box<str>>,
) -> Result<(), CurrentSymbolError> {
    let tree = source.syntax();
    let file = source_map
        .get(tree.source())
        .ok_or(CurrentSymbolError::MissingSource(tree.source()))?;
    let mut declarations = vec![SyntaxElement::Node(tree.root_id())];
    while let Some(element) = declarations.pop() {
        let SyntaxElement::Node(node) = element else {
            continue;
        };
        let kind = tree
            .node(node)
            .ok_or(CurrentSymbolError::InconsistentSyntax(tree.source()))?
            .kind();
        if kind == NodeKind::Block {
            collect_subtree_spellings(file, tree, node, spellings)?;
            continue;
        }
        declarations.extend(tree.children(node).iter().rev().copied());
    }
    Ok(())
}

fn collect_subtree_spellings(
    source: &nocter_source::SourceFile,
    tree: &nocter_syntax::SyntaxTree,
    root: nocter_syntax::NodeId,
    spellings: &mut Vec<Box<str>>,
) -> Result<(), CurrentSymbolError> {
    let mut pending = vec![SyntaxElement::Node(root)];
    while let Some(element) = pending.pop() {
        match element {
            SyntaxElement::Node(node) => {
                let kind = tree
                    .node(node)
                    .ok_or(CurrentSymbolError::InconsistentSyntax(tree.source()))?
                    .kind();
                if kind == NodeKind::StringLiteral {
                    let decoded = nocter_syntax::decode_string_literal(source, tree, node)
                        .ok_or(CurrentSymbolError::InconsistentSyntax(tree.source()))?;
                    spellings.push(decoded);
                    continue;
                }
                pending.extend(tree.children(node).iter().rev().copied());
            }
            SyntaxElement::Token(token)
                if matches!(
                    token.kind(),
                    TokenKind::Identifier | TokenKind::Keyword(Keyword::Void | Keyword::Never)
                ) =>
            {
                let spelling = source
                    .text_at(token.range())
                    .ok_or(CurrentSymbolError::InconsistentSyntax(tree.source()))?;
                spellings.push(spelling.into());
            }
            SyntaxElement::Token(_) | SyntaxElement::Missing(_) => {}
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentSymbolError {
    MissingSource(SourceId),
    InconsistentSyntax(SourceId),
}

impl fmt::Display for CurrentSymbolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid current checking symbol domain: {self:?}"
        )
    }
}

impl std::error::Error for CurrentSymbolError {}
