//! Request-local call and resource metadata for bound clauses, never recursive admission.
use super::{Requirements, Violation, cleanup, drops, flow, instances};
use nocter_declarations::value_access::CodeInstanceKey;
use nocter_model::{AccessExpressionId, ValueClosure};

/// An unresolved invocation capability guarded by the stable absence of its original input.
#[derive(Clone, Debug)]
pub(in crate::value_access) struct InvocationCondition {
    pub expression: AccessExpressionId,
    pub input: usize,
    pub fields: Box<[usize]>,
    pub cause: flow::Location,
    pub violation: Violation,
}
/// Selected call metadata and definition-only residuals; neither supplies const eligibility.
pub(in crate::value_access) enum BoundCheck {
    Violation(Violation),
    Calls {
        calls: std::collections::BTreeMap<AccessExpressionId, Box<[CodeInstanceKey]>>,
        control: crate::value_access::InstanceControl,
        conditions: std::collections::BTreeMap<AccessExpressionId, InvocationCondition>,
        /// Always empty in a strict instance report; these IDs are not selected call frames.
        residual_calls: std::collections::BTreeSet<AccessExpressionId>,
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
    calls: std::collections::BTreeMap<AccessExpressionId, Option<Box<[CodeInstanceKey]>>>,
    resources: std::collections::BTreeMap<(flow::Location, usize), cleanup::Projected>,
    control: crate::value_access::InstanceControl,
    conditions: std::collections::BTreeMap<AccessExpressionId, InvocationCondition>,
    residual_calls: std::collections::BTreeSet<AccessExpressionId>,
}
impl BoundContext<'_, '_> {
    pub(super) fn retain_input_capability(
        &mut self,
        body: &flow::Query,
        expression: AccessExpressionId,
        writes: Option<&std::collections::BTreeSet<nocter_model::AccessLocalId>>,
    ) -> Result<bool, BoundCheckFailure> {
        if self.residual_calls.contains(&expression) {
            return Ok(true);
        }
        let retained = super::guards::stable_callable_field_input(
            body,
            &self.control,
            expression,
            writes,
            self.work,
        )
        .map_err(|failure| BoundCheckFailure::Invocation {
            expression,
            failure,
        })?;
        if retained {
            self.residual_calls.insert(expression);
        }
        Ok(retained)
    }
    pub(super) fn defer_invocation(
        &mut self,
        body: &flow::Query,
        expression: AccessExpressionId,
        resource: Option<&super::guards::InputResource>,
        violation: Violation,
    ) -> Result<bool, BoundCheckFailure> {
        let Some(resource) = resource.filter(|resource| resource.stable) else {
            return Ok(false);
        };
        let local = resource.local;
        let target = match violation.rule() {
            super::super::Rule::RuntimeCall(target) => nocter_model::ValueCode::Declaration(target),
            super::super::Rule::CapturedCall(target) => nocter_model::ValueCode::Closure(target),
            _ => return Ok(false),
        };
        let Some(frames) = self.calls.get(&expression).and_then(Option::as_ref) else {
            return Err(super::INVALID.into());
        };
        let [frame] = frames.as_ref() else {
            return Ok(false);
        };
        if frame.code() != target {
            return Err(super::INVALID.into());
        }
        if !super::guards::derives_from_input(
            body,
            &self.control,
            expression,
            local,
            &resource.fields,
            self.work,
        )
        .map_err(|failure| BoundCheckFailure::Invocation {
            expression,
            failure,
        })? {
            return Ok(false);
        }
        let evidence = self.query.result_evidence.as_ref().ok_or(super::INVALID)?;
        match crate::value_access::results::instantiate_call_boundary(
            self.query.program(),
            evidence,
            None,
            frame,
            self.work,
        ) {
            Ok(_) => {}
            Err(crate::value_access::InstantiationFailure::WorkLimit) => {
                return Err(BoundCheckFailure::Invocation {
                    expression,
                    failure: crate::value_access::InstantiationFailure::WorkLimit,
                });
            }
            Err(_) => return Ok(false),
        }
        let input = body
            .typed()
            .body()
            .inputs()
            .ordered()
            .position(|input| input.local == local)
            .ok_or(super::INVALID)?;
        let sources = evidence
            .value_sources(self.caller.code())
            .ok_or(super::INVALID)?;
        let mut cause = None;
        for call in sources.call(expression).ok_or(super::INVALID)? {
            self.work
                .step()
                .map_err(|_| BoundCheckFailure::Invocation {
                    expression,
                    failure: crate::value_access::InstantiationFailure::WorkLimit,
                })?;
            if self.control.reachable(call.origin.point) {
                cause = Some(call.origin);
                break;
            }
        }
        let cause = cause.ok_or(super::INVALID)?;
        self.conditions.insert(
            expression,
            InvocationCondition {
                expression,
                input,
                fields: resource.fields.clone(),
                cause,
                violation,
            },
        );
        Ok(true)
    }
    pub(super) fn interrupted_invocation(
        &mut self,
        expression: AccessExpressionId,
        selections: &[(flow::Location, crate::value_access::ValueSourceId)],
        available: &dyn Fn(ValueClosure) -> bool,
    ) -> Result<Option<Violation>, BoundCheckFailure> {
        let typed = self
            .query
            .body()
            .expressions()
            .get(expression)
            .ok_or(super::INVALID)?;
        if !super::super::interrupted_contract_call(&typed.kind) {
            return Err(super::INVALID.into());
        }
        let sources = self.query.result_evidence.as_ref().ok_or(super::INVALID)?;
        let program = self.query.program();
        let mut resolution = crate::value_access::results::InstantiationQuery::with_control(
            program,
            sources,
            self.work,
            &self.control,
        );
        let mut found = false;
        for &(at, source) in selections {
            if !self.control.reachable(at.point) {
                continue;
            }
            found = true;
            let witness = match resolution.value(self.caller, source) {
                Ok(witness) => witness,
                Err(crate::value_access::InstantiationFailure::WorkLimit) => {
                    return Err(BoundCheckFailure::Invocation {
                        expression,
                        failure: crate::value_access::InstantiationFailure::WorkLimit,
                    });
                }
                Err(_) => {
                    return Ok(Some(Violation {
                        site: super::Site::Expression(expression),
                        rule: super::Rule::ValueInvocation,
                    }));
                }
            };
            if let Some(rule) = super::super::operation_rule(
                program,
                &typed.kind,
                available,
                Some(witness.witness().origin),
            )? {
                return Ok(Some(Violation {
                    site: super::Site::Expression(expression),
                    rule,
                }));
            }
        }
        Ok((!found).then_some(Violation {
            site: super::Site::Expression(expression),
            rule: super::Rule::ValueInvocation,
        }))
    }
    pub(super) fn active(&self, clause: &super::Clause) -> Result<bool, BoundCheckFailure> {
        Ok(match clause {
            super::Clause::Invocation {
                receiver: Some(_), ..
            } => true,
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
    pub(super) fn resolve_all(
        &mut self,
        expression: AccessExpressionId,
    ) -> Result<Option<&[CodeInstanceKey]>, BoundCheckFailure> {
        use std::collections::btree_map::Entry;
        let call = match self.calls.entry(expression) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(
                instances::condition_calls(
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
        Ok(call.as_deref())
    }
}

impl Requirements {
    /// Definition-only metadata retains unresolved payload clauses for exact-instance admission.
    /// The returned selections carry no executable frame or const-eligibility receipt.
    pub(in crate::value_access) fn check_definition(
        &self,
        available: &dyn Fn(ValueClosure) -> bool,
        caller: &CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<BoundCheck, BoundCheckFailure> {
        self.check_in(available, caller, work, super::CheckMode::Definition)
    }
    fn bound_context<'a, 'work>(
        &'a self,
        caller: &'a CodeInstanceKey,
        work: &'work mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<BoundContext<'a, 'work>, BoundCheckFailure> {
        let query = self.body.typed();
        let control = crate::value_access::results::instantiate_control(
            query.program(),
            query.result_evidence.as_ref().ok_or(super::INVALID)?,
            Some(&self.body),
            caller,
            work,
        )
        .map_err(BoundCheckFailure::Control)?;
        Ok(BoundContext {
            query: self.body.typed(),
            caller,
            work,
            calls: std::collections::BTreeMap::new(),
            resources: std::collections::BTreeMap::new(),
            conditions: std::collections::BTreeMap::new(),
            residual_calls: std::collections::BTreeSet::new(),
            control,
        })
    }
    pub(in crate::value_access) fn check_bound(
        &self,
        available: &dyn Fn(ValueClosure) -> bool,
        caller: &CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<BoundCheck, BoundCheckFailure> {
        self.check_in(available, caller, work, super::CheckMode::Exact)
    }
    fn check_in(
        &self,
        available: &dyn Fn(ValueClosure) -> bool,
        caller: &CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
        mode: super::CheckMode,
    ) -> Result<BoundCheck, BoundCheckFailure> {
        let mut calls = self.bound_context(caller, work)?;
        if let Some(violation) =
            self.check_clauses(available, Some(caller), Some(&mut calls), mode)?
        {
            return Ok(BoundCheck::Violation(violation));
        }
        Ok(BoundCheck::Calls {
            calls: calls
                .calls
                .into_iter()
                .filter_map(|(expression, call)| call.map(|call| (expression, call)))
                .collect(),
            control: calls.control,
            conditions: calls.conditions,
            residual_calls: calls.residual_calls,
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
    fn readonly_field_definition_residuals_cannot_become_strict_instance_calls() {
        let fixture = ValueSourceFixture::new(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/native/value-access/received-record-callable-conditions.nct"
        )));
        let unit = crate::value_access::accepted::SourceUnit::prepare(fixture.syntax())
            .unwrap()
            .into_unit();
        let program = unit.program();
        let invoke = program
            .resolve(program.symbols().get("invoke").unwrap())
            .unwrap();
        let hypothesis = program.bind_code(invoke.into(), []).unwrap();
        assert!(hypothesis.input_data(0).is_none());
        let requirements = unit.const_requirements(invoke.into()).unwrap();
        let mut work = WorkBudget::new(Limits::default().steps());
        let BoundCheck::Calls {
            calls,
            residual_calls,
            ..
        } = requirements
            .check_definition(&|_| true, &hypothesis, &mut work)
            .unwrap()
        else {
            panic!("a stable field can retain an explicit definition capability condition");
        };
        assert_eq!(residual_calls.len(), 1);
        assert!(residual_calls.iter().all(|call| !calls.contains_key(call)));
        let mut work = WorkBudget::new(Limits::default().steps());
        assert!(matches!(
            requirements.check_bound(&|_| true, &hypothesis, &mut work),
            Err(BoundCheckFailure::Invocation {
                failure: crate::value_access::InstantiationFailure::Unresolved,
                ..
            })
        ));
    }
    #[test]
    fn definition_payload_residuals_never_defer_work_exhaustion_and_can_retry() {
        let fixture = ValueSourceFixture::new(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/native/value-access/contextual-scoped-record-input-growth.nct"
        )));
        let unit = crate::value_access::accepted::SourceUnit::prepare(fixture.syntax())
            .unwrap()
            .into_unit();
        let program = unit.program();
        let inside = program
            .resolve(program.symbols().get("inside").unwrap())
            .unwrap();
        let hypothesis = program.bind_code(inside.into(), []).unwrap();
        assert!(hypothesis.input_data(0).is_none());
        let requirements = unit.const_requirements(inside.into()).unwrap();
        let mut work = WorkBudget::new(NonZeroU64::new(1).unwrap());
        work.step().unwrap();
        let Err(failure) = requirements.check_definition(&|_| true, &hypothesis, &mut work) else {
            panic!("definition scanning must not defer metadata exhaustion");
        };
        assert!(matches!(
            failure,
            BoundCheckFailure::Control(crate::value_access::InstantiationFailure::WorkLimit)
                | BoundCheckFailure::Data {
                    failure: crate::value_access::InstantiationFailure::WorkLimit,
                    ..
                }
                | BoundCheckFailure::Invocation {
                    failure: crate::value_access::InstantiationFailure::WorkLimit,
                    ..
                }
        ));
        let mut work = WorkBudget::new(Limits::default().steps());
        let BoundCheck::Calls { calls, .. } = requirements
            .check_definition(&|_| true, &hypothesis, &mut work)
            .unwrap()
        else {
            panic!("a healthy negative scan can retain payload conditions");
        };
        assert!(!calls.is_empty());
    }
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
            conditions: BTreeMap::new(),
            residual_calls: std::collections::BTreeSet::new(),
            control: unit
                .instantiate_control(&caller, &mut WorkBudget::new(Limits::default().steps()))
                .unwrap(),
        };
        let selected = context.resolve_all(expression).unwrap().unwrap().to_vec();
        *context.work = WorkBudget::new(NonZeroU64::new(1).unwrap());
        context.work.step().unwrap();
        assert_eq!(
            context.resolve_all(expression).unwrap(),
            Some(selected.as_slice())
        );
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
            conditions: BTreeMap::new(),
            residual_calls: std::collections::BTreeSet::new(),
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
