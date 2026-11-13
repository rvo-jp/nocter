//! One lazy source/contract call resolution per bound clause check, never recursive admission.
use super::{Requirements, Violation, flow, instances};
use nocter_declarations::value_access::CodeInstanceKey;
use nocter_model::{AccessExpressionId, ValueClosure};

/// Exact call metadata discovered while checking local clauses; not recursive const admission.
pub(in crate::value_access) enum BoundCheck {
    Violation(Violation),
    Calls(std::collections::BTreeMap<AccessExpressionId, CodeInstanceKey>),
}
#[derive(Clone, Copy, Debug)]
pub(in crate::value_access) enum BoundCheckFailure {
    Integrity(flow::IntegrityError),
    Invocation {
        expression: AccessExpressionId,
        failure: crate::value_access::InstantiationFailure,
    },
}
impl From<flow::IntegrityError> for BoundCheckFailure {
    fn from(error: flow::IntegrityError) -> Self {
        Self::Integrity(error)
    }
}
pub(super) struct BoundCalls<'a, 'work> {
    query: &'a crate::value_access::Query,
    caller: &'a CodeInstanceKey,
    work: &'work mut nocter_constant_evaluation::state::WorkBudget,
    calls: std::collections::BTreeMap<AccessExpressionId, Option<CodeInstanceKey>>,
}
impl BoundCalls<'_, '_> {
    pub(super) fn resolve(
        &mut self,
        expression: AccessExpressionId,
    ) -> Result<Option<&CodeInstanceKey>, BoundCheckFailure> {
        use std::collections::btree_map::Entry;
        let call = match self.calls.entry(expression) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(
                instances::condition_call(self.query, self.caller, expression, self.work).map_err(
                    |failure| BoundCheckFailure::Invocation {
                        expression,
                        failure,
                    },
                )?,
            ),
        };
        Ok(call.as_ref())
    }
}

impl Requirements {
    pub(in crate::value_access) fn check_bound(
        &self,
        available: &dyn Fn(ValueClosure) -> bool,
        caller: &CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<BoundCheck, BoundCheckFailure> {
        let mut calls = BoundCalls {
            query: self.body.typed(),
            caller,
            work,
            calls: std::collections::BTreeMap::new(),
        };
        if let Some(violation) = self.check_clauses(available, Some(caller), Some(&mut calls))? {
            return Ok(BoundCheck::Violation(violation));
        }
        Ok(BoundCheck::Calls(
            calls
                .calls
                .into_iter()
                .filter_map(|(expression, call)| call.map(|call| (expression, call)))
                .collect(),
        ))
    }
}
