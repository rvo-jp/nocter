//! Static eligibility selections reference prepared receiver operands, never invocation receipts.
use super::{INVALID, flow};
use crate::value_access::{ExpressionKind, ValueSourceId};
use nocter_model::AccessExpressionId;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub(super) struct Condition {
    pub(super) selections: Box<[(flow::Location, ValueSourceId)]>,
}

pub(super) fn collect(
    body: &flow::Query,
) -> Result<BTreeMap<AccessExpressionId, Condition>, flow::IntegrityError> {
    let query = body.typed();
    let mut receivers = BTreeMap::<_, Vec<_>>::new();
    let mut conditions = BTreeMap::<_, Vec<_>>::new();
    for (id, expression) in query.body().expressions().iter() {
        if super::super::interrupted_contract_call(&expression.kind) {
            let ExpressionKind::Call { arguments, .. } = &expression.kind else {
                return Err(INVALID);
            };
            let receiver = arguments.first().ok_or(INVALID)?.value;
            receivers.entry(receiver).or_default().push(id);
            conditions.insert(id, Vec::new());
        }
    }
    if !conditions.is_empty() {
        let flow = query
            .result_evidence
            .as_ref()
            .and_then(|evidence| evidence.value_sources(query.code()))
            .and_then(|sources| sources.flow())
            .ok_or(INVALID)?;
        for ((at, expression), source) in flow.operands() {
            for call in receivers.get(&expression).into_iter().flatten() {
                conditions.get_mut(call).ok_or(INVALID)?.push((at, source));
            }
        }
    }
    Ok(conditions
        .into_iter()
        .map(|(call, selections)| {
            (
                call,
                Condition {
                    selections: selections.into(),
                },
            )
        })
        .collect())
}
