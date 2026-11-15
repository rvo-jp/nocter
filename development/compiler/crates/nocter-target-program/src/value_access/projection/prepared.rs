//! Instantiate checking's original preparation metadata without adding another source traversal.
use super::{Failure, InstanceKey};
use crate::value_access::Payload;
use nocter_checking::value_access::{PreparedDataFailure, accepted::Unit};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_model::AccessExpressionId;
use std::collections::BTreeMap;

pub(in crate::value_access) fn arguments(
    unit: &Unit,
    frame: &InstanceKey,
    work: &mut WorkBudget,
) -> Result<BTreeMap<(AccessExpressionId, usize), Payload>, Failure> {
    let invalid = Failure::InvalidAcceptedUnit;
    let prepared =
        unit.instantiate_prepared_data(frame, work)
            .map_err(|failure| match failure {
                PreparedDataFailure::Body => invalid,
                PreparedDataFailure::Control(failure) => Failure::Control {
                    code: frame.code(),
                    failure,
                },
                PreparedDataFailure::Argument { site, failure, .. } => {
                    Failure::Invocation { site, failure }
                }
            })?;
    prepared
        .iter()
        .map(|(slot, key)| {
            let payload = unit
                .data_evidence(key.data())
                .map_err(|_| invalid)?
                .payload()
                .ok_or(invalid)?
                .instantiate(unit.program(), key)
                .map_err(|_| invalid)?;
            Ok((slot, payload))
        })
        .collect()
}
