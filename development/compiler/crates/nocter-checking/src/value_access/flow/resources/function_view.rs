//! Preserve the same code witness and ownership obligation through a selected invocation adapter.
use super::{Holder, INVALID, Payload, Place, Program, Result, State, Step};

impl State {
    pub fn adapt_function(
        &mut self,
        program: &Program,
        source: &Place,
        destination: Holder,
        payload: Payload,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        self.reclassify(program, source, destination, payload, steps, |from| {
            if !program.function_binding(from.data(), payload.data()) {
                return Err(INVALID);
            }
            Ok(Step::AdaptFunction {
                source: source.clone(),
                destination,
                from,
                payload,
            })
        })
    }
}
