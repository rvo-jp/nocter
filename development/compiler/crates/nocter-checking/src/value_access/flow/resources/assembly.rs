//! Assemble complete components in the existing ledger; interrupted construction owns temporaries.
use super::{Holder, INVALID, Payload, Place, Program, Result, State, Step};
use nocter_declarations::value_access::{PayloadSchema, Record};
use nocter_model::AccessFieldId;
use std::collections::BTreeSet;

impl State {
    pub fn assemble(
        &mut self,
        program: &Program,
        destination: Holder,
        payload: Payload,
        fields: &[(AccessFieldId, Place)],
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let PayloadSchema::Record(record) = program.data_schema(payload.data()).ok_or(INVALID)?
        else {
            return Err(INVALID);
        };
        if fields.len() != record.fields().len() {
            return Err(INVALID);
        }
        let mut staged = self.clone();
        let mut selected = BTreeSet::new();
        for (field, source) in fields {
            let declaration = program.fields().get(*field).ok_or(INVALID)?;
            if program.records().get(declaration.owner()).map(Record::data) != Some(payload.data())
                || !selected.insert(*field)
            {
                return Err(INVALID);
            }
            let component = staged.take(program, source)?;
            if component.data() != declaration.data() {
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
