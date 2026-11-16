//! Freeze original operand and stored-definition data without adding an ownership state.
use super::{Evidence, InstantiationFailure, ValueSourceId, ValueSourceSlot};
use crate::value_access::{
    Referent,
    flow::{self, resources::InstantiatedPayload as Payload},
};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{CodeInstanceKey, DataInstanceKey, Program};
use nocter_model::{AccessExpressionId, TypeId};
use std::collections::BTreeMap;
mod receipts;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueDataFailure {
    Body,
    Control(InstantiationFailure),
    Value {
        at: flow::Location,
        failure: InstantiationFailure,
    },
}
type Failure = ValueDataFailure;
const INVALID: Failure = Failure::Body;

#[derive(Clone, Debug)]
struct RootContracts {
    before: DataInstanceKey,
    after: Option<DataInstanceKey>,
}
#[derive(Clone, Debug)]
pub struct ValueData {
    operands: BTreeMap<(flow::Location, AccessExpressionId), Payload>,
    definitions: BTreeMap<(flow::Location, ValueSourceSlot), Payload>,
    roots: BTreeMap<(flow::Location, Referent), RootContracts>,
    receipts: BTreeMap<AccessExpressionId, DataInstanceKey>,
}
impl ValueData {
    /// Common represented domain of an original expression merge, not a value or owner receipt.
    #[must_use]
    pub fn receipt(&self, expression: AccessExpressionId) -> Option<&DataInstanceKey> {
        self.receipts.get(&expression)
    }
    /// Reference-root contract metadata, not an initialized value or physical allocation receipt.
    #[must_use]
    pub fn root(&self, at: flow::Location, referent: Referent) -> Option<&DataInstanceKey> {
        self.roots.get(&(at, referent)).map(|roots| &roots.before)
    }
    /// Normal-completion contract only; this is not a write or execution receipt.
    #[must_use]
    pub fn root_after(&self, at: flow::Location, referent: Referent) -> Option<&DataInstanceKey> {
        self.roots.get(&(at, referent))?.after.as_ref()
    }
    pub fn roots(&self) -> impl Iterator<Item = &DataInstanceKey> {
        self.roots
            .values()
            .flat_map(|roots| std::iter::once(&roots.before).chain(roots.after.as_ref()))
            .chain(self.receipts.values())
    }
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
    let control = super::instantiate_control(program, evidence, Some(body), frame, work)
        .map_err(Failure::Control)?;
    let mut values = ValueData {
        operands: BTreeMap::new(),
        definitions: BTreeMap::new(),
        roots: BTreeMap::new(),
        receipts: BTreeMap::new(),
    };
    let mut resolution =
        super::instantiation::Query::with_control(program, evidence, work, &control);
    for ((at, expression), source) in sources.operands() {
        if !control.reachable(at.point) {
            continue;
        }
        if let Some(data) = expression_data(body, expression)?
            && let Some(payload) =
                payload(program, evidence, frame, at, source, data, &mut resolution)?
        {
            values.operands.insert((at, expression), payload);
        }
    }
    let mut contextual_receipts = false;
    for ((at, slot), source) in sources.definitions() {
        if !control.reachable(at.point) {
            continue;
        }
        if matches!(
            slot,
            ValueSourceSlot::Argument { .. } | ValueSourceSlot::Return
        ) {
            continue;
        }
        let data = slot.data(body.typed()).map_err(|_| INVALID)?;
        if let Some(data) = data
            && let Some(payload) =
                payload(program, evidence, frame, at, source, data, &mut resolution)?
        {
            contextual_receipts |= !payload.instance().is_context_free();
            values.definitions.insert((at, slot), payload);
        }
    }
    for ((at, referent), source) in sources.roots() {
        if !control.reachable(at.point) {
            continue;
        }
        let data = root_type(body.typed(), referent).ok_or(INVALID)?;
        if evidence.unproduced(program, data) {
            continue;
        }
        let key = root_data(
            program,
            body.typed(),
            frame,
            source,
            referent,
            &mut resolution,
        )
        .map_err(|failure| Failure::Value { at, failure })?;
        let after = sources
            .root_after(at, referent)
            .map(|source| {
                root_data(
                    program,
                    body.typed(),
                    frame,
                    source,
                    referent,
                    &mut resolution,
                )
            })
            .transpose()
            .map_err(|failure| Failure::Value { at, failure })?;
        values
            .roots
            .insert((at, referent), RootContracts { before: key, after });
    }
    drop(resolution);
    if contextual_receipts {
        values.receipts = receipts::prepare(program, evidence, frame, &values, work)?;
    }
    Ok(values)
}
pub(super) fn root_type(body: &crate::value_access::Query, referent: Referent) -> Option<TypeId> {
    let body = body.body();
    match referent {
        Referent::Place(place) => body
            .locals()
            .get(body.places().get(place)?.local)
            .map(|local| local.data),
        Referent::Evaluated(_) => body.referent_data(referent),
    }
}
/// Resolve only an original root contract. Closed metadata does not assert a produced value.
pub(in crate::value_access) fn root(
    program: &Program,
    evidence: &Evidence,
    body: &flow::Query,
    frame: &CodeInstanceKey,
    at: flow::Location,
    referent: Referent,
    work: &mut WorkBudget,
) -> Result<DataInstanceKey, InstantiationFailure> {
    let source = root_flow(evidence, body, frame)?
        .root(at, referent)
        .ok_or(InstantiationFailure::Unresolved)?;
    selected_root_data(program, evidence, body, frame, source, referent, work)
}
pub(in crate::value_access) fn root_after(
    program: &Program,
    evidence: &Evidence,
    body: &flow::Query,
    frame: &CodeInstanceKey,
    at: flow::Location,
    referent: Referent,
    work: &mut WorkBudget,
) -> Result<DataInstanceKey, InstantiationFailure> {
    let source = root_flow(evidence, body, frame)?
        .root_after(at, referent)
        .ok_or(InstantiationFailure::Unresolved)?;
    selected_root_data(program, evidence, body, frame, source, referent, work)
}
fn selected_root_data(
    program: &Program,
    evidence: &Evidence,
    body: &flow::Query,
    frame: &CodeInstanceKey,
    source: ValueSourceId,
    referent: Referent,
    work: &mut WorkBudget,
) -> Result<DataInstanceKey, InstantiationFailure> {
    let data = root_type(body.typed(), referent).ok_or(InstantiationFailure::Unresolved)?;
    if let Ok(key) = program.instantiate_closed_data(data) {
        return Ok(key);
    }
    super::instantiation::Query::controlled(program, evidence, Some(body), frame, work)?
        .data(frame, source, data)
}
pub(super) fn root_flow<'a>(
    evidence: &'a Evidence,
    body: &flow::Query,
    frame: &CodeInstanceKey,
) -> Result<&'a super::ValueSourceFlow, InstantiationFailure> {
    if body.typed().code() != frame.code() {
        return Err(InstantiationFailure::WrongOwner);
    }
    evidence
        .value_sources(frame.code())
        .and_then(|graph| graph.flow())
        .ok_or(InstantiationFailure::Unresolved)
}
pub(super) fn root_data(
    program: &Program,
    body: &crate::value_access::Query,
    frame: &CodeInstanceKey,
    source: ValueSourceId,
    referent: Referent,
    resolution: &mut super::instantiation::Query<'_, '_>,
) -> Result<DataInstanceKey, InstantiationFailure> {
    let data = root_type(body, referent).ok_or(InstantiationFailure::Unresolved)?;
    if let Ok(key) = program.instantiate_closed_data(data) {
        return Ok(key);
    }
    resolution.data(frame, source, data)
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
    resolution: &mut super::instantiation::Query<'_, '_>,
) -> Result<Option<Payload>, Failure> {
    if evidence.unproduced(program, data) {
        return Ok(None);
    }
    let contract = flow::resources::payload(program, data).map_err(|_| INVALID)?;
    let key = resolution
        .data(frame, source, data)
        .map_err(|failure| Failure::Value { at, failure })?;
    Ok(Some(
        contract.instantiate(program, &key).map_err(|_| INVALID)?,
    ))
}
