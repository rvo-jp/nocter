use super::*;

impl LoweringContext<'_> {
    pub(in crate::ir::lower) fn define_outcome_local_at_slot(
        &mut self,
        name: String,
        slot_index: usize,
        storage: OutcomeStorageLayout,
        payload_type: Type,
        is_copy: bool,
    ) {
        self.locals.push(LocalBinding {
            name,
            kind: LocalKind::Outcome(OutcomeLocal {
                slot_index,
                storage,
                payload_type,
                is_copy,
            }),
            index: 0,
        });
    }

    pub(in crate::ir::lower) fn outcome_local(&self, name: &str) -> Option<OutcomeLocal> {
        self.locals.iter().find_map(|local| match &local.kind {
            LocalKind::Outcome(outcome) if local.name == name => Some(outcome.clone()),
            _ => None,
        })
    }
}
