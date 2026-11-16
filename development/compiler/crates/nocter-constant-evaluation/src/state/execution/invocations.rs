//! Explicit call continuations keep language depth independent of the host call stack.
use super::{Cell, Executor, Place, Plan, Result, Rule, Site, Terminator, Value};
use crate::state::{CodeInstance, budget::Lease};
use nocter_model::AccessFlowPointId;
use std::sync::Arc;

pub(super) struct PreparedCall {
    pub(super) destination: usize,
    pub(super) target: CodeInstance,
    pub(super) inputs: Vec<Cell>,
    pub(super) _storage: Lease,
    pub(super) _callee: Option<Value>,
}

pub(super) struct Pending {
    pub(super) alternative: usize,
    pub(super) call: PreparedCall,
    pub(super) _taken: Vec<Option<Value>>,
    pub(super) _taken_storage: Option<Lease>,
}

pub(super) struct Activation {
    code: CodeInstance,
    plan: Arc<Plan>,
    frame: usize,
    point: AccessFlowPointId,
    action: usize,
    saved_site: Option<Site>,
    pending: Option<Pending>,
}
impl Activation {
    pub(super) fn new(
        code: CodeInstance,
        plan: Arc<Plan>,
        frame: usize,
        saved_site: Option<Site>,
    ) -> Self {
        Self {
            code,
            point: plan.entry,
            plan,
            frame,
            action: 0,
            saved_site,
            pending: None,
        }
    }
}
enum Progress {
    Continue,
    Invoke,
    Return(Value),
}

impl Executor<'_, '_> {
    pub(super) fn invoke(&mut self, code: CodeInstance, inputs: &[Cell]) -> Result<Value> {
        let mut active = vec![self.enter(code, inputs)?];
        let result = (|| {
            self.conditions(code, &active[0].plan, active[0].frame)?;
            loop {
                let current = active.last_mut().ok_or(Rule::InvalidPlan)?;
                match self.advance(current)? {
                    Progress::Continue => {}
                    Progress::Invoke => {
                        let call = &current.pending.as_ref().ok_or(Rule::InvalidPlan)?.call;
                        let next = self.enter(call.target, &call.inputs)?;
                        active.push(next);
                        let next = active.last().ok_or(Rule::InvalidPlan)?;
                        self.conditions(next.code, &next.plan, next.frame)?;
                    }
                    Progress::Return(value) => {
                        let returned = active.pop().ok_or(Rule::InvalidPlan)?;
                        self.site = returned.saved_site;
                        self.memory.frames.pop();
                        let Some(caller) = active.last_mut() else {
                            return Ok(value);
                        };
                        self.complete(caller, value)?;
                    }
                }
            }
        })();
        if result.is_err() {
            while let Some(frame) = active.pop() {
                if self.failed_instance.is_none()
                    && self.site.is_some_and(|site| site.code == frame.code.source)
                {
                    self.failed_instance = Some(frame.code);
                }
                if self.failed_site.is_none() {
                    self.failed_site = self.site;
                }
                if let Some(site) = frame.saved_site {
                    self.trace.push(site);
                }
                self.site = frame.saved_site;
                self.memory.frames.pop();
            }
        }
        result
    }

    fn advance(&mut self, active: &mut Activation) -> Result<Progress> {
        let block = active
            .plan
            .points
            .get(active.point)
            .ok_or(Rule::InvalidPlan)?;
        if let Some(action) = block.actions.get(active.action) {
            self.site = Some(Site {
                code: active.code.source,
                point: active.point,
                action: active.action,
            });
            self.tick()?;
            if let Some(pending) = self.action(active.frame, action)? {
                active.pending = Some(pending);
                return Ok(Progress::Invoke);
            }
            active.action += 1;
            return Ok(Progress::Continue);
        }
        self.tick()?;
        active.point = match &block.terminator {
            Terminator::Jump(next) => *next,
            Terminator::Branch { condition, yes, no } => {
                let value = self.read(active.frame, condition)?;
                let super::Data::Bool(value) = value.data() else {
                    return Err(Rule::InvalidPlan);
                };
                if *value { *yes } else { *no }
            }
            Terminator::Return(place) => {
                let address = self.memory.address(&self.program, active.frame, place)?;
                let (_, slot) = address.local()?;
                let owners = &self.memory.frames[active.frame].owned;
                if owners.len() != 1 || owners.get(&slot).is_none_or(|holes| !holes.is_empty()) {
                    return Err(Rule::InvalidPlan);
                }
                return self.memory.read(&address).map(Progress::Return);
            }
            Terminator::Unreachable => return Err(Rule::ReachedUnreachable),
        };
        active.action = 0;
        Ok(Progress::Continue)
    }

    fn complete(&mut self, active: &mut Activation, value: Value) -> Result<()> {
        let pending = active.pending.take().ok_or(Rule::InvalidPlan)?;
        let destination = pending.call.destination;
        // Argument cells and selected-callee backing end with the invocation, before its write.
        drop(pending.call);
        self.write(active.frame, &Place::root(destination), value)?;
        let action = active
            .plan
            .points
            .get(active.point)
            .and_then(|point| point.actions.get(active.action))
            .ok_or(Rule::InvalidPlan)?;
        let alternative = action
            .alternatives
            .get(pending.alternative)
            .ok_or(Rule::InvalidPlan)?;
        for step in alternative.sequence(&action.operation) {
            if let crate::state::sequence::Step::After(effect) = step {
                self.tick()?;
                self.effect(active.frame, effect)?;
            }
        }
        active.action += 1;
        Ok(())
    }
}
