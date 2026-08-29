use std::collections::HashMap;

use crate::{
    DeclarationSyntaxLocator, NodeId, SyntaxElement, SyntaxOrigin, SyntaxToken, SyntaxTree,
};

/// Source-independent position of one node or token inside an executable body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BodySyntaxLocator {
    Node(u32),
    Token(u32),
}

/// Bidirectional current-generation interpretation of body-local syntax locators.
pub struct BodySyntaxProjection {
    nodes: Box<[NodeId]>,
    tokens: Box<[SyntaxToken]>,
    locators: HashMap<SyntaxOrigin, BodySyntaxLocator>,
}

impl BodySyntaxProjection {
    /// Enumerates one body in deterministic syntax order.
    #[must_use]
    pub fn for_body(tree: &SyntaxTree, body: NodeId) -> Option<Self> {
        tree.node(body)?;
        let mut pending = vec![SyntaxElement::Node(body)];
        let mut nodes = Vec::new();
        let mut tokens = Vec::new();
        let mut locators = HashMap::new();
        while let Some(element) = pending.pop() {
            match element {
                SyntaxElement::Node(node) => {
                    let index = u32::try_from(nodes.len()).ok()?;
                    nodes.push(node);
                    locators.insert(SyntaxOrigin::Node(node), BodySyntaxLocator::Node(index));
                    pending.extend(tree.children(node).iter().rev().copied());
                }
                SyntaxElement::Token(token) => {
                    let index = u32::try_from(tokens.len()).ok()?;
                    tokens.push(token);
                    locators.insert(SyntaxOrigin::Token(token), BodySyntaxLocator::Token(index));
                }
                SyntaxElement::Missing(_) => {}
            }
        }
        Some(Self {
            nodes: nodes.into_boxed_slice(),
            tokens: tokens.into_boxed_slice(),
            locators,
        })
    }

    #[must_use]
    pub fn locator(&self, origin: SyntaxOrigin) -> Option<BodySyntaxLocator> {
        self.locators.get(&origin).copied()
    }

    #[must_use]
    pub fn resolve(&self, locator: BodySyntaxLocator) -> Option<SyntaxOrigin> {
        match locator {
            BodySyntaxLocator::Node(index) => self
                .nodes
                .get(usize::try_from(index).ok()?)
                .copied()
                .map(SyntaxOrigin::Node),
            BodySyntaxLocator::Token(index) => self
                .tokens
                .get(usize::try_from(index).ok()?)
                .copied()
                .map(SyntaxOrigin::Token),
        }
    }
}

/// Exact normalized source input of one executable declaration body.
///
/// `locator` belongs to the enclosing declaration surface and is therefore stable across edits to
/// this or another body while that declaration surface remains equal. Exact body bytes retain all
/// source information required by diagnostics; later semantic queries may derive a narrower
/// semantic fingerprint only when they publish the matching source projection with it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodySyntaxSurface {
    locator: DeclarationSyntaxLocator,
    canonical: Box<[u8]>,
}

impl BodySyntaxSurface {
    #[must_use]
    pub const fn locator(&self) -> DeclarationSyntaxLocator {
        self.locator
    }

    #[must_use]
    pub const fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
}

pub(crate) fn body_surface(
    locator: DeclarationSyntaxLocator,
    tree: &SyntaxTree,
    body: NodeId,
    normalized_text: &str,
) -> BodySyntaxSurface {
    let range = tree
        .node(body)
        .expect("a declaration surface body belongs to its syntax tree")
        .range();
    let start = usize::try_from(range.start().get()).expect("source offsets fit usize");
    let end = usize::try_from(range.end().get()).expect("source offsets fit usize");
    let canonical = normalized_text
        .get(start..end)
        .expect("body ranges address normalized source text")
        .as_bytes()
        .into();
    BodySyntaxSurface { locator, canonical }
}
