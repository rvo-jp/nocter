//! Assemble complete components in the existing ledger; interrupted construction owns temporaries.
use super::{Holder, INVALID, Payload, Place, Program, Result, State, Step};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::ValueField;
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub(in crate::value_access::flow) struct Assembly<'a> {
    pub tag: Option<u64>,
    pub fields: &'a [(ValueField, Place)],
}

impl State {
    pub fn assemble(
        &mut self,
        program: &Program,
        destination: Holder,
        payload: Payload,
        fields: &[(ValueField, Place)],
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        self.assemble_with(
            program,
            destination,
            payload,
            Assembly { tag: None, fields },
            steps,
            |actual, expected| actual == expected,
        )
    }
    pub(in crate::value_access::flow) fn assemble_with(
        &mut self,
        program: &Program,
        destination: Holder,
        payload: Payload,
        assembly: Assembly<'_>,
        steps: &mut Vec<Step>,
        compatible: impl Fn(nocter_model::TypeId, nocter_model::TypeId) -> bool,
    ) -> Result<()> {
        let Assembly { tag, fields } = assembly;
        let schema = program.data_schema(payload.data()).ok_or(INVALID)?;
        let complete = match (schema, tag) {
            (PayloadSchema::Enumeration(schema), Some(tag)) => {
                let case = *schema
                    .variants()
                    .get(usize::try_from(tag).map_err(|_| INVALID)?)
                    .ok_or(INVALID)?;
                program
                    .variant_components(payload.data(), case)
                    .ok_or(INVALID)?
                    .map(|component| component.field)
                    .eq(fields.iter().map(|&(field, _)| field))
            }
            (PayloadSchema::Aggregate(aggregate), None) => fields.len() == aggregate.len(),
            (PayloadSchema::Array(array), None) => {
                u64::try_from(fields.len()).ok() == Some(array.length())
            }
            (PayloadSchema::Optional(_), None) => fields.len() <= 1,
            (PayloadSchema::Fallible(_), None) => fields.len() == 1,
            _ => false,
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
            if !compatible(component.data(), declaration.data) {
                return Err(INVALID);
            }
        }
        staged.insert(destination, payload, false)?;
        *self = staged;
        steps.push(Step::Assemble {
            destination,
            payload,
            enum_case: tag,
            fields: fields.into(),
        });
        Ok(())
    }
}
