//! Preparation receipts survive later argument effects and interrupted commits.
use super::{Failure, InstanceKey};
use crate::value_access::Payload;
use nocter_checking::value_access::{
    ExpressionKind, InstantiationFailure,
    accepted::Unit,
    flow::{self, Operation},
};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_model::{AccessExpressionId, ValueApplication};
use std::collections::BTreeMap;

pub(in crate::value_access) fn arguments(
    unit: &Unit,
    frame: &InstanceKey,
    body: &flow::Query,
    work: &mut WorkBudget,
) -> Result<BTreeMap<(AccessExpressionId, usize), Payload>, Failure> {
    let invalid = Failure::InvalidAcceptedUnit;
    let sources = unit
        .value_sources(frame.code())
        .and_then(|graph| graph.flow())
        .ok_or(invalid)?;
    let mut prepared = BTreeMap::new();
    for (point, block) in body.graph().points().iter() {
        if !body.initialization().reachable(point) {
            continue;
        }
        for (action, event) in block.actions.iter().enumerate() {
            let Operation::PrepareArgument { call, argument } = event.operation else {
                continue;
            };
            let ExpressionKind::Call { arguments, .. } = &body
                .typed()
                .body()
                .expressions()
                .get(call)
                .ok_or(invalid)?
                .kind
            else {
                return Err(invalid);
            };
            let expression = arguments.get(argument).ok_or(invalid)?.value;
            let data = body
                .typed()
                .body()
                .expressions()
                .get(expression)
                .ok_or(invalid)?
                .ty
                .normal()
                .ok_or(invalid)?
                .data();
            let source = sources
                .operand(flow::Location { point, action }, expression)
                .ok_or(invalid)?;
            let failure = |failure| Failure::Invocation {
                site: ValueApplication {
                    owner: frame.code(),
                    call,
                },
                failure,
            };
            let key = unit
                .instantiate_value_data(frame, source, data, work)
                .map_err(failure)?;
            let payload = unit
                .data_evidence(data)
                .map_err(|_| invalid)?
                .payload()
                .ok_or(invalid)?
                .instantiate(unit.program(), &key)
                .map_err(|_| invalid)?;
            if prepared
                .get(&(call, argument))
                .is_some_and(|previous| previous != &payload)
            {
                return Err(failure(InstantiationFailure::Unresolved));
            }
            prepared.insert((call, argument), payload);
        }
    }
    Ok(prepared)
}
