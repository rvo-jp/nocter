//! Assemble complete components in the existing ledger; interrupted construction owns temporaries.
use super::{Holder, INVALID, Payload, Place, Program, Result, State, Step};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::ValueField;
use std::collections::BTreeSet;

impl State {
    pub fn assemble(
        &mut self,
        program: &Program,
        destination: Holder,
        payload: Payload,
        fields: &[(ValueField, Place)],
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let PayloadSchema::Aggregate(aggregate) =
            program.data_schema(payload.data()).ok_or(INVALID)?
        else {
            return Err(INVALID);
        };
        if fields.len() != aggregate.len() {
            return Err(INVALID);
        }
        let mut staged = self.clone();
        let mut selected = BTreeSet::new();
        for (field, source) in fields {
            let declaration = aggregate.component(*field).ok_or(INVALID)?;
            if !selected.insert(*field) {
                return Err(INVALID);
            }
            let component = staged.take(program, source)?;
            if component.data() != declaration.data {
                return Err(INVALID);
            }
        }
        staged.insert(destination, payload, false)?;
        *self = staged;
        steps.push(Step::Assemble {
            destination,
            payload,
            fields: fields.into(),
        });
        Ok(())
    }
}
