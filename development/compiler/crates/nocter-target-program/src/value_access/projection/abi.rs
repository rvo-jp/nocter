//! Body channels retain actual input locals and original return producers, not type-map defaults.
use super::{DataEvidence, Failure, InstanceKey};
use crate::value_access::Payload;
use nocter_checking::value_access::{
    accepted::{DataEvidence as CheckedEvidence, Unit},
    flow,
};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_model::AccessLocalId;
use std::collections::BTreeMap;
const INVALID: Failure = Failure::InvalidAcceptedUnit;

pub(in crate::value_access) fn inputs(
    unit: &Unit,
    frame: &InstanceKey,
    body: &flow::Query,
    work: &mut WorkBudget,
) -> Result<BTreeMap<AccessLocalId, Payload>, Failure> {
    let mut payloads = BTreeMap::new();
    let layout = body.typed().body().inputs();
    for input in layout
        .environment()
        .into_iter()
        .chain(layout.explicit().iter().copied())
    {
        let source = body
            .typed()
            .body()
            .locals()
            .get(input.local)
            .ok_or(INVALID)?
            .data;
        let contract = unit
            .data_evidence(source)
            .map_err(|_| INVALID)?
            .payload()
            .ok_or(INVALID)?;
        let key = unit
            .instantiate_input_data(frame, input.local, work)
            .map_err(|failure| Failure::Input {
                code: frame.code(),
                local: input.local,
                failure,
            })?;
        let payload = contract
            .instantiate_in_frame(unit.program(), frame, &key)
            .map_err(|_| INVALID)?;
        if payloads.insert(input.local, payload).is_some() {
            return Err(INVALID);
        }
    }
    Ok(payloads)
}

pub(in crate::value_access) fn result(
    unit: &Unit,
    frame: &InstanceKey,
    body: &flow::Query,
    work: &mut WorkBudget,
) -> Result<DataEvidence, Failure> {
    let source = body.typed().result_data();
    match unit.data_evidence(source).map_err(|_| INVALID)? {
        CheckedEvidence::Unproduced => Ok(DataEvidence::Unproduced),
        CheckedEvidence::Payload(contract) => {
            let key = unit
                .instantiate_result_contract(frame, work)
                .map_err(|failure| Failure::Result {
                    code: frame.code(),
                    failure,
                })?;
            Ok(DataEvidence::Payload(
                contract
                    .instantiate_in_frame(unit.program(), frame, &key)
                    .map_err(|_| INVALID)?,
            ))
        }
    }
}
