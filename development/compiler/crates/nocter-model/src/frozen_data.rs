//! Immutable, storage-independent literal DAGs. Types and physical layout belong to consumers.
use crate::ConstantValue;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum FrozenNode {
    Unit,
    Scalar(ConstantValue),
    /// Declaration-order component indexes into the same graph, all preceding this node.
    Record(Box<[usize]>),
}
/// A validated acyclic graph with a selected root. Clones and component views share its data.
/// Contains neither evaluator memory, semantic type IDs nor source/storage identities.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FrozenData {
    nodes: Arc<[FrozenNode]>,
    root: usize,
}
impl FrozenData {
    /// # Errors
    /// Rejects an empty graph or forward, cyclic or missing component edges.
    pub fn new(nodes: impl Into<Arc<[FrozenNode]>>) -> Result<Self, InvalidFrozenData> {
        let nodes = nodes.into();
        let root = nodes.len().checked_sub(1).ok_or(InvalidFrozenData)?;
        if nodes.iter().enumerate().any(|(id, node)| {
            matches!(node, FrozenNode::Record(fields) if fields.iter().any(|&field| field >= id))
        }) { return Err(InvalidFrozenData); }
        Ok(Self { nodes, root })
    }
    #[must_use]
    pub fn nodes(&self) -> &[FrozenNode] {
        &self.nodes
    }
    #[must_use]
    pub const fn root(&self) -> usize {
        self.root
    }
    #[must_use]
    pub fn node(&self) -> &FrozenNode {
        &self.nodes[self.root]
    }
    /// Select a component without copying its data or manufacturing another graph authority.
    #[must_use]
    pub fn at(&self, root: usize) -> Option<Self> {
        self.nodes.get(root)?;
        Some(Self {
            nodes: self.nodes.clone(),
            root,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidFrozenData;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn graph_closure_rejects_missing_forward_and_cyclic_edges() {
        assert!(FrozenData::new(Vec::new()).is_err());
        assert!(FrozenData::new(vec![FrozenNode::Record(vec![0].into())]).is_err());
        assert!(
            FrozenData::new(vec![FrozenNode::Record(vec![1].into()), FrozenNode::Unit]).is_err()
        );
        let graph = FrozenData::new(vec![
            FrozenNode::Unit,
            FrozenNode::Record(vec![0, 0].into()),
        ])
        .unwrap();
        let component = graph.at(0).unwrap();
        assert!(Arc::ptr_eq(&graph.nodes, &component.nodes));
        assert_eq!(component.node(), &FrozenNode::Unit);
        assert!(graph.at(2).is_none());
    }
}
