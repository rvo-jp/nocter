//! Immutable, storage-independent literal DAGs. Types and physical layout belong to consumers.
use crate::ConstantValue;
use std::num::NonZeroU64;
use std::sync::Arc;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum FrozenNode {
    Unit,
    Scalar(ConstantValue),
    /// One selected branch and its optional stored payload; the consuming type owns tag meaning.
    Variant {
        primary: bool,
        payload: Option<usize>,
    },
    /// Declaration/index-order component indexes into the same graph, all preceding this node.
    Aggregate(Box<[usize]>),
    /// Homogeneous logical storage, without one graph edge per repeated element. Sparse
    /// replacements are ordered, unique and in range; all value edges precede this node.
    Repeated {
        element: usize,
        length: NonZeroU64,
        replacements: Box<[(u64, usize)]>,
    },
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
        if nodes.iter().enumerate().any(|(id, node)| match node {
            FrozenNode::Aggregate(fields) => fields.iter().any(|&field| field >= id),
            FrozenNode::Variant { payload, .. } => payload.is_some_and(|payload| payload >= id),
            FrozenNode::Repeated {
                element,
                length,
                replacements,
            } => {
                *element >= id
                    || replacements
                        .iter()
                        .any(|&(index, value)| index >= length.get() || value >= id)
                    || replacements.windows(2).any(|pair| pair[0].0 >= pair[1].0)
            }
            FrozenNode::Unit | FrozenNode::Scalar(_) => false,
        }) {
            return Err(InvalidFrozenData);
        }
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
        assert!(FrozenData::new(vec![FrozenNode::Aggregate(vec![0].into())]).is_err());
        assert!(
            FrozenData::new(vec![
                FrozenNode::Aggregate(vec![1].into()),
                FrozenNode::Unit
            ])
            .is_err()
        );
        let graph = FrozenData::new(vec![
            FrozenNode::Unit,
            FrozenNode::Aggregate(vec![0, 0].into()),
        ])
        .unwrap();
        let component = graph.at(0).unwrap();
        assert!(Arc::ptr_eq(&graph.nodes, &component.nodes));
        assert_eq!(component.node(), &FrozenNode::Unit);
        assert!(graph.at(2).is_none());
    }
    #[test]
    fn repeated_edges_and_sparse_positions_validate_without_count_expansion() {
        let length = NonZeroU64::new(u64::MAX).unwrap();
        let make = |element, replacements| {
            vec![
                FrozenNode::Unit,
                FrozenNode::Repeated {
                    element,
                    length,
                    replacements,
                },
            ]
        };
        for nodes in [
            make(1, Box::new([])),
            make(0, vec![(0, 1)].into()),
            make(0, vec![(u64::MAX, 0)].into()),
            make(0, vec![(1, 0), (1, 0)].into()),
            make(0, vec![(2, 0), (1, 0)].into()),
        ] {
            assert!(FrozenData::new(nodes).is_err());
        }
        let value = FrozenData::new(make(0, vec![(0, 0), (u64::MAX - 1, 0)].into())).unwrap();
        assert_eq!(value.nodes().len(), 2);
        assert_eq!(value.at(0).unwrap().node(), &FrozenNode::Unit);
    }
}

#[cfg(test)]
mod variant_tests {
    use super::*;
    #[test]
    fn variant_edges_are_validated_even_when_not_selected_as_the_final_root() {
        assert!(
            FrozenData::new(vec![FrozenNode::Variant {
                primary: true,
                payload: Some(0)
            }])
            .is_err()
        );
        assert!(
            FrozenData::new(vec![
                FrozenNode::Variant {
                    primary: false,
                    payload: Some(1)
                },
                FrozenNode::Unit
            ])
            .is_err()
        );
        assert!(
            FrozenData::new(vec![FrozenNode::Variant {
                primary: false,
                payload: None
            }])
            .is_ok()
        );
        assert!(
            FrozenData::new(vec![
                FrozenNode::Unit,
                FrozenNode::Variant {
                    primary: true,
                    payload: Some(0)
                }
            ])
            .is_ok()
        );
    }
}
