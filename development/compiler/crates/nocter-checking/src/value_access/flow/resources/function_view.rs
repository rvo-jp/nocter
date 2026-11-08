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
        let mut staged = self.clone();
        let from = staged.take(program, source)?;
        if !program.function_adapter(from.data(), payload.data()) {
            return Err(INVALID);
        }
        staged.insert(destination, payload, false)?;
        *self = staged;
        steps.push(Step::AdaptFunction {
            source: source.clone(),
            destination,
            from,
            payload,
        });
        Ok(())
    }
}
