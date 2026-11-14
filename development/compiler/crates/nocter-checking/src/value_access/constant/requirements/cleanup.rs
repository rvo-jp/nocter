//! Original ordered cleanup selections, not a type/site reconstruction of a retired value.
use super::{INVALID, Rule, Site, Violation, evaluation, flow};
use crate::value_access::flow::resources::{Payload, Step};
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
