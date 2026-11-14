//! Freeze original operand and stored-definition data without adding an ownership state.
use super::{Failure, InstanceKey};
use crate::value_access::Payload;
use nocter_checking::value_access::{ValueSourceId, ValueSourceSlot, accepted::Unit, flow};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_model::{AccessExpressionId, TypeId};
use std::collections::BTreeMap;
const INVALID: Failure = Failure::InvalidAcceptedUnit;

#[derive(Clone, Debug, Default)]
pub(in crate::value_access) struct Values {
    operands: BTreeMap<(flow::Location, AccessExpressionId), Payload>,
    definitions: BTreeMap<(flow::Location, ValueSourceSlot), Payload>,
}
impl Values {
    pub(in crate::value_access) fn operand(
        &self,
        at: flow::Location,
        expression: AccessExpressionId,
    ) -> Option<&Payload> {
        self.operands.get(&(at, expression))
    }
    pub(in crate::value_access) fn definition(
        &self,
        at: flow::Location,
        slot: ValueSourceSlot,
    ) -> Option<&Payload> {
        self.definitions.get(&(at, slot))
    }
    pub(in crate::value_access) fn payloads(&self) -> impl Iterator<Item = &Payload> {
        self.operands.values().chain(self.definitions.values())
    }
}

pub(in crate::value_access) fn values(
    unit: &Unit,
    frame: &InstanceKey,
    body: &flow::Query,
    work: &mut WorkBudget,
) -> Result<Values, Failure> {
    let sources = unit
        .value_sources(frame.code())
        .and_then(|graph| graph.flow())
        .ok_or(INVALID)?;
    let mut values = Values::default();
    for ((at, expression), source) in sources.operands() {
        if !body.initialization().reachable(at.point) {
            continue;
        }
        if let Some(data) = expression_data(body, expression)?
            && let Some(payload) = payload(unit, frame, at, source, data, work)?
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
            && let Some(payload) = payload(unit, frame, at, source, data, work)?
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
    unit: &Unit,
    frame: &InstanceKey,
    at: flow::Location,
    source: ValueSourceId,
    data: TypeId,
    work: &mut WorkBudget,
) -> Result<Option<Payload>, Failure> {
    let Some(contract) = unit.data_evidence(data).map_err(|_| INVALID)?.payload() else {
        return Ok(None);
    };
    let key = unit
        .instantiate_value_data(frame, source, data, work)
        .map_err(|failure| Failure::Value {
            code: frame.code(),
            at,
            failure,
        })?;
    Ok(Some(
        contract
            .instantiate(unit.program(), &key)
            .map_err(|_| INVALID)?,
    ))
}
