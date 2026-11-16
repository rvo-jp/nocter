//! Original expression merges select one receiving domain; original writes still own availability.
use super::{
    AccessExpressionId, BTreeMap, CodeInstanceKey, DataInstanceKey, Evidence, Failure, INVALID,
    InstantiationFailure, Program, ValueData, ValueSourceSlot, WorkBudget,
};
use nocter_declarations::value_access::DataDomainJoinFailure;
use std::collections::BTreeSet;

pub(super) fn prepare(
    program: &Program,
    evidence: &Evidence,
    frame: &CodeInstanceKey,
    values: &ValueData,
    work: &mut WorkBudget,
) -> Result<BTreeMap<AccessExpressionId, DataInstanceKey>, Failure> {
    let graph = evidence.value_sources(frame.code()).ok_or(INVALID)?;
    let mut expressions = BTreeSet::new();
    for (_, node) in graph.nodes() {
        work.step()
            .map_err(|_| Failure::Control(InstantiationFailure::WorkLimit))?;
        if let super::super::ValueSource::Merge {
            slot: ValueSourceSlot::Expression(expression),
            ..
        } = node.value
        {
            expressions.insert(expression);
        }
    }
    let mut joined = BTreeMap::<AccessExpressionId, DataInstanceKey>::new();
    for (&(at, slot), payload) in &values.definitions {
        work.step().map_err(|_| Failure::Value {
            at,
            failure: InstantiationFailure::WorkLimit,
        })?;
        let ValueSourceSlot::Expression(expression) = slot else {
            continue;
        };
        if !expressions.contains(&expression) {
            continue;
        }
        let next = if let Some(previous) = joined.get(&expression) {
            program
                .join_data_domains(previous, payload.instance(), || work.step())
                .map_err(|failure| Failure::Value {
                    at,
                    failure: match failure {
                        DataDomainJoinFailure::Binding(_) => InstantiationFailure::Unresolved,
                        DataDomainJoinFailure::Work(_) => InstantiationFailure::WorkLimit,
                    },
                })?
        } else {
            payload.instance().clone()
        };
        joined.insert(expression, next);
    }
    Ok(joined)
}
