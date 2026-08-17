use std::collections::BTreeMap;

use nocter_declarations::{DeclarationProgramBuilder, PackageTarget, PackageTargetKind};
use nocter_model::{ModuleId, PackageId};
use nocter_source::{SourceFile, SourceMap};
use nocter_source_index::{SemanticEntity, SourceIndexBuilder, SourceOrigin, SourceRole};
use nocter_syntax::{Keyword, NodeId, NodeKind, SyntaxElement, SyntaxTree, TokenKind};

use crate::{ModuleIdentity, PackageInput, PackageTargetResolutionInput, ReservationError};

/// Reserves discovery-selected package targets without interpreting an authored module path.
pub(crate) fn reserve_package_targets(
    source_map: &SourceMap,
    packages: &[PackageInput<'_>],
    resolutions: &[PackageTargetResolutionInput],
    package_ids: &BTreeMap<crate::PackageIdentity, PackageId>,
    module_ids: &BTreeMap<ModuleIdentity, ModuleId>,
    program: &mut DeclarationProgramBuilder,
    source_index: &mut SourceIndexBuilder,
) -> Result<(), ReservationError> {
    let mut selected_names = BTreeMap::new();
    for resolution in resolutions {
        let declaration = resolution.declaration();
        let package = packages
            .iter()
            .find(|package| {
                package
                    .declaration()
                    .is_some_and(|input| input.syntax().source() == declaration.source())
            })
            .ok_or(ReservationError::InvalidPackageTarget(declaration))?;
        let package_declaration = package
            .declaration()
            .ok_or(ReservationError::InvalidPackageTarget(declaration))?;
        let tree = package_declaration.syntax();
        let source = source_map
            .get(tree.source())
            .ok_or(ReservationError::InconsistentSource(tree.source()))?;
        let (kind, order) = target_kind_and_order(source, tree, declaration)?;
        let (name, name_literal) = target_name(source, tree, declaration)?;
        let name_symbol = program
            .symbols()
            .get(&name)
            .ok_or_else(|| ReservationError::MissingSymbol(name.clone()))?;
        let package_id = *package_ids
            .get(package.identity())
            .ok_or_else(|| ReservationError::UnknownPackage(resolution.module().clone()))?;
        let module_id = *module_ids
            .get(resolution.module())
            .ok_or_else(|| ReservationError::UnknownModule(resolution.module().clone()))?;
        if selected_names
            .insert((package_id, kind, name_symbol), declaration)
            .is_some()
        {
            return Err(ReservationError::DuplicatePackageTarget(declaration));
        }
        let id = program.add_package_target(PackageTarget::new(
            package_id,
            module_id,
            name_symbol,
            kind,
            order,
        ))?;
        source_index.insert(
            SemanticEntity::PackageTarget(id),
            SourceRole::Declaration,
            SourceOrigin::from_node(tree, name_literal)
                .map_err(|_| ReservationError::InconsistentSource(tree.source()))?,
        )?;
    }
    Ok(())
}

fn target_kind_and_order(
    source: &SourceFile,
    tree: &SyntaxTree,
    declaration: NodeId,
) -> Result<(PackageTargetKind, u32), ReservationError> {
    let mut order = 0_u32;
    for child in child_nodes(tree, tree.root_id()) {
        if tree
            .node(child)
            .is_none_or(|node| node.kind() != NodeKind::PackageDirective)
        {
            continue;
        }
        let Some(kind) = target_kind(source, tree, child) else {
            continue;
        };
        if child == declaration {
            return Ok((kind, order));
        }
        order = order
            .checked_add(1)
            .ok_or(ReservationError::InvalidPackageTarget(declaration))?;
    }
    Err(ReservationError::InvalidPackageTarget(declaration))
}

fn target_kind(
    source: &SourceFile,
    tree: &SyntaxTree,
    declaration: NodeId,
) -> Option<PackageTargetKind> {
    tree.children(declaration).iter().find_map(|element| {
        let SyntaxElement::Token(token) = element else {
            return None;
        };
        if token.kind() == TokenKind::Keyword(Keyword::Test) {
            Some(PackageTargetKind::Test)
        } else if token.kind() == TokenKind::Identifier
            && source.text_at(token.range()) == Some("executable")
        {
            Some(PackageTargetKind::Executable)
        } else {
            None
        }
    })
}

fn target_name(
    source: &SourceFile,
    tree: &SyntaxTree,
    declaration: NodeId,
) -> Result<(Box<str>, NodeId), ReservationError> {
    let mut result = None;
    for field in descendants(tree, declaration, NodeKind::DirectiveField) {
        let is_name = tree.children(field).iter().any(|element| {
            matches!(element, SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier && source.text_at(token.range()) == Some("name"))
        });
        if !is_name {
            continue;
        }
        let literal = descendants(tree, field, NodeKind::StringLiteral)
            .into_iter()
            .next()
            .ok_or(ReservationError::InvalidPackageTarget(declaration))?;
        let name = nocter_syntax::decode_string_literal(source, tree, literal)
            .ok_or(ReservationError::InvalidPackageTarget(declaration))?;
        if name.is_empty() || result.replace((name, literal)).is_some() {
            return Err(ReservationError::InvalidPackageTarget(declaration));
        }
    }
    result.ok_or(ReservationError::InvalidPackageTarget(declaration))
}

fn child_nodes(tree: &SyntaxTree, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    tree.children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(node) => Some(*node),
            SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
}

fn descendants(tree: &SyntaxTree, root: NodeId, kind: NodeKind) -> Vec<NodeId> {
    let mut result = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if tree.node(node).is_some_and(|node| node.kind() == kind) {
            result.push(node);
            continue;
        }
        pending.extend(child_nodes(tree, node));
    }
    result
}
