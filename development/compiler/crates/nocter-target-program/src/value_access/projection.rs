//! Mechanical witness/data substitution, consuming checked binding edges rather than lookup inputs.
use super::{DataEvidence, Failure, InstanceKey};
use nocter_checking::value_access::{
    accepted::{DataEvidence as CheckedDataEvidence, Unit},
    flow,
};
use nocter_model::TypeId;
use std::collections::{BTreeMap, BTreeSet};
mod abi;
mod prepared;
mod values;
pub(super) use prepared::arguments as prepared;
pub(super) use values::{Values, values};
mod requirements;
pub(super) use abi::{inputs, result};

pub(super) fn data(
    unit: &Unit,
    key: &InstanceKey,
    body: &flow::Query,
) -> Result<BTreeMap<TypeId, DataEvidence>, Failure> {
    let program = unit.program();
    let typed = body.typed().body();
    let mut metadata = BTreeSet::new();
    metadata.extend(typed.locals().iter().map(|(_, local)| local.data));
    metadata.extend(typed.places().iter().map(|(_, place)| place.data));
    metadata.extend(
        typed
            .expressions()
            .iter()
            .filter_map(|(_, e)| e.ty.access().map(nocter_model::ValueAccess::data)),
    );
    metadata.extend(
        typed
            .blocks()
            .iter()
            .filter_map(|(_, b)| b.ty.access().map(nocter_model::ValueAccess::data)),
    );
    let mut pending = requirements::data(body)?;
    let mut concrete = BTreeMap::new();
    // Preserve source-checked absent shapes as metadata. Positive dormant source does not
    // request a concrete carrier, storage schema or an otherwise unavailable witness binding.
    for source in metadata {
        let evidence = unit
            .data_evidence(source)
            .map_err(|_| Failure::InvalidAcceptedUnit)?;
        if evidence.payload().is_none() {
            concrete.insert(source, DataEvidence::Unproduced);
        }
    }
    while let Some(source) = pending.pop_first() {
        if concrete.contains_key(&source) {
            continue;
        }
        let evidence = unit
            .data_evidence(source)
            .map_err(|_| Failure::InvalidAcceptedUnit)?;
        let projected = match evidence {
            CheckedDataEvidence::Unproduced => DataEvidence::Unproduced,
            CheckedDataEvidence::Payload(contract) => {
                let instance = program
                    .instantiate_data(source, key)
                    .map_err(|_| Failure::InvalidAcceptedUnit)?;
                DataEvidence::Payload(
                    contract
                        .instantiate(program, &instance)
                        .map_err(|_| Failure::InvalidAcceptedUnit)?,
                )
            }
        };
        concrete.insert(source, projected);
    }
    Ok(concrete)
}
