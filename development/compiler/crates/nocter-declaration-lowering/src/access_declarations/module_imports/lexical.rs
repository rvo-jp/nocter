//! Sparse current-source projection of import-bearing blocks onto the shared lexical namespace.
use super::{ImportBindings, ImportSite, ModuleSourcesFailure, ScopeId, integrity, syntax};
use nocter_declarations::value_access::Reservations;
use nocter_source::{SourceId, TextRange};
use nocter_syntax::{BoundSyntax, NodeId, NodeKind, SyntaxOrigin, SyntaxToken, child_node_iter};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default)]
pub(in crate::access_declarations) struct LexicalScopes {
    sources: BTreeMap<SourceId, Vec<BlockScope>>,
}
#[derive(Debug)]
struct BlockScope {
    node: NodeId,
    range: TextRange,
    scope: ScopeId,
    parent: Option<usize>,
    names: Box<[SyntaxToken]>,
}
impl LexicalScopes {
    pub fn at(&self, source: SourceId, range: TextRange) -> Option<ScopeId> {
        let blocks = self.sources.get(&source)?;
        let mut candidate = blocks
            .partition_point(|block| block.range.start() <= range.start())
            .checked_sub(1);
        while let Some(index) = candidate {
            let block = &blocks[index];
            if block.range.contains_range(range) {
                return Some(block.scope);
            }
            candidate = block.parent;
        }
        None
    }
    pub fn names(&self, syntax: BoundSyntax<'_>, node: NodeId) -> &[SyntaxToken] {
        let Some(blocks) = self.sources.get(&syntax.tree().source()) else {
            return &[];
        };
        let Some(view) = syntax.tree().node(node) else {
            return &[];
        };
        let index = blocks.partition_point(|block| block.range.start() < view.range().start());
        blocks
            .get(index)
            .filter(|block| block.node == node)
            .map_or(&[], |block| &block.names)
    }
}
impl ImportBindings<'_, '_> {
    pub(super) fn bind_blocks(
        &mut self,
        reservations: &mut Reservations,
    ) -> Result<LexicalScopes, ModuleSourcesFailure> {
        let paired = self.paired;
        let resolutions = paired
            .inventory
            .block_imports()
            .iter()
            .map(|import| (import.node(), import))
            .collect::<BTreeMap<_, _>>();
        let mut visited = BTreeSet::new();
        let mut lexical = LexicalScopes::default();
        for (source, _) in paired.inventory.source_entries() {
            let syntax = syntax(paired, source)?;
            let mut blocks = Vec::<BlockScope>::new();
            let mut pending = vec![(syntax.tree().root_id(), None::<usize>)];
            while let Some((node, mut parent)) = pending.pop() {
                let imports = child_node_iter(syntax.tree(), node)
                    .filter_map(|node| resolutions.get(&node).copied())
                    .collect::<Vec<_>>();
                if !imports.is_empty() {
                    if syntax.tree().node(node).ok_or_else(integrity)?.kind() != NodeKind::Block {
                        return Err(integrity());
                    }
                    let inherited = parent.map_or(self.source_scopes[source.index()], |index| {
                        blocks[index].scope
                    });
                    let scope = reservations
                        .new_lexical_scope(inherited)
                        .map_err(|_| integrity())?;
                    for import in imports {
                        if !visited.insert(import.node()) {
                            return Err(integrity());
                        }
                        self.bind(
                            ImportSite {
                                source,
                                scope,
                                node: import.node(),
                                target: import.target(),
                            },
                            reservations,
                        )?;
                    }
                    let names = self
                        .locals
                        .get(&scope)
                        .ok_or_else(integrity)?
                        .values()
                        .map(|origin| {
                            let SyntaxOrigin::Token(token) = origin else {
                                unreachable!("import local origins are tokens")
                            };
                            *token
                        })
                        .collect();
                    let index = blocks.len();
                    blocks.push(BlockScope {
                        node,
                        range: syntax.tree().node(node).ok_or_else(integrity)?.range(),
                        scope,
                        parent,
                        names,
                    });
                    parent = Some(index);
                }
                let children = child_node_iter(syntax.tree(), node).collect::<Vec<_>>();
                pending.extend(children.into_iter().rev().map(|node| (node, parent)));
            }
            if !blocks.is_empty() {
                lexical.sources.insert(syntax.tree().source(), blocks);
            }
        }
        if visited.len() != resolutions.len() {
            return Err(integrity());
        }
        Ok(lexical)
    }
}
