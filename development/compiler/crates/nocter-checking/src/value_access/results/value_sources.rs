//! Sparse value-source SSA over the original checked control graph; no access or execution proof.
use crate::value_access::{Query, flow, ir::LocalOrigin};
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{
    AccessExpressionId, AccessFlowPointId, AccessLocalId, TypeId, ValueApplication, ValueClosure,
    ValueCode, ValueField, ValuePath,
};
use std::collections::{BTreeMap, BTreeSet};
mod build;
mod positions;
pub use positions::{
    ValueSourceBoundary, ValueSourceCarrier, ValueSourceFlow, ValueSourceRead, ValueSourceSelection,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ValueSourceId(usize);
#[cfg(test)]
impl ValueSourceId {
    pub(in crate::value_access::results) const fn test(index: usize) -> Self {
        Self(index)
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ValueSourceSlot {
    Local(AccessLocalId),
    Expression(AccessExpressionId),
    Return,
    Argument {
        call: AccessExpressionId,
        index: usize,
    },
}
/// Original value formation, independent of owner availability and physical layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueSource {
    /// No source value has reached this edge. This is not an initialization/bounds proof.
    Empty,
    Input {
        local: AccessLocalId,
        origin: LocalOrigin,
    },
    Merge {
        point: AccessFlowPointId,
        slot: ValueSourceSlot,
        inputs: BTreeSet<ValueSourceId>,
    },
    Select {
        source: ValueSourceId,
        path: ValuePath<()>,
    },
    Alternatives(BTreeSet<ValueSourceId>),
    Function(FunctionWitness),
    Closure {
        closure: ValueClosure,
        environment: ValueSourceId,
    },
    Aggregate(Box<[(ValueField, ValueSourceId)]>),
    Repeat {
        seed: ValueSourceId,
    },
    Adapt {
        source: ValueSourceId,
    },
    Index {
        source: ValueSourceId,
        index: ValueSourceId,
    },
    CallResult(ValueApplication),
    Replace {
        source: ValueSourceId,
        value: ValueSourceId,
        path: ValuePath<()>,
        weak: bool,
    },
    /// A selected edit may change this root. No unchanged-context or callee effect proof is implied.
    CallWrite {
        call: AccessExpressionId,
        argument: usize,
        source: ValueSourceId,
        path: ValuePath<()>,
    },
    /// Scalar operations and context effects not interpreted by this metadata graph.
    Opaque,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueSourceNode {
    pub origin: Option<flow::Location>,
    pub data: Option<TypeId>,
    pub expression: Option<AccessExpressionId>,
    pub value: ValueSource,
}
/// Prepared values at an actual invocation commit, in the original selected bundle order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallValueSources {
    pub kind: crate::value_access::CallKind,
    pub invocation: nocter_model::ValueInvocation,
    pub parameters: BTreeMap<nocter_model::ValueParameter, usize>,
    pub origin: flow::Location,
    pub callee: ValueSourceId,
    pub arguments: Box<[ValueSourceId]>,
}
/// One exact body's original value definitions and sparse incoming merge edges.
/// IDs and canonical schemas are metadata, never source/revision or executable receipts.
#[derive(Clone, Debug)]
pub struct ValueSources {
    owner: ValueCode,
    nodes: Box<[ValueSourceNode]>,
    calls: BTreeMap<AccessExpressionId, Vec<CallValueSources>>,
    creations: BTreeMap<ValueClosure, ValueSourceId>,
    returned: BTreeSet<ValueSourceId>,
    flow: Option<ValueSourceFlow>,
}
impl ValueSources {
    #[cfg(test)]
    pub(in crate::value_access::results) fn test(
        owner: ValueCode,
        nodes: Vec<(Option<TypeId>, ValueSource)>,
        calls: BTreeMap<AccessExpressionId, Vec<CallValueSources>>,
        returned: BTreeSet<ValueSourceId>,
    ) -> Self {
        Self {
            owner,
            nodes: nodes
                .into_iter()
                .map(|(data, value)| ValueSourceNode {
                    origin: None,
                    data,
                    expression: None,
                    value,
                })
                .collect(),
            calls,
            creations: BTreeMap::new(),
            returned,
            flow: None,
        }
    }
    #[cfg(test)]
    pub(in crate::value_access::results) fn with_test_flow(
        mut self,
        flow: ValueSourceFlow,
    ) -> Self {
        self.flow = Some(flow);
        self
    }
    #[must_use]
    pub const fn flow(&self) -> Option<&ValueSourceFlow> {
        self.flow.as_ref()
    }
    #[must_use]
    pub const fn owner(&self) -> ValueCode {
        self.owner
    }
    #[must_use]
    pub fn node(&self, id: ValueSourceId) -> Option<&ValueSourceNode> {
        self.nodes.get(id.0)
    }
    #[must_use]
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = (ValueSourceId, &ValueSourceNode)> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(id, node)| (ValueSourceId(id), node))
    }
    #[must_use]
    pub fn call(&self, expression: AccessExpressionId) -> Option<&[CallValueSources]> {
        self.calls.get(&expression).map(Vec::as_slice)
    }
    #[must_use]
    pub fn creation(&self, code: ValueClosure) -> Option<ValueSourceId> {
        self.creations.get(&code).copied()
    }
    #[must_use]
    pub fn returned(&self) -> &BTreeSet<ValueSourceId> {
        &self.returned
    }
}
pub(super) use build::analyze;
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
type State = BTreeMap<ValueSourceSlot, ValueSourceId>;

pub(super) fn data(
    query: &Query,
    expression: AccessExpressionId,
) -> Result<Option<TypeId>, flow::IntegrityError> {
    Ok(query
        .body
        .expressions
        .get(expression)
        .ok_or(INVALID)?
        .ty
        .access()
        .map(nocter_model::ValueAccess::data))
}
pub(super) fn root(
    query: &Query,
    root: crate::value_access::paths::ProjectionRoot,
) -> Result<(ValueSourceSlot, nocter_model::TypeId), flow::IntegrityError> {
    match root {
        crate::value_access::paths::ProjectionRoot::Place(place) => {
            let place = query.body.places.get(place).ok_or(INVALID)?;
            let data = query.body.locals.get(place.local).ok_or(INVALID)?.data;
            Ok((ValueSourceSlot::Local(place.local), data))
        }
        crate::value_access::paths::ProjectionRoot::Temporary(value) => Ok((
            ValueSourceSlot::Expression(value),
            data(query, value)?.ok_or(INVALID)?,
        )),
    }
}
