//! Sparse source writes and actual operand reads, independent of ownership and resource phases.
use super::{State, ValueSourceId, ValueSourceSlot};
use crate::value_access::flow;
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
pub struct ValueSourceFlow {
    pub(super) entry: AccessFlowPointId,
    pub(super) inputs: State,
    pub(super) points: BTreeMap<AccessFlowPointId, usize>,
    pub(super) predecessors: BTreeMap<AccessFlowPointId, Vec<AccessFlowPointId>>,
    pub(super) writes: BTreeMap<(AccessFlowPointId, ValueSourceSlot), Vec<(usize, ValueSourceId)>>,
    pub(super) reads: BTreeMap<(flow::Location, AccessExpressionId), ValueSourceId>,
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
    #[must_use]
    pub fn write_count(&self) -> usize {
        self.writes.values().map(Vec::len).sum()
    }
}
