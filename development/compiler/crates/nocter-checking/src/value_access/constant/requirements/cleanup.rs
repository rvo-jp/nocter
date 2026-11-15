//! Original ordered cleanup selections, not a type/site reconstruction of a retired value.
use super::{INVALID, Rule, Site, Violation, evaluation, flow};
use crate::value_access::flow::resources::{self, Payload, Step};
use crate::value_access::{ResourceDataFailure, ResourceDataProjection};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{CodeInstanceKey, DataInstanceKey};
use nocter_model::TypeId;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Before,
    After,
}
#[derive(Clone, Debug)]
pub(super) struct Condition {
    violation: Violation,
    transition: usize,
    phase: Phase,
    step: usize,
}
impl Condition {
    pub(super) fn request(&self) -> Result<(flow::Location, usize), flow::IntegrityError> {
        let Site::Cleanup(at) = self.violation.site else {
            return Err(INVALID);
        };
        Ok((at, self.transition))
    }
    pub(super) fn violation(&self) -> Violation {
        self.violation
    }
    fn original<'a>(&self, body: &'a flow::Query) -> Result<&'a Step, flow::IntegrityError> {
        let Site::Cleanup(at) = self.violation.site else {
            return Err(INVALID);
        };
        let transition = body
            .access()
            .resources()
            .at(at)
            .nth(self.transition)
            .ok_or(INVALID)?;
        let steps = match self.phase {
            Phase::Before => transition.before_steps(),
            Phase::After => transition.after_steps(),
        };
        steps.get(self.step).ok_or(INVALID)
    }
    pub(super) fn data(&self, body: &flow::Query) -> Result<TypeId, flow::IntegrityError> {
        let Rule::UnsupportedCleanup(expected) = self.violation.rule() else {
            return Err(INVALID);
        };
        let data = payload(self.original(body)?).ok_or(INVALID)?.data();
        if data != expected {
            return Err(INVALID);
        }
        Ok(data)
    }
    pub(super) fn projected_data(
        &self,
        projected: &Projected,
    ) -> Result<DataInstanceKey, flow::IntegrityError> {
        let steps = match self.phase {
            Phase::Before => &projected.before,
            Phase::After => &projected.after,
        };
        let step = steps.get(self.step).ok_or(INVALID)?;
        Ok(payload(step).ok_or(INVALID)?.instance().clone())
    }
}
pub(super) struct Projected {
    before: Vec<Step<resources::InstantiatedPayload>>,
    after: Vec<Step<resources::InstantiatedPayload>>,
}
pub(super) fn project(
    body: &flow::Query,
    caller: &CodeInstanceKey,
    at: flow::Location,
    transition: usize,
    work: &mut WorkBudget,
) -> Result<Projected, ResourceDataFailure> {
    let query = body.typed();
    let evidence = query
        .result_evidence
        .as_ref()
        .ok_or(ResourceDataFailure::Integrity)?;
    let transition = body
        .access()
        .resources()
        .at(at)
        .nth(transition)
        .ok_or(ResourceDataFailure::Integrity)?;
    let mut projection =
        ResourceDataProjection::new(query.program(), evidence, Some(body), caller, at, work)?;
    let before = projection.steps(transition.before_steps())?;
    let after = projection.after_steps(transition.after_steps())?;
    Ok(Projected { before, after })
}
// The same role selector also applies to concretely projected resource steps. Replacements
// inspect the retired obligation; the supplied value has a different role even at the same type.
fn payload<P>(step: &Step<P>) -> Option<&P> {
    match step {
        Step::Dispose { payload, .. }
        | Step::DisposeArrayRemainder { payload, .. }
        | Step::DisposeArrayElements { payload, .. } => Some(payload),
        Step::ReplaceReferent { retired, .. } => Some(retired),
        Step::Capture { .. }
        | Step::AdaptFunction { .. }
        | Step::Transport { .. }
        | Step::Assemble { .. }
        | Step::Construct { .. }
        | Step::Receive { .. }
        | Step::Move { .. }
        | Step::MoveArrayElement { .. }
        | Step::ConsolidateArrayPrefix { .. }
        | Step::ReleaseContainer { .. }
        | Step::Invoke { .. } => None,
    }
}
pub(super) fn collect(
    body: &flow::Query,
    domain: &mut evaluation::Types,
) -> Result<Vec<Condition>, flow::IntegrityError> {
    let mut conditions = Vec::new();
    for (point, block) in body.graph().points().iter() {
        for action in 0..block.actions.len() {
            let at = flow::Location { point, action };
            for (transition, alternative) in body.access().resources().at(at).enumerate() {
                for (phase, steps) in [
                    (Phase::Before, alternative.before_steps()),
                    (Phase::After, alternative.after_steps()),
                ] {
                    for (step, operation) in steps.iter().enumerate() {
                        if let Some(payload) = payload::<Payload>(operation)
                            && !domain.supports(body.typed().program(), payload.data())?
                        {
                            conditions.push(Condition {
                                violation: Violation {
                                    site: Site::Cleanup(at),
                                    rule: Rule::UnsupportedCleanup(payload.data()),
                                },
                                transition,
                                phase,
                                step,
                            });
                        }
                    }
                }
            }
        }
    }
    Ok(conditions)
}
