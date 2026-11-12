//! One syntax-owned selection authority for source items gated by `#target`.

use std::collections::{BTreeMap, BTreeSet};

use nocter_model::CompilationTarget;
use nocter_source::{SourceId, SourceMap};
use nocter_syntax::{
    NodeId, NodeKind, ParseGoal, SyntaxTree, child_node_iter, direct_node, node_is_complete,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetSelectionError {
    MissingSource(SourceId),
    InconsistentSyntax(SourceId),
    InconsistentSnapshot,
    UnknownTarget(NodeId),
}

/// The immutable item/use activity decision shared by discovery and semantic lowering.
#[derive(Clone, Debug)]
pub struct TargetSelection {
    source_goal: ParseGoal,
    target: CompilationTarget,
    syntax_roots: Vec<NodeId>,
    item_targets: BTreeMap<NodeId, CompilationTarget>,
    inactive_items: BTreeSet<NodeId>,
    inactive_uses: BTreeSet<NodeId>,
    authored_error: Option<TargetSelectionError>,
}

/// Sole incremental construction authority used while discovery closes a package graph.
#[derive(Debug)]
pub struct TargetSelectionBuilder {
    selection: TargetSelection,
}

impl TargetSelection {
    /// Selects all supplied syntax trees for one closed compilation target.
    ///
    /// # Errors
    ///
    /// Returns an error when source storage is inconsistent with a tree. An authored unknown
    /// target is retained by the completed selection so discovery can safely deactivate its item
    /// before declaration lowering emits the source diagnostic.
    pub fn prepare<'tree>(
        target: CompilationTarget,
        sources: &SourceMap,
        trees: impl IntoIterator<Item = &'tree SyntaxTree>,
    ) -> Result<Self, TargetSelectionError> {
        Self::prepare_for_goal(ParseGoal::SourceFile, target, sources, trees)
    }

    /// Selects one explicitly chosen source grammar, without accepting another grammar as fallback.
    /// # Errors
    /// Rejects inconsistent snapshots and trees from a different grammar.
    pub fn prepare_for_goal<'tree>(
        source_goal: ParseGoal,
        target: CompilationTarget,
        sources: &SourceMap,
        trees: impl IntoIterator<Item = &'tree SyntaxTree>,
    ) -> Result<Self, TargetSelectionError> {
        let mut builder = TargetSelectionBuilder::for_goal(source_goal, target);
        for tree in trees {
            builder.include_tree(sources, tree)?;
        }
        Ok(builder.finish())
    }

    #[must_use]
    pub const fn source_goal(&self) -> ParseGoal {
        self.source_goal
    }

    /// Reports whether this decision belongs to the exact target, source map, and syntax snapshots
    /// supplied by one compile input.
    #[must_use]
    pub fn matches<'tree>(
        &self,
        target: CompilationTarget,
        sources: &SourceMap,
        trees: impl IntoIterator<Item = &'tree SyntaxTree>,
    ) -> bool {
        let mut roots = Vec::new();
        for tree in trees {
            if tree.root().kind() != self.source_goal.root_kind() {
                return false;
            }
            let Some(source) = sources.get(tree.source()) else {
                return false;
            };
            if nocter_syntax::BoundSyntax::new(source, tree).is_none() {
                return false;
            }
            roots.push(tree.root_id());
        }
        roots.sort_unstable();
        self.target == target && self.syntax_roots == roots
    }

    #[must_use]
    pub fn item_is_active(&self, item: NodeId) -> bool {
        self.contains_node(item) && !self.inactive_items.contains(&item)
    }

    /// Returns the normalized target named by one gated item.
    #[must_use]
    pub fn item_target(&self, item: NodeId) -> Option<CompilationTarget> {
        self.item_targets.get(&item).copied()
    }

    #[must_use]
    pub fn use_is_active(&self, declaration: NodeId) -> bool {
        self.contains_node(declaration) && !self.inactive_uses.contains(&declaration)
    }

    /// Returns the first source-authored target error retained by this selection.
    ///
    /// Selection remains usable for dependency discovery when such an error exists: the invalid
    /// gated item is inactive, and declaration lowering later projects this retained error through
    /// the ordinary source-diagnostic boundary.
    #[must_use]
    pub const fn authored_error(&self) -> Option<TargetSelectionError> {
        self.authored_error
    }

    fn contains_node(&self, node: NodeId) -> bool {
        self.syntax_roots
            .iter()
            .any(|root| root.shares_tree_with(node))
    }

    fn collect_tree(
        &mut self,
        sources: &SourceMap,
        tree: &SyntaxTree,
    ) -> Result<(), TargetSelectionError> {
        if self
            .syntax_roots
            .iter()
            .any(|root| root.source() == tree.source())
        {
            return Err(TargetSelectionError::InconsistentSnapshot);
        }
        if tree.root().kind() != self.source_goal.root_kind() {
            return Err(TargetSelectionError::InconsistentSyntax(tree.source()));
        }
        let source = sources
            .get(tree.source())
            .ok_or(TargetSelectionError::MissingSource(tree.source()))?;
        let syntax = nocter_syntax::BoundSyntax::new(source, tree)
            .ok_or(TargetSelectionError::InconsistentSyntax(tree.source()))?;
        self.syntax_roots.push(tree.root_id());
        for child in child_node_iter(tree, tree.root_id()) {
            if tree
                .node(child)
                .is_none_or(|node| node.kind() != NodeKind::Item)
            {
                continue;
            }
            let Some(gate) = direct_node(tree, child, NodeKind::TargetDirective) else {
                continue;
            };
            if !node_is_complete(tree, gate) {
                self.deactivate_item(tree, child)?;
                continue;
            }
            let literal = descendant(tree, gate, NodeKind::StringLiteral)
                .ok_or(TargetSelectionError::InconsistentSyntax(tree.source()))?;
            let name = nocter_syntax::decode_string_literal(syntax, literal)
                .ok_or(TargetSelectionError::InconsistentSyntax(tree.source()))?;
            let Some(selected) = CompilationTarget::from_name(&name) else {
                if self.authored_error.is_none() {
                    self.authored_error = Some(TargetSelectionError::UnknownTarget(literal));
                }
                self.deactivate_item(tree, child)?;
                continue;
            };
            self.item_targets.insert(child, selected);
            if selected != self.target {
                self.deactivate_item(tree, child)?;
            }
        }
        Ok(())
    }

    fn deactivate_item(
        &mut self,
        tree: &SyntaxTree,
        item: NodeId,
    ) -> Result<(), TargetSelectionError> {
        self.inactive_items.insert(item);
        self.collect_inactive_uses(tree, item)
    }

    fn collect_inactive_uses(
        &mut self,
        tree: &SyntaxTree,
        item: NodeId,
    ) -> Result<(), TargetSelectionError> {
        for node in std::iter::once(item).chain(nocter_syntax::descendant_node_iter(tree, item)) {
            let kind = tree
                .node(node)
                .ok_or(TargetSelectionError::InconsistentSyntax(tree.source()))?
                .kind();
            if matches!(
                kind,
                NodeKind::UseDeclaration | NodeKind::BlockUseDeclaration
            ) {
                self.inactive_uses.insert(node);
            }
        }
        Ok(())
    }
}

