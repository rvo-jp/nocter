//! One syntax-owned selection authority for source items gated by `#target`.

use std::collections::BTreeSet;

use nocter_model::CompilationTarget;
use nocter_source::{SourceId, SourceMap};
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxTree};

type SyntaxKey = (SourceId, usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetSelectionError {
    MissingSource(SourceId),
    InconsistentSyntax(SourceId),
    UnknownTarget(NodeId),
}

/// The immutable item/use activity decision shared by discovery and semantic lowering.
#[derive(Debug, Default)]
pub struct TargetSelection {
    inactive_items: BTreeSet<SyntaxKey>,
    inactive_uses: BTreeSet<SyntaxKey>,
}

impl TargetSelection {
    /// Selects all supplied syntax trees for one closed compilation target.
    ///
    /// # Errors
    ///
    /// Returns an error when source storage is inconsistent with a tree or a gate names a target
    /// unknown to this compiler release.
    pub fn prepare<'tree>(
        target: CompilationTarget,
        sources: &SourceMap,
        trees: impl IntoIterator<Item = &'tree SyntaxTree>,
    ) -> Result<Self, TargetSelectionError> {
        let mut selection = Self::default();
        for tree in trees {
            selection.collect_tree(target, sources, tree)?;
        }
        Ok(selection)
    }

    #[must_use]
    pub fn item_is_active(&self, item: NodeId) -> bool {
        !self.inactive_items.contains(&key(item))
    }

    #[must_use]
    pub fn use_is_active(&self, declaration: NodeId) -> bool {
        !self.inactive_uses.contains(&key(declaration))
    }

    fn collect_tree(
        &mut self,
        target: CompilationTarget,
        sources: &SourceMap,
        tree: &SyntaxTree,
    ) -> Result<(), TargetSelectionError> {
        if tree.has_errors() || tree.root().kind() != NodeKind::SourceFile {
            return Ok(());
        }
        let source = sources
            .get(tree.source())
            .ok_or(TargetSelectionError::MissingSource(tree.source()))?;
        for child in child_nodes(tree, tree.root_id()) {
            if tree
                .node(child)
                .is_none_or(|node| node.kind() != NodeKind::Item)
            {
                continue;
            }
            let Some(gate) = direct_child(tree, child, NodeKind::TargetDirective) else {
                continue;
            };
            let literal = descendant(tree, gate, NodeKind::StringLiteral)
                .ok_or(TargetSelectionError::InconsistentSyntax(tree.source()))?;
            let name = nocter_syntax::decode_string_literal(source, tree, literal)
                .ok_or(TargetSelectionError::InconsistentSyntax(tree.source()))?;
            let selected = CompilationTarget::from_name(&name)
                .ok_or(TargetSelectionError::UnknownTarget(literal))?;
            if selected != target {
                self.inactive_items.insert(key(child));
                self.collect_inactive_uses(tree, child)?;
            }
        }
        Ok(())
    }

    fn collect_inactive_uses(
        &mut self,
        tree: &SyntaxTree,
        item: NodeId,
    ) -> Result<(), TargetSelectionError> {
        let mut pending = vec![item];
        while let Some(node) = pending.pop() {
            let kind = tree
                .node(node)
                .ok_or(TargetSelectionError::InconsistentSyntax(tree.source()))?
                .kind();
            if matches!(
                kind,
                NodeKind::UseDeclaration | NodeKind::BlockUseDeclaration
            ) {
                self.inactive_uses.insert(key(node));
            }
            pending.extend(child_nodes(tree, node));
        }
        Ok(())
    }
}

const fn key(node: NodeId) -> SyntaxKey {
    (node.source(), node.index())
}

fn child_nodes(tree: &SyntaxTree, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    tree.children(node).iter().filter_map(|child| match child {
        SyntaxElement::Node(node) => Some(*node),
        SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
    })
}

fn direct_child(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    child_nodes(tree, node).find(|child| tree.node(*child).is_some_and(|node| node.kind() == kind))
}

fn descendant(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        if tree.node(node).is_some_and(|node| node.kind() == kind) {
            return Some(node);
        }
        pending.extend(child_nodes(tree, node));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use nocter_source::SourceName;
    use nocter_syntax::{ParseGoal, parse};

    #[test]
    fn selects_nested_uses_once_for_the_requested_target() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("gated.nct"),
                b"#target: \"x64-linux\"\nfunc inactive(): void {\n    use std/io.print\n    return\n}\nfunc active(): void {\n    use std/io.print\n    return\n}\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        assert!(!tree.has_errors());

        let uses = descendants_of_kind(&tree, NodeKind::BlockUseDeclaration);
        let selection =
            TargetSelection::prepare(CompilationTarget::Arm64Darwin, &sources, [&tree]).unwrap();

        assert_eq!(uses.len(), 2);
        assert!(!selection.use_is_active(uses[0]));
        assert!(selection.use_is_active(uses[1]));
    }

    fn descendants_of_kind(tree: &SyntaxTree, kind: NodeKind) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut pending = vec![tree.root_id()];
        while let Some(node) = pending.pop() {
            if tree.node(node).is_some_and(|node| node.kind() == kind) {
                result.push(node);
            }
            pending.extend(child_nodes(tree, node));
        }
        result.sort_unstable_by_key(|node| tree.node(*node).unwrap().range().start());
        result
    }
}
