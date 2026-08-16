use nocter_declarations::Visibility;
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxElement, TokenKind};

use crate::{ReservedDeclarations, SurfaceSourceId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VisibilityResolutionError {
    MissingSource(SurfaceSourceId),
    Invalid(NodeId),
    AbovePackageRoot(NodeId),
}

pub(crate) fn resolve_authored(
    reserved: &ReservedDeclarations<'_>,
    source_id: SurfaceSourceId,
    visibility: Option<NodeId>,
) -> Result<Visibility, VisibilityResolutionError> {
    let Some(visibility) = visibility else {
        return Ok(Visibility::Private);
    };
    let source = reserved
        .sources
        .get(source_id.index())
        .ok_or(VisibilityResolutionError::MissingSource(source_id))?;
    let tree = source.syntax();
    if tree.node(visibility).map(nocter_syntax::SyntaxNode::kind) != Some(NodeKind::Visibility) {
        return Err(VisibilityResolutionError::Invalid(visibility));
    }
    let punctuation = descendant_punctuation(tree, visibility);
    if !punctuation.contains(&Punctuation::LeftParen) {
        return Ok(Visibility::Public);
    }
    let module_index = reserved
        .modules
        .iter()
        .position(|module| module == source.module())
        .ok_or(VisibilityResolutionError::MissingSource(source_id))?;
    let module = &reserved.modules[module_index];
    if punctuation.contains(&Punctuation::Slash) && !punctuation.contains(&Punctuation::Dot) {
        let package_index = reserved
            .packages
            .iter()
            .position(|package| package.identity() == module.package())
            .ok_or(VisibilityResolutionError::Invalid(visibility))?;
        return Ok(Visibility::Package(reserved.package_ids[package_index]));
    }
    let dots = punctuation
        .iter()
        .filter(|punctuation| **punctuation == Punctuation::Dot)
        .count();
    let ancestors = match dots {
        1 => 0,
        value if value % 2 == 0 => value / 2,
        _ => return Err(VisibilityResolutionError::Invalid(visibility)),
    };
    if ancestors > module.path().len() {
        return Err(VisibilityResolutionError::AbovePackageRoot(visibility));
    }
    let boundary_path = &module.path()[..module.path().len() - ancestors];
    let boundary = reserved
        .modules
        .iter()
        .position(|candidate| {
            candidate.package() == module.package() && candidate.path() == boundary_path
        })
        .and_then(|index| reserved.module_ids.get(index))
        .copied()
        .ok_or(VisibilityResolutionError::Invalid(visibility))?;
    Ok(Visibility::Descendants(boundary))
}

fn descendant_punctuation(tree: &nocter_syntax::SyntaxTree, node: NodeId) -> Vec<Punctuation> {
    let mut punctuation = Vec::new();
    let mut pending: Vec<_> = tree.children(node).iter().rev().copied().collect();
    while let Some(element) = pending.pop() {
        match element {
            SyntaxElement::Node(node) => pending.extend(tree.children(node).iter().rev().copied()),
            SyntaxElement::Token(token) => {
                if let TokenKind::Punctuation(value) = token.kind() {
                    punctuation.push(value);
                }
            }
            SyntaxElement::Missing(_) => {}
        }
    }
    punctuation
}
