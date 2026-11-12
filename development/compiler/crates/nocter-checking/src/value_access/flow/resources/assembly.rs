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
        let complete = match program.data_schema(payload.data()).ok_or(INVALID)? {
            PayloadSchema::Aggregate(aggregate) => fields.len() == aggregate.len(),
            PayloadSchema::Array(array) => u64::try_from(fields.len()).ok() == Some(array.length()),
            PayloadSchema::Optional(_) => fields.len() <= 1,
            PayloadSchema::Fallible(_) => fields.len() == 1,
            PayloadSchema::Builtin(_) | PayloadSchema::Function(_) => false,
        };
        if !complete || self.contains(destination) {
            return Err(INVALID);
        }
        let mut staged = self.clone();
        let mut selected = BTreeSet::new();
        for (field, source) in fields {
            let declaration = program.component(payload.data(), *field).ok_or(INVALID)?;
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
