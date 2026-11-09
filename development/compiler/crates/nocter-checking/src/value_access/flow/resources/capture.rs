//! Atomic transfer of one complete owned environment into a closure payload.
use super::{Holder, INVALID, Payload, Place, Program, Result, State, Step};

impl State {
    pub fn capture(
        &mut self,
        program: &Program,
        source: &Place,
        destination: Holder,
        payload: Payload,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let Some(nocter_declarations::value_access::PayloadSchema::Function(witness)) =
            program.data_schema(payload.data())
        else {
            return Err(INVALID);
        };
        let nocter_model::ValueFunctionOrigin::Closure(id) = witness.origin else {
            return Err(INVALID);
        };
        let declared = program.closure(id).ok_or(INVALID)?;
        let mut staged = self.clone();
        let environment = staged.take(program, source)?;
        if environment.data() != declared.environment() {
            return Err(INVALID);
        }
        staged.insert(destination, payload, false)?;
        *self = staged;
        steps.push(Step::Capture {
            source: source.clone(),
            destination,
            environment,
            payload,
        });
        Ok(())
    }
}
