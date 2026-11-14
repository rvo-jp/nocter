//! Freeze original operand and stored-definition data without adding an ownership state.
use super::{Evidence, InstantiationFailure, ValueSourceId, ValueSourceSlot};
use crate::value_access::flow::{self, resources::InstantiatedPayload as Payload};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{CodeInstanceKey, Program};
use nocter_model::{AccessExpressionId, TypeId};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueDataFailure {
    Body,
    Value {
        at: flow::Location,
        failure: InstantiationFailure,
    },
}
type Failure = ValueDataFailure;
const INVALID: Failure = Failure::Body;

#[derive(Clone, Debug)]
pub struct ValueData {
    operands: BTreeMap<(flow::Location, AccessExpressionId), Payload>,
    definitions: BTreeMap<(flow::Location, ValueSourceSlot), Payload>,
}
impl ValueData {
    #[must_use]
    pub fn operand(&self, at: flow::Location, expression: AccessExpressionId) -> Option<&Payload> {
        self.operands.get(&(at, expression))
    }
    #[must_use]
    pub fn definition(&self, at: flow::Location, slot: ValueSourceSlot) -> Option<&Payload> {
        self.definitions.get(&(at, slot))
    }
    pub fn payloads(&self) -> impl Iterator<Item = &Payload> {
        self.operands.values().chain(self.definitions.values())
    }
    pub(in crate::value_access) fn definitions(
        &self,
    ) -> impl Iterator<Item = ((flow::Location, ValueSourceSlot), &Payload)> {
        self.definitions
            .iter()
            .map(|(&slot, payload)| (slot, payload))
    }
}

pub(in crate::value_access) fn values(
    program: &Program,
    evidence: &Evidence,
    body: Option<&flow::Query>,
    frame: &CodeInstanceKey,
    work: &mut WorkBudget,
) -> Result<ValueData, Failure> {
    let body = body
        .filter(|body| body.typed().code() == frame.code())
        .ok_or(INVALID)?;
    let sources = evidence
        .value_sources(frame.code())
        .and_then(|graph| graph.flow())
        .ok_or(INVALID)?;
    let mut values = ValueData {
        operands: BTreeMap::new(),
        definitions: BTreeMap::new(),
    };
    for ((at, expression), source) in sources.operands() {
        if !body.initialization().reachable(at.point) {
            continue;
        }
        if let Some(data) = expression_data(body, expression)?
            && let Some(payload) = payload(program, evidence, frame, at, source, data, work)?
        {
            values.operands.insert((at, expression), payload);
        }
    }
    for ((at, slot), source) in sources.definitions() {
        if !body.initialization().reachable(at.point) {
            continue;
        }
        let data = match slot {
            ValueSourceSlot::Local(local) => {
                Some(body.typed().body().locals().get(local).ok_or(INVALID)?.data)
            }
            ValueSourceSlot::Expression(expression) => expression_data(body, expression)?,
            ValueSourceSlot::Argument { .. } | ValueSourceSlot::Return => continue,
        };
        if let Some(data) = data
            && let Some(payload) = payload(program, evidence, frame, at, source, data, work)?
        {
            values.definitions.insert((at, slot), payload);
        }
    }
    Ok(values)
}
fn expression_data(
    body: &flow::Query,
    expression: AccessExpressionId,
) -> Result<Option<TypeId>, Failure> {
    Ok(body
        .typed()
        .body()
        .expressions()
        .get(expression)
        .ok_or(INVALID)?
        .ty
        .access()
        .map(nocter_model::ValueAccess::data))
}
fn payload(
    program: &Program,
    evidence: &Evidence,
    frame: &CodeInstanceKey,
    at: flow::Location,
    source: ValueSourceId,
    data: TypeId,
    work: &mut WorkBudget,
) -> Result<Option<Payload>, Failure> {
    if evidence.unproduced(program, data) {
        return Ok(None);
    }
    let contract = flow::resources::payload(program, data).map_err(|_| INVALID)?;
    let key = super::instantiate_value_data(program, evidence, frame, source, data, work)
        .map_err(|failure| Failure::Value { at, failure })?;
    Ok(Some(
        contract.instantiate(program, &key).map_err(|_| INVALID)?,
    ))
}
