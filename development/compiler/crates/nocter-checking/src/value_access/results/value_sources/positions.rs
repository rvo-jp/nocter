//! Sparse source writes and actual operand reads, independent of ownership and resource phases.
use super::{State, ValueSourceId, ValueSourceSlot};
use crate::value_access::{Referent, flow};
use nocter_model::{AccessExpressionId, AccessFlowPointId};
use std::collections::BTreeMap;

/// A boundary after exactly `action` original actions in one original control point.
/// This describes source-definition order, not a resource-transition phase or an access grant.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ValueSourceBoundary {
    pub point: AccessFlowPointId,
    pub action: usize,
}
/// A selected stored source at one original action boundary, independent of resource timing.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ValueSourceSelection {
    pub boundary: ValueSourceBoundary,
    pub slot: ValueSourceSlot,
    pub path: nocter_model::ValuePath<()>,
}
/// A local reaching definition or the original predecessor boundaries still to be joined.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueSourceRead {
    Empty,
    Value(ValueSourceId),
    Incoming(Box<[ValueSourceBoundary]>),
}
#[derive(Clone, Debug)]
pub(super) struct RootSources {
    pub(super) before: ValueSourceId,
    pub(super) after: Option<ValueSourceId>,
}
#[derive(Clone, Debug)]
pub struct ValueSourceFlow {
    pub(super) entry: AccessFlowPointId,
    pub(super) inputs: State,
    pub(super) points: BTreeMap<AccessFlowPointId, usize>,
    pub(super) predecessors: BTreeMap<AccessFlowPointId, Vec<AccessFlowPointId>>,
    pub(super) writes: BTreeMap<(AccessFlowPointId, ValueSourceSlot), Vec<(usize, ValueSourceId)>>,
    pub(super) reads: BTreeMap<(flow::Location, AccessExpressionId), ValueSourceId>,
    pub(super) referents: BTreeMap<(flow::Location, Referent), ValueSourceId>,
    pub(super) roots: BTreeMap<(flow::Location, Referent), RootSources>,
}
impl ValueSourceFlow {
    #[cfg(test)]
    pub(in crate::value_access::results) fn test(
        entry: AccessFlowPointId,
        points: BTreeMap<AccessFlowPointId, usize>,
        inputs: State,
        predecessors: BTreeMap<AccessFlowPointId, Vec<AccessFlowPointId>>,
        writes: BTreeMap<(AccessFlowPointId, ValueSourceSlot), Vec<(usize, ValueSourceId)>>,
    ) -> Self {
        Self {
            entry,
            points,
            inputs,
            predecessors,
            writes,
            reads: BTreeMap::new(),
            referents: BTreeMap::new(),
            roots: BTreeMap::new(),
        }
    }
    /// Select a sparse reaching definition without allocating a point-by-slot state matrix.
    /// Missing/uninitialized inputs supply Empty, never an availability or runtime-value proof.
    #[must_use]
    pub fn read(
        &self,
        boundary: ValueSourceBoundary,
        slot: ValueSourceSlot,
    ) -> Option<ValueSourceRead> {
        let length = *self.points.get(&boundary.point)?;
        if boundary.action > length {
            return None;
        }
        if let Some(writes) = self.writes.get(&(boundary.point, slot)) {
            let end = writes.partition_point(|&(action, _)| action < boundary.action);
            if let Some(&(_, value)) = writes[..end].last() {
                return Some(ValueSourceRead::Value(value));
            }
        }
        if boundary.point == self.entry {
            return Some(
                self.inputs
                    .get(&slot)
                    .copied()
                    .map_or(ValueSourceRead::Empty, ValueSourceRead::Value),
            );
        }
        let mut incoming = Vec::new();
        for &point in self.predecessors.get(&boundary.point).into_iter().flatten() {
            incoming.push(ValueSourceBoundary {
                point,
                action: *self.points.get(&point)?,
            });
        }
        Some(if incoming.is_empty() {
            ValueSourceRead::Empty
        } else {
            ValueSourceRead::Incoming(incoming.into())
        })
    }
    /// The actual operand source read by this action, including transparent receiving adapters.
    /// A stored alias slot is not a substitute for the alias authority's selected operand source.
    #[must_use]
    pub fn operand(
        &self,
        location: flow::Location,
        expression: AccessExpressionId,
    ) -> Option<ValueSourceId> {
        self.reads.get(&(location, expression)).copied()
    }
    /// The previous selected value at an original write target, resolving lexical aliases.
    /// Recording an uninitialized target does not make its previous value available.
    #[must_use]
    pub fn referent(&self, location: flow::Location, target: Referent) -> Option<ValueSourceId> {
        self.referents.get(&(location, target)).copied()
    }
    /// The current referent root before its authored place-field suffix, through lexical aliases.
    /// For an evaluated referent the complete selected value is its root. This is source metadata,
    /// not the enclosing physical allocation, an initialized owner or a resource-phase receipt.
    #[must_use]
    pub fn root(&self, location: flow::Location, target: Referent) -> Option<ValueSourceId> {
        self.roots
            .get(&(location, target))
            .map(|roots| roots.before)
    }
    /// Reference-root source on normal completion of this original write/call. This proves neither
    /// a mutation nor that execution reaches that boundary, and is not a resource-phase receipt.
    #[must_use]
    pub fn root_after(&self, location: flow::Location, target: Referent) -> Option<ValueSourceId> {
        self.roots.get(&(location, target))?.after
    }
    /// Only roots selected by original referent operations, without a place-by-action matrix.
    pub fn roots(&self) -> impl Iterator<Item = ((flow::Location, Referent), ValueSourceId)> + '_ {
        self.roots.iter().map(|(&site, roots)| (site, roots.before))
    }
    /// Original operand occurrences, without another traversal of source operation kinds.
    pub fn operands(
        &self,
    ) -> impl Iterator<Item = ((flow::Location, AccessExpressionId), ValueSourceId)> + '_ {
        self.reads.iter().map(|(&site, &source)| (site, source))
    }
    /// The final stored definition of each slot written by an original action.
    /// Intermediate writes in one action are not a second source boundary or resource phase.
    pub fn definitions(
        &self,
    ) -> impl Iterator<Item = ((flow::Location, ValueSourceSlot), ValueSourceId)> + '_ {
        self.writes.iter().flat_map(|(&(point, slot), writes)| {
            writes
                .iter()
                .enumerate()
                .filter_map(move |(index, &(action, value))| {
                    writes
                        .get(index + 1)
                        .is_none_or(|&(next, _)| next != action)
                        .then_some(((flow::Location { point, action }, slot), value))
                })
        })
    }
    #[must_use]
    pub fn write_count(&self) -> usize {
        self.writes.values().map(Vec::len).sum()
    }
}
