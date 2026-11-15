//! Instance-specific continuation selection over original points, never an access or value proof.
use super::{Evidence, InstantiationFailure as Failure};
use crate::value_access::{ExpressionKind, Referent, flow};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{CodeInstanceKey, Program};
use nocter_model::{AccessFlowPointId, ValueField};
use std::collections::{BTreeMap, BTreeSet};

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
    let mut points = BTreeMap::new();
    let mut pending = BTreeSet::from([graph.entry()]);
    while let Some(point) = pending.pop_first() {
        if points.contains_key(&point) {
            continue;
        }
        work.step().map_err(|_| Failure::WorkLimit)?;
        let block = graph.points().get(point).ok_or(Failure::Unresolved)?;
        let mut terminator = block.terminator;
        if let flow::Terminator::Branch { condition, no, .. } = terminator {
            let expression = typed
                .body()
                .expressions()
                .get(condition)
                .ok_or(Failure::Unresolved)?;
            if let ExpressionKind::PrimaryCondition { source } = expression.kind {
                let action = block.actions.iter().position(|action| matches!(action.operation, flow::Operation::Evaluate(id) if id == condition)).ok_or(Failure::Unresolved)?;
                if excluded(
                    program,
                    evidence,
                    typed,
                    frame,
                    flow::Location { point, action },
                    source,
                    work,
                )? {
                    terminator = flow::Terminator::Jump(no);
                }
            }
        }
        pending.extend(terminator.successors());
        points.insert(point, terminator);
    }
    Ok(InstanceControl {
        frame: frame.clone(),
        points,
    })
}

fn excluded(
    program: &Program,
    evidence: &Evidence,
    body: &crate::value_access::Query,
    frame: &CodeInstanceKey,
    at: flow::Location,
    referent: Referent,
    work: &mut WorkBudget,
) -> Result<bool, Failure> {
    let source = evidence
        .value_sources(frame.code())
        .and_then(|sources| sources.flow())
        .and_then(|flow| flow.root(at, referent))
        .ok_or(Failure::Unresolved)?;
    let key = match super::values::root_data(
        program,
        body,
        frame,
        source,
        referent,
        &mut super::instantiation::Query::new(program, evidence, work),
    ) {
        Ok(key) => key,
        Err(Failure::WorkLimit) => return Err(Failure::WorkLimit),
        Err(_) => return Ok(false),
    };
    let selected = match referent {
        Referent::Place(place) => key.project_path(
            program,
            &body
                .body()
                .places()
                .get(place)
                .ok_or(Failure::Unresolved)?
                .fields,
        ),
        Referent::Evaluated(_) => Ok(key),
    }
    .map_err(|_| Failure::Unresolved)?;
    Ok(selected
        .component(program, ValueField::Variant(true))
        .map_err(|_| Failure::Unresolved)?
        .is_uninhabited())
}

#[cfg(test)]
mod tests;
