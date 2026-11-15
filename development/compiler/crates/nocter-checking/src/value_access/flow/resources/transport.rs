//! One existing owner moves between independently selected representation contracts.
use super::{Holder, INVALID, Payload, Place, Program, Result, State, Step};

impl State {
    pub(super) fn reclassify(
        &mut self,
        program: &Program,
        source: &Place,
        destination: Holder,
        payload: Payload,
        steps: &mut Vec<Step>,
        transition: impl FnOnce(Payload) -> Result<Step>,
    ) -> Result<()> {
        let mut staged = self.clone();
        let from = staged.take(program, source)?;
        let step = transition(from)?;
        let (recorded_source, recorded_destination, recorded_from, recorded_payload) = match &step {
            Step::AdaptFunction {
                source,
                destination,
                from,
                payload,
            }
            | Step::Transport {
                source,
                destination,
                from,
                payload,
            } => (source, *destination, *from, *payload),
            _ => return Err(INVALID),
        };
        if recorded_source != source
            || recorded_destination != destination
            || recorded_from != from
            || recorded_payload != payload
        {
            return Err(INVALID);
        }
        staged.insert(destination, payload, false)?;
        *self = staged;
        steps.push(step);
        Ok(())
    }
}