impl TargetSelectionBuilder {
    #[must_use]
    pub fn new(target: CompilationTarget) -> Self {
        Self::for_goal(ParseGoal::SourceFile, target)
    }

    #[must_use]
    pub fn for_goal(source_goal: ParseGoal, target: CompilationTarget) -> Self {
        Self {
            selection: TargetSelection {
                source_goal,
                target,
                syntax_roots: Vec::new(),
                item_targets: BTreeMap::new(),
                inactive_items: BTreeSet::new(),
                inactive_uses: BTreeSet::new(),
                authored_error: None,
            },
        }
    }

    /// Extends this authority with one newly discovered syntax tree.
    ///
    /// # Errors
    ///
    /// Returns an error when the source snapshot is inconsistent. Authored target errors remain
    /// inside the selection as inactive-item evidence.
    pub fn include_tree(
        &mut self,
        sources: &SourceMap,
        tree: &SyntaxTree,
    ) -> Result<(), TargetSelectionError> {
        self.selection.collect_tree(sources, tree)
    }

    /// Borrows the decisions completed so far for dependency traversal.
    #[must_use]
    pub const fn selection(&self) -> &TargetSelection {
        &self.selection
    }

    #[must_use]
    pub fn finish(mut self) -> TargetSelection {
        self.selection.syntax_roots.sort_unstable();
        self.selection
    }
}

