//! One syntax-owned selection authority for source items gated by `#target`.

use std::collections::{BTreeMap, BTreeSet};

use nocter_model::CompilationTarget;
use nocter_source::{SourceId, SourceMap};
use nocter_syntax::{
    NodeId, NodeKind, SyntaxTree, child_node_iter as child_nodes, direct_node as direct_child,
};

type SyntaxKey = (SourceId, usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetSelectionError {
    MissingSource(SourceId),
    InconsistentSyntax(SourceId),
    UnknownTarget(NodeId),
}

/// The immutable item/use activity decision shared by discovery and semantic lowering.
#[derive(Clone, Debug, Default)]
pub struct TargetSelection {
    item_targets: BTreeMap<SyntaxKey, CompilationTarget>,
    inactive_items: BTreeSet<SyntaxKey>,
    inactive_uses: BTreeSet<SyntaxKey>,
}

/// Sole incremental construction authority used while discovery closes a package graph.
#[derive(Debug, Default)]
pub struct TargetSelectionBuilder {
    selection: TargetSelection,
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
        let mut builder = TargetSelectionBuilder::new();
        for tree in trees {
            builder.include_tree(target, sources, tree)?;
        }
        Ok(builder.finish())
    }

    #[must_use]
    pub fn item_is_active(&self, item: NodeId) -> bool {
        !self.inactive_items.contains(&key(item))
    }

    /// Returns the normalized target named by one gated item.
    #[must_use]
    pub fn item_target(&self, item: NodeId) -> Option<CompilationTarget> {
        self.item_targets.get(&key(item)).copied()
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
            self.item_targets.insert(key(child), selected);
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

impl TargetSelectionBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Extends this authority with one newly discovered syntax tree.
    ///
    /// # Errors
    ///
    /// Returns an error when the source snapshot is inconsistent or a gate names an unknown
    /// target.
    pub fn include_tree(
        &mut self,
        target: CompilationTarget,
        sources: &SourceMap,
        tree: &SyntaxTree,
    ) -> Result<(), TargetSelectionError> {
        self.selection.collect_tree(target, sources, tree)
    }

    /// Borrows the decisions completed so far for dependency traversal.
    #[must_use]
    pub const fn selection(&self) -> &TargetSelection {
        &self.selection
    }

    #[must_use]
    pub fn finish(self) -> TargetSelection {
        self.selection
    }
}

const fn key(node: NodeId) -> SyntaxKey {
    (node.source(), node.index())
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
        let items = descendants_of_kind(&tree, NodeKind::Item);
        let selection =
            TargetSelection::prepare(CompilationTarget::Arm64Darwin, &sources, [&tree]).unwrap();

        assert_eq!(uses.len(), 2);
        assert_eq!(items.len(), 2);
        assert_eq!(
            selection.item_target(items[0]),
            Some(CompilationTarget::X64Linux)
        );
        assert_eq!(selection.item_target(items[1]), None);
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
