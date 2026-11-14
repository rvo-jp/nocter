//! Demand-driven containing contracts from original source selections, not physical layout proof.
use super::{Evidence, InstantiationFailure as Failure, ValueSourceCarrier};
use crate::value_access::{Referent, flow};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{CodeInstanceKey, DataInstanceKey, Program};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferentCarrierData {
    selection: ValueSourceCarrier,
    instance: DataInstanceKey,
}
impl ReferentCarrierData {
    #[must_use]
    pub const fn selection(&self) -> &ValueSourceCarrier {
        &self.selection
    }
    #[must_use]
    pub const fn instance(&self) -> &DataInstanceKey {
        &self.instance
    }
}
/// Complete original containing contracts for a single referent occurrence. Metadata does not
/// establish an initialized owner, enclosing allocation, stable geometry or resource phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferentCarrierContracts {
    before: Box<[ReferentCarrierData]>,
    after: Option<Box<[ReferentCarrierData]>>,
}
impl ReferentCarrierContracts {
    #[must_use]
    pub fn before(&self) -> &[ReferentCarrierData] {
        &self.before
    }
    #[must_use]
    pub fn after(&self) -> Option<&[ReferentCarrierData]> {
        self.after.as_deref()
    }
}
pub(in crate::value_access) fn instantiate(
    program: &Program,
    evidence: &Evidence,
    body: &flow::Query,
    frame: &CodeInstanceKey,
    at: flow::Location,
    referent: Referent,
    work: &mut WorkBudget,
) -> Result<ReferentCarrierContracts, Failure> {
    let roots = super::values::root_flow(evidence, body, frame)?;
    let before = resolve(
        program,
        evidence,
        body,
        frame,
        roots
            .root_carriers(at, referent)
            .ok_or(Failure::Unresolved)?,
        work,
    )?;
    let after = roots
        .root_carriers_after(at, referent)
        .map(|selections| resolve(program, evidence, body, frame, selections, work))
        .transpose()?;
    Ok(ReferentCarrierContracts { before, after })
}
fn resolve(
    program: &Program,
    evidence: &Evidence,
    body: &flow::Query,
    frame: &CodeInstanceKey,
    selections: &[ValueSourceCarrier],
    work: &mut WorkBudget,
) -> Result<Box<[ReferentCarrierData]>, Failure> {
    selections
        .iter()
        .map(|selection| {
            work.step().map_err(|_| Failure::WorkLimit)?;
            let data = selection
                .slot
                .data(body.typed())
                .map_err(|_| Failure::Unresolved)?
                .ok_or(Failure::Unresolved)?;
            let instance = match program.instantiate_closed_data(data) {
                Ok(key) => key,
                Err(_) => super::instantiate_value_data(
                    program,
                    evidence,
                    frame,
                    selection.source,
                    data,
                    work,
                )?,
            };
            Ok(ReferentCarrierData {
                selection: selection.clone(),
                instance,
            })
        })
        .collect()
}
