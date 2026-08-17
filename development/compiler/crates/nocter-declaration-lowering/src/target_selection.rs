use std::collections::BTreeSet;

use nocter_model::CompilationTarget;
use nocter_source::SourceId;
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxTree};

use crate::{CompileUnitInput, LoweringError, ModuleInput};

type SyntaxKey = (SourceId, usize);

/// The single target-gate decision shared by topology, symbol collection, and surface lowering.
///
/// Keeping this inventory syntax-owned and temporary prevents later stages from seeing an
/// inactive declaration and prevents three frontend passes from reinterpreting `#target`.
#[derive(Debug)]
pub(crate) struct TargetSelection {
    inactive_items: BTreeSet<SyntaxKey>,
    inactive_uses: BTreeSet<SyntaxKey>,
}

impl TargetSelection {
    pub(crate) fn prepare(
        input: &CompileUnitInput<'_>,
        modules: &[&ModuleInput<'_>],
    ) -> Result<Self, LoweringError> {
        let mut selection = Self {
            inactive_items: BTreeSet::new(),
            inactive_uses: BTreeSet::new(),
        };
        for module in modules {
            for source in module.sources() {
                selection.collect_tree(input, source.syntax())?;
            }
        }
        Ok(selection)
    }

    #[must_use]
    pub(crate) fn item_is_active(&self, item: NodeId) -> bool {
        !self.inactive_items.contains(&key(item))
    }

    #[must_use]
    pub(crate) fn use_is_inactive(&self, declaration: NodeId) -> bool {
        self.inactive_uses.contains(&key(declaration))
    }

    fn collect_tree(
        &mut self,
        input: &CompileUnitInput<'_>,
        tree: &SyntaxTree,
    ) -> Result<(), LoweringError> {
        if tree.has_errors() || tree.root().kind() != NodeKind::ModuleSource {
            return Ok(());
        }
        let source = input
            .sources()
            .get(tree.source())
            .ok_or(LoweringError::MissingSource(tree.source()))?;
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
                .ok_or(LoweringError::InconsistentSyntax(tree.source()))?;
            let name = nocter_syntax::decode_string_literal(source, tree, literal)
                .ok_or(LoweringError::InconsistentSyntax(tree.source()))?;
            let target = CompilationTarget::from_name(&name)
                .ok_or(LoweringError::UnknownTargetGate(literal))?;
            if target != input.target() {
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
    ) -> Result<(), LoweringError> {
        let mut pending = vec![item];
        while let Some(node) = pending.pop() {
            let kind = tree
                .node(node)
                .ok_or(LoweringError::InconsistentSyntax(tree.source()))?
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
