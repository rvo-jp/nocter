//! Instance-specific continuation selection over original points, never an access or value proof.
use super::{Evidence, InstantiationFailure as Failure};
use crate::value_access::flow;
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{CodeInstanceKey, Program};
use nocter_model::AccessFlowPointId;
use std::{collections::BTreeMap, sync::Arc};
pub(super) mod original;

#[derive(Clone, Debug)]
pub struct InstanceControl {
    frame: CodeInstanceKey,
    points: BTreeMap<AccessFlowPointId, flow::Terminator>,
}
impl InstanceControl {
    pub(in crate::value_access) fn call_reachable(
        &self,
        sources: &super::ValueSources,
        expression: nocter_model::AccessExpressionId,
    ) -> Result<bool, flow::IntegrityError> {
        if sources.owner() != self.frame.code() {
            return Err(flow::IntegrityError::InconsistentTypedBody);
        }
        Ok(sources
            .call(expression)
            .is_none_or(|calls| calls.iter().any(|call| self.reachable(call.origin.point))))
    }
    #[must_use]
    pub const fn frame(&self) -> &CodeInstanceKey {
        &self.frame
    }
    #[must_use]
    pub fn reachable(&self, point: AccessFlowPointId) -> bool {
        self.points.contains_key(&point)
    }
    #[must_use]
    pub fn terminator(&self, point: AccessFlowPointId) -> Option<flow::Terminator> {
        self.points.get(&point).copied()
    }
    #[must_use]
    pub fn edge_reachable(&self, from: AccessFlowPointId, to: AccessFlowPointId) -> bool {
        self.terminator(from)
            .is_some_and(|terminator| terminator.successors().contains(&to))
    }
}

pub(in crate::value_access) fn instantiate(
    program: &Program,
    evidence: &Evidence,
    body: Option<&flow::Query>,
    frame: &CodeInstanceKey,
    work: &mut WorkBudget,
) -> Result<InstanceControl, Failure> {
    let body = body
        .filter(|body| body.typed().code() == frame.code())
        .ok_or(Failure::WrongOwner)?;
    from_graph(program, evidence, body.typed(), body.graph(), frame, work)
}

pub(in crate::value_access) fn from_graph(
    program: &Program,
    evidence: &Evidence,
    typed: &crate::value_access::Query,
    graph: &flow::Graph,
    frame: &CodeInstanceKey,
    work: &mut WorkBudget,
) -> Result<InstanceControl, Failure> {
    if typed.code() != frame.code() {
        return Err(Failure::WrongOwner);
    }
    let sources = evidence.value_sources(frame.code());
    if let Some(original) = sources.and_then(|sources| sources.control.as_ref()) {
        if !std::ptr::eq(graph, original.graph.as_ref()) && graph != original.graph.as_ref() {
            return Err(Failure::WrongOwner);
        }
        return original.instantiate(program, evidence, frame, work);
    }
    // A caller holding an original typed graph can still establish its probes. A metadata-only
    // query without that body must retain conservative control when no frozen recipe exists.
    let original = original::Original::prepare(
        typed,
        Arc::new(graph.clone()),
        sources.and_then(super::ValueSources::flow),
    )
    .map_err(|_| Failure::Unresolved)?;
    original.instantiate(program, evidence, frame, work)
}

#[cfg(test)]
mod tests;
