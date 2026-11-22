//! Mechanical witness/data substitution, consuming checked binding edges rather than lookup inputs.
use super::{DataEvidence, Failure, InstanceKey};
use nocter_checking::value_access::{accepted::Unit, flow};
use nocter_declarations::value_access::DataInstanceKey;
use std::collections::BTreeSet;
mod abi;
mod prepared;
mod values;
pub(super) use prepared::arguments as prepared;
pub(super) use values::{Values, values};
mod requirements;
pub(super) use abi::{inputs, result};

/// Independently closed storage metadata only; actual occurrences own contextual roots.
pub(super) fn metadata(
    unit: &Unit,
    body: &flow::Query,
    control: &nocter_checking::value_access::InstanceControl,
) -> Result<BTreeSet<DataInstanceKey>, Failure> {
    let program = unit.program();
    let mut roots = BTreeSet::new();
    for source in requirements::data(body, control)? {
        if unit.elides_future_data(body.typed().code(), source)
            || unit
                .data_evidence(source)
                .map_err(|_| Failure::InvalidAcceptedUnit)?
                .payload()
                .is_none()
        {
            continue;
        }
        if let Ok(key) = program.instantiate_closed_data(source) {
            roots.insert(key);
        }
    }
    Ok(roots)
}