fn descendant(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    std::iter::once(node)
        .chain(nocter_syntax::descendant_node_iter(tree, node))
        .find(|node| tree.node(*node).is_some_and(|node| node.kind() == kind))
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
                b"#target: \"x64-linux\"\nfunc inactive(): void {\n    use std/io\n    return\n}\nfunc active(): void {\n    use std/io\n    return\n}\n",
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

    #[test]
    fn selects_complete_target_gates_beside_an_incomplete_body() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("gated.nct"),
                b"#target: \"x64-linux\"\nfunc inactive(): void {\n    use ./missing\n    return\n}\nfunc active(): void {\n    use ./available\n    let value =\n}\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        assert!(tree.has_errors());

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
        assert!(!selection.item_is_active(items[0]));
        assert!(!selection.use_is_active(uses[0]));
        assert!(selection.item_is_active(items[1]));
        assert!(selection.use_is_active(uses[1]));
    }

    #[test]
    fn incomplete_target_gate_never_activates_its_item_imports() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("gated.nct"),
                b"#target:\nfunc unavailable(): void {\n    use ./missing\n    return\n}\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        assert!(tree.has_errors());

        let uses = descendants_of_kind(&tree, NodeKind::BlockUseDeclaration);
        let items = descendants_of_kind(&tree, NodeKind::Item);
        let selection =
            TargetSelection::prepare(CompilationTarget::Arm64Darwin, &sources, [&tree]).unwrap();

        assert_eq!(uses.len(), 1);
        assert_eq!(items.len(), 1);
        assert!(!selection.item_is_active(items[0]));
        assert!(!selection.use_is_active(uses[0]));
    }

    #[test]
    fn unknown_target_beside_incomplete_syntax_cannot_preempt_syntax_recovery() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("gated.nct"),
                b"#target: \"mips-templeos\"\nfunc unavailable(): void {\n    use ./missing\n    return\n}\nfunc broken(): void {\n    let value =\n}\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        assert!(tree.has_errors());

        let uses = descendants_of_kind(&tree, NodeKind::BlockUseDeclaration);
        let items = descendants_of_kind(&tree, NodeKind::Item);
        let selection =
            TargetSelection::prepare(CompilationTarget::Arm64Darwin, &sources, [&tree]).unwrap();

        assert_eq!(uses.len(), 1);
        assert_eq!(items.len(), 2);
        assert!(matches!(
            selection.authored_error(),
            Some(TargetSelectionError::UnknownTarget(_))
        ));
        assert!(!selection.item_is_active(items[0]));
        assert!(!selection.use_is_active(uses[0]));
        assert!(selection.item_is_active(items[1]));
    }

    #[test]
    fn unknown_target_is_retained_beside_a_safe_inactive_selection() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("gated.nct"),
                b"#target: \"mips-templeos\"\nfunc unavailable(): void { return }\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        assert!(!tree.has_errors());

        let items = descendants_of_kind(&tree, NodeKind::Item);
        let selection =
            TargetSelection::prepare(CompilationTarget::Arm64Darwin, &sources, [&tree]).unwrap();

        assert_eq!(items.len(), 1);
        assert!(matches!(
            selection.authored_error(),
            Some(TargetSelectionError::UnknownTarget(_))
        ));
        assert!(!selection.item_is_active(items[0]));
    }

    #[test]
    fn rejects_a_tree_from_an_independent_source_map_with_the_same_index() {
        let mut first = SourceMap::new();
        let first_id = first
            .add_bytes(
                SourceName::new("first.nct"),
                b"#target: \"x64-linux\"\nfunc value(): void { return }\n",
            )
            .unwrap();
        let tree = parse(first.get(first_id).unwrap(), ParseGoal::SourceFile);
        let mut second = SourceMap::new();
        let second_id = second
            .add_bytes(
                SourceName::new("second.nct"),
                b"#target: \"x64-linux\"\nfunc value(): void { return }\n",
            )
            .unwrap();

        assert_eq!(first_id.index(), second_id.index());
        assert!(matches!(
            TargetSelection::prepare(CompilationTarget::Arm64Darwin, &second, [&tree]),
            Err(TargetSelectionError::MissingSource(source)) if source == first_id
        ));
    }

    #[test]
    fn selection_records_its_target_and_exact_syntax_snapshots() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("source.nct"),
                b"func value(): void { return }\n",
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        let selection =
            TargetSelection::prepare(CompilationTarget::Arm64Darwin, &sources, [&tree]).unwrap();

        assert!(selection.matches(CompilationTarget::Arm64Darwin, &sources, [&tree]));
        assert!(!selection.matches(CompilationTarget::X64Linux, &sources, [&tree]));
        assert!(!selection.matches(CompilationTarget::Arm64Darwin, &sources, []));
        let reparsed = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        assert!(!selection.matches(CompilationTarget::Arm64Darwin, &sources, [&reparsed]));

        let mut foreign_sources = SourceMap::new();
        foreign_sources
            .add_bytes(
                SourceName::new("source.nct"),
                b"func value(): void { return }\n",
            )
            .unwrap();
        assert!(!selection.matches(CompilationTarget::Arm64Darwin, &foreign_sources, [&tree]));
    }

    #[test]
    fn activity_from_one_parse_cannot_apply_to_a_reparsed_tree() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("source.nct"),
                b"#target: \"x64-linux\"\nfunc value(): void { return }\n",
            )
            .unwrap();
        let first = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        let second = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        let selection =
            TargetSelection::prepare(CompilationTarget::Arm64Darwin, &sources, [&first]).unwrap();
        let first_item = descendants_of_kind(&first, NodeKind::Item)[0];
        let second_item = descendants_of_kind(&second, NodeKind::Item)[0];

        assert!(!selection.item_is_active(first_item));
        assert!(!selection.item_is_active(second_item));
    }

    #[test]
    fn future_private_unit_cannot_enter_released_compilation_as_an_empty_source() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("core.nct"), b"func f(): void {}\n")
            .unwrap();
        let future = parse(sources.get(source).unwrap(), ParseGoal::V2CoreUnit);
        assert!(!future.has_errors());
        assert!(matches!(
            TargetSelection::prepare(CompilationTarget::Arm64Darwin, &sources, [&future]),
            Err(TargetSelectionError::InconsistentSyntax(id)) if id == source
        ));
        let mut builder = TargetSelectionBuilder::new(CompilationTarget::Arm64Darwin);
        assert!(matches!(
            builder.include_tree(&sources, &future),
            Err(TargetSelectionError::InconsistentSyntax(_))
        ));
        let current = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        builder.include_tree(&sources, &current).unwrap();
        let selection = builder.finish();
        assert!(selection.matches(CompilationTarget::Arm64Darwin, &sources, [&current]));
        assert!(!selection.matches(CompilationTarget::Arm64Darwin, &sources, [&future]));
        assert!(selection.item_is_active(descendants_of_kind(&current, NodeKind::Item)[0]));
    }

    #[test]
    fn explicit_goal_selection_never_mixes_grammars_or_admits_a_rejected_tree() {
        let mut sources = SourceMap::new();
        let future_id = sources
            .add_bytes(SourceName::new("future.nct"), b"func future(): void {}")
            .unwrap();
        let current_id = sources
            .add_bytes(SourceName::new("current.nct"), b"func current(): void {}")
            .unwrap();
        let future = parse(sources.get(future_id).unwrap(), ParseGoal::V2CoreUnit);
        let current = parse(sources.get(current_id).unwrap(), ParseGoal::SourceFile);
        let mut builder =
            TargetSelectionBuilder::for_goal(ParseGoal::V2CoreUnit, CompilationTarget::Arm64Darwin);
        builder.include_tree(&sources, &future).unwrap();
        assert_eq!(builder.selection().source_goal(), ParseGoal::V2CoreUnit);
        assert!(
            matches!(builder.include_tree(&sources, &current), Err(TargetSelectionError::InconsistentSyntax(id)) if id == current_id)
        );
        let selected = builder.finish();
        assert!(selected.matches(CompilationTarget::Arm64Darwin, &sources, [&future]));
        assert!(!selected.matches(
            CompilationTarget::Arm64Darwin,
            &sources,
            [&future, &current]
        ));
        assert!(!selected.item_is_active(descendants_of_kind(&current, NodeKind::Item)[0]));
        assert!(selected.item_is_active(descendants_of_kind(&future, NodeKind::Item)[0]));
        assert!(matches!(
            TargetSelection::prepare_for_goal(
                ParseGoal::V2CoreUnit,
                CompilationTarget::Arm64Darwin,
                &sources,
                [&future, &current]
            ),
            Err(TargetSelectionError::InconsistentSyntax(_))
        ));
    }

    #[test]
    fn one_selection_cannot_admit_two_trees_for_the_same_source() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("source.nct"),
                b"func value(): void { return }\n",
            )
            .unwrap();
        let file = sources.get(source).unwrap();
        let first = parse(file, ParseGoal::SourceFile);
        let second = parse(file, ParseGoal::SourceFile);

        assert!(matches!(
            TargetSelection::prepare(CompilationTarget::Arm64Darwin, &sources, [&first, &second]),
            Err(TargetSelectionError::InconsistentSnapshot)
        ));
    }

    fn descendants_of_kind(tree: &SyntaxTree, kind: NodeKind) -> Vec<NodeId> {
        std::iter::once(tree.root_id())
            .chain(nocter_syntax::descendant_node_iter(tree, tree.root_id()))
            .filter(|node| tree.node(*node).is_some_and(|node| node.kind() == kind))
            .collect()
    }
}
