//! Data of original preparation snapshots, including prefixes with no invocation commit.
use super::{
    CodeInstanceKey, DataInstanceKey, Evidence, InstantiationFailure, Program, WorkBudget, flow,
};
use crate::value_access::{ExpressionKind, ValueSourceSlot};
use nocter_model::{AccessExpressionId, ValueApplication};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct PreparedData {
    arguments: BTreeMap<(AccessExpressionId, usize), DataInstanceKey>,
}
impl PreparedData {
    #[must_use]
    pub fn get(&self, call: AccessExpressionId, index: usize) -> Option<&DataInstanceKey> {
        self.arguments.get(&(call, index))
    }
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = ((AccessExpressionId, usize), &DataInstanceKey)> + '_ {
        self.arguments.iter().map(|(&slot, key)| (slot, key))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedDataFailure {
    Body,
    Argument {
        site: ValueApplication,
        index: usize,
        failure: InstantiationFailure,
    },
}

pub(in crate::value_access) fn arguments(
    program: &Program,
    evidence: &Evidence,
    body: Option<&flow::Query>,
    frame: &CodeInstanceKey,
    work: &mut WorkBudget,
) -> Result<PreparedData, PreparedDataFailure> {
    let invalid = PreparedDataFailure::Body;
    let body = body
        .filter(|body| body.typed().code() == frame.code())
        .ok_or(invalid)?;
    let sources = evidence
        .value_sources(frame.code())
        .and_then(|graph| graph.flow())
        .ok_or(invalid)?;
    let mut prepared = PreparedData {
        arguments: BTreeMap::new(),
    };
    let mut resolution = super::super::instantiation::Query::new(program, evidence, work);
    for ((at, slot), source) in sources.definitions() {
        let ValueSourceSlot::Argument { call, index } = slot else {
            continue;
        };
        if !body.initialization().reachable(at.point) {
            continue;
        }
        // This slot may also have a post-call definition. Preparation contracts belong only
        // to the original preparation action, never to a completed edited temporary.
        if !matches!(
            body.graph().points().get(at.point)
                .and_then(|point| point.actions.get(at.action))
                .map(|action| &action.operation),
            Some(flow::Operation::PrepareArgument { call: prepared, argument })
                if *prepared == call && *argument == index
        ) {
            continue;
        }
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
        let expression = arguments.get(index).ok_or(invalid)?.value;
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
        let failure = |failure| PreparedDataFailure::Argument {
            site: ValueApplication {
                owner: frame.code(),
                call,
            },
            index,
            failure,
        };
        let key = resolution.data(frame, source, data).map_err(failure)?;
        if prepared
            .arguments
            .get(&(call, index))
            .is_some_and(|previous| previous != &key)
        {
            return Err(failure(InstantiationFailure::Unresolved));
        }
        prepared.arguments.insert((call, index), key);
    }
    Ok(prepared)
}
