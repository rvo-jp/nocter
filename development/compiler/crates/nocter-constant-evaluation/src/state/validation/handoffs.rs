//! Protection is checked at its ordered before-effect, and completed at the invocation boundary.
use super::{Effect, INVALID, Operation, Program, Result, place};
use crate::state::{Input, Plan};
use std::collections::BTreeSet;

pub(super) struct Handoffs<'a> {
    inputs: Option<&'a [Input]>,
    protected: BTreeSet<usize>,
}
impl<'a> Handoffs<'a> {
    pub(super) fn new(operation: &'a Operation) -> Self {
        Self {
            inputs: if let Operation::Call { inputs, .. } = operation {
                Some(inputs)
            } else {
                None
            },
            protected: BTreeSet::new(),
        }
    }
    pub(super) fn protect(
        &mut self,
        program: &Program,
        plan: &Plan,
        effect: &Effect,
    ) -> Result<()> {
        let Effect::Invoke {
            source,
            inputs: positions,
        } = effect
        else {
            return Ok(());
        };
        let inputs = self.inputs.ok_or(INVALID)?;
        for (index, payload) in positions {
            let index = *index;
            let Input::Take(data) = inputs.get(index).ok_or(INVALID)? else {
                return Err(INVALID);
            };
            if place(program, plan, payload)? != *data
                || !self.protected.insert(index)
                || payload.slot != source.slot
                || !payload.fields.starts_with(&source.fields)
            {
                return Err(INVALID);
            }
        }
        Ok(())
    }
    pub(super) fn complete(&self) -> Result<()> {
        if self
            .inputs
            .unwrap_or_default()
            .iter()
            .enumerate()
            .any(|(index, input)| {
                matches!(input, Input::Take(_)) != self.protected.contains(&index)
            })
        {
            return Err(INVALID);
        }
        Ok(())
    }
}
