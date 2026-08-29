use crate::{DeclarationSyntaxLocator, NodeId, SyntaxTree};

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
