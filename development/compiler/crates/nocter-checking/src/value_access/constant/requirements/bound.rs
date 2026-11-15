//! Request-local call and resource metadata for bound clauses, never recursive admission.
use super::{Requirements, Violation, cleanup, drops, flow, instances};
use nocter_declarations::value_access::CodeInstanceKey;
use nocter_model::{AccessExpressionId, ValueClosure};

/// Exact call metadata discovered while checking local clauses; not recursive const admission.
pub(in crate::value_access) enum BoundCheck {
    Violation(Violation),
    Calls {
        calls: std::collections::BTreeMap<AccessExpressionId, CodeInstanceKey>,
        control: crate::value_access::InstanceControl,
    },
}
#[derive(Clone, Copy, Debug)]
pub(in crate::value_access) enum BoundCheckFailure {
    Integrity(flow::IntegrityError),
    Control(crate::value_access::InstantiationFailure),
    Invocation {
        expression: AccessExpressionId,
        failure: crate::value_access::InstantiationFailure,
    },
    Data {
        at: flow::Location,
        failure: crate::value_access::InstantiationFailure,
    },
}
impl From<flow::IntegrityError> for BoundCheckFailure {
    fn from(error: flow::IntegrityError) -> Self {
        Self::Integrity(error)
    }
}
pub(super) struct BoundContext<'a, 'work> {
    query: &'a crate::value_access::Query,
    caller: &'a CodeInstanceKey,
    work: &'work mut nocter_constant_evaluation::state::WorkBudget,
    calls: std::collections::BTreeMap<AccessExpressionId, Option<CodeInstanceKey>>,
    resources: std::collections::BTreeMap<(flow::Location, usize), cleanup::Projected>,
    control: crate::value_access::InstanceControl,
}
impl BoundContext<'_, '_> {
    pub(super) fn active(&self, clause: &super::Clause) -> Result<bool, BoundCheckFailure> {
        Ok(match clause {
            super::Clause::Invocation { expression, .. } | super::Clause::Inputs(expression) => {
                let sources = self
                    .query
                    .result_evidence
                    .as_ref()
                    .and_then(|evidence| evidence.value_sources(self.caller.code()))
                    .ok_or(super::INVALID)?;
                self.control.call_reachable(sources, *expression)?
            }
            super::Clause::Data(condition) => condition.active(&self.control),
            super::Clause::Drop(condition) => condition.active(&self.control),
            super::Clause::Cleanup(condition) => {
                self.control.reachable(condition.request()?.0.point)
            }
        })
    }
    pub(super) fn expression(
        &mut self,
        body: &flow::Query,
        condition: &super::expressions::Condition,
        domain: &mut crate::value_access::evaluation::Types,
    ) -> Result<bool, BoundCheckFailure> {
        condition.supports(body, self.caller, &self.control, self.work, domain)
    }
    pub(super) fn drop(
        &mut self,
        body: &flow::Query,
        condition: &drops::Condition,
        domain: &mut crate::value_access::evaluation::Types,
    ) -> Result<bool, BoundCheckFailure> {
        condition.supports(body, self.caller, &self.control, self.work, domain)
    }
    pub(super) fn cleanup(
        &mut self,
        body: &flow::Query,
        condition: &cleanup::Condition,
    ) -> Result<nocter_declarations::value_access::DataInstanceKey, BoundCheckFailure> {
        use std::collections::btree_map::Entry;
        let request = condition.request()?;
        let projected = match self.resources.entry(request) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(
                cleanup::project(
                    body,
                    self.caller,
                    request.0,
                    request.1,
                    &self.control,
                    self.work,
                )
                .map_err(|failure| match failure {
                    crate::value_access::ResourceDataFailure::Integrity => {
                        BoundCheckFailure::Integrity(super::INVALID)
                    }
                    crate::value_access::ResourceDataFailure::Instantiation(failure) => {
                        BoundCheckFailure::Data {
                            at: request.0,
                            failure,
                        }
                    }
                })?,
            ),
        };
        condition.projected_data(projected).map_err(Into::into)
    }
    pub(super) fn resolve(
        &mut self,
        expression: AccessExpressionId,
    ) -> Result<Option<&CodeInstanceKey>, BoundCheckFailure> {
        use std::collections::btree_map::Entry;
        let call = match self.calls.entry(expression) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(
                instances::condition_call(
                    self.query,
                    self.caller,
                    expression,
                    Some(&self.control),
                    self.work,
                )
                .map_err(|failure| BoundCheckFailure::Invocation {
                    expression,
                    failure,
                })?,
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
        let query = self.body.typed();
        let control = crate::value_access::results::instantiate_control(
            query.program(),
            query.result_evidence.as_ref().ok_or(super::INVALID)?,
            Some(&self.body),
            caller,
            work,
        )
        .map_err(BoundCheckFailure::Control)?;
        let mut calls = BoundContext {
            query: self.body.typed(),
            caller,
            work,
            calls: std::collections::BTreeMap::new(),
            resources: std::collections::BTreeMap::new(),
            control,
        };
        if let Some(violation) = self.check_clauses(available, Some(caller), Some(&mut calls))? {
            return Ok(BoundCheck::Violation(violation));
        }
        Ok(BoundCheck::Calls {
            calls: calls
                .calls
                .into_iter()
                .filter_map(|(expression, call)| call.map(|call| (expression, call)))
                .collect(),
            control: calls.control,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nocter_constant_evaluation::state::{Limits, WorkBudget};
    use nocter_declarations::value_access::FunctionWitness;
    use nocter_test_support::ValueSourceFixture;
    use std::{collections::BTreeMap, num::NonZeroU64};

    #[test]
    fn invocation_cache_reuses_the_original_frame_after_metadata_work_is_exhausted() {
        let source = ValueSourceFixture::new(
            "const func identity(value: i32): i32 { value }\nconst func apply(action: func(i32): i32): i32 { action(42) }",
        );
        let unit = crate::value_access::accepted::SourceUnit::prepare(source.syntax())
            .unwrap()
            .into_unit();
        let program = unit.program();
        let code = |name| {
            program
                .resolve_code(program.symbols().get(name).unwrap())
                .unwrap()
        };
        let apply = code("apply");
        let formal = program.codes().get(apply).unwrap().parameters()[0];
        let caller = program
            .bind_code(
                apply.into(),
                [(formal.into(), FunctionWitness::from(code("identity")))],
            )
            .unwrap();
        let body = unit.code_body(apply.into()).unwrap();
        let expression = unit
            .const_requirements(apply.into())
            .unwrap()
            .completed_calls()
            .next()
            .unwrap()
            .expression();
        let mut work = WorkBudget::new(Limits::default().steps());
        let mut context = BoundContext {
            query: body.typed(),
            caller: &caller,
            work: &mut work,
            calls: BTreeMap::new(),
            resources: BTreeMap::new(),
            control: unit
                .instantiate_control(&caller, &mut WorkBudget::new(Limits::default().steps()))
                .unwrap(),
        };
        let selected = context.resolve(expression).unwrap().unwrap().clone();
        *context.work = WorkBudget::new(NonZeroU64::new(1).unwrap());
        context.work.step().unwrap();
        assert_eq!(context.resolve(expression).unwrap(), Some(&selected));
        assert_eq!(context.calls.len(), 1);
    }

    #[test]
    fn cleanup_cache_publishes_only_complete_projections_and_reuses_them_without_work() {
        let source = ValueSourceFixture::new(
            "func first(): void {}\nconst func discard(take action: func(): void): void {}",
        );
        let unit = crate::value_access::accepted::SourceUnit::prepare(source.syntax())
            .unwrap()
            .into_unit();
        let program = unit.program();
        let code = |name| {
            program
                .resolve_code(program.symbols().get(name).unwrap())
                .unwrap()
        };
        let discard = code("discard");
        let formal = program.codes().get(discard).unwrap().parameters()[0];
        let caller = program
            .bind_code(
                discard.into(),
                [(formal.into(), FunctionWitness::from(code("first")))],
            )
            .unwrap();
        let body = unit.code_body(discard.into()).unwrap();
        let conditions =
            cleanup::collect(body, &mut crate::value_access::evaluation::Types::default()).unwrap();
        assert_eq!(conditions.len(), 1);
        let condition = &conditions[0];
        let mut work = WorkBudget::new(NonZeroU64::new(1).unwrap());
        work.step().unwrap();
        let mut calls = BoundContext {
            query: body.typed(),
            caller: &caller,
            work: &mut work,
            calls: BTreeMap::new(),
            resources: BTreeMap::new(),
            control: unit
                .instantiate_control(&caller, &mut WorkBudget::new(Limits::default().steps()))
                .unwrap(),
        };
        assert!(matches!(
            calls.cleanup(body, condition),
            Err(BoundCheckFailure::Data {
                failure: crate::value_access::InstantiationFailure::WorkLimit,
                ..
            })
        ));
        assert!(calls.resources.is_empty());
        *calls.work = WorkBudget::new(Limits::default().steps());
        let selected = calls.cleanup(body, condition).unwrap();
        assert_eq!(calls.resources.len(), 1);
        *calls.work = WorkBudget::new(NonZeroU64::new(1).unwrap());
        calls.work.step().unwrap();
        assert_eq!(calls.cleanup(body, condition).unwrap(), selected);
        assert_eq!(calls.resources.len(), 1);
    }
}
