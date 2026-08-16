use nocter_declarations::Visibility;
use nocter_syntax::{NodeKind, Punctuation, SyntaxElement, TokenKind};

use super::HeaderError;
use crate::{
    ReservedDeclarations, SurfaceDeclaration, SurfaceDeclarationId, SurfaceDeclarationKind,
};

pub(super) fn resolve(
    reserved: &ReservedDeclarations<'_>,
    id: SurfaceDeclarationId,
    declaration: SurfaceDeclaration,
    resolved: &[Option<Visibility>],
) -> Result<Visibility, HeaderError> {
    match declaration.kind() {
        SurfaceDeclarationKind::Variant => inherited(resolved, id, declaration),
        SurfaceDeclarationKind::ConformanceMethod
        | SurfaceDeclarationKind::Construction
        | SurfaceDeclarationKind::Instance
        | SurfaceDeclarationKind::Conformance
        | SurfaceDeclarationKind::Drop
        | SurfaceDeclarationKind::Test => {
            require_absent(id, declaration)?;
            Ok(Visibility::Private)
        }
        SurfaceDeclarationKind::InterfaceMethod | SurfaceDeclarationKind::AssociatedType => {
            let visibility = authored(reserved, id, declaration)?;
            if visibility == Visibility::Public {
                Ok(visibility)
            } else {
                Err(HeaderError::InvalidVisibility(id))
            }
        }
        SurfaceDeclarationKind::OpaqueType => Err(HeaderError::InvalidVisibility(id)),
        _ => authored(reserved, id, declaration),
    }
}

fn inherited(
    resolved: &[Option<Visibility>],
    id: SurfaceDeclarationId,
    declaration: SurfaceDeclaration,
) -> Result<Visibility, HeaderError> {
    require_absent(id, declaration)?;
    declaration
        .owner()
        .and_then(|owner| resolved.get(owner.index()))
        .copied()
        .flatten()
        .ok_or(HeaderError::InvalidVisibility(id))
}

fn require_absent(
    id: SurfaceDeclarationId,
    declaration: SurfaceDeclaration,
) -> Result<(), HeaderError> {
    if declaration.visibility().is_none() {
        Ok(())
    } else {
        Err(HeaderError::InvalidVisibility(id))
    }
}

fn authored(
    reserved: &ReservedDeclarations<'_>,
    id: SurfaceDeclarationId,
    declaration: SurfaceDeclaration,
) -> Result<Visibility, HeaderError> {
    let Some(visibility) = declaration.visibility() else {
        return Ok(Visibility::Private);
    };
    let source = reserved
        .sources
        .get(declaration.source().index())
        .ok_or(HeaderError::MissingSource(declaration.source()))?;
    let tree = source.syntax();
    if tree.node(visibility).map(nocter_syntax::SyntaxNode::kind) != Some(NodeKind::Visibility) {
        return Err(HeaderError::InvalidVisibility(id));
    }
    let punctuation = descendant_punctuation(tree, visibility);
    if !punctuation.contains(&Punctuation::LeftParen) {
        return Ok(Visibility::Public);
    }
    let module_index = reserved
        .sources
        .get(declaration.source().index())
        .and_then(|source| {
            reserved
                .modules
                .iter()
                .position(|module| module == source.module())
        })
        .ok_or(HeaderError::MissingSource(declaration.source()))?;
    let module = &reserved.modules[module_index];
    if punctuation.contains(&Punctuation::Slash) && !punctuation.contains(&Punctuation::Dot) {
        let package_index = reserved
            .packages
            .iter()
            .position(|package| package.identity() == module.package())
            .ok_or(HeaderError::InvalidVisibility(id))?;
        return Ok(Visibility::Package(reserved.package_ids[package_index]));
    }
    let dots = punctuation
        .iter()
        .filter(|punctuation| **punctuation == Punctuation::Dot)
        .count();
    let ancestors = match dots {
        1 => 0,
        value if value % 2 == 0 => value / 2,
        _ => return Err(HeaderError::InvalidVisibility(id)),
    };
    if ancestors > module.path().len() {
        return Err(HeaderError::VisibilityAbovePackageRoot(id));
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
        .ok_or(HeaderError::InvalidVisibility(id))?;
    Ok(Visibility::Descendants(boundary))
}

fn descendant_punctuation(
    tree: &nocter_syntax::SyntaxTree,
    node: nocter_syntax::NodeId,
) -> Vec<Punctuation> {
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
