use super::*;

pub(super) struct CallEvaluationContext<'base, 'context> {
    base: &'base LoweringContext<'context>,
    context: LoweringContext<'context>,
    reserved_temporary_words: usize,
}

impl<'base, 'context> CallEvaluationContext<'base, 'context> {
    pub(super) fn new(
        base: &'base LoweringContext<'context>,
        temporaries: &TemporaryAllocator,
    ) -> Result<Self, Vec<Diagnostic>> {
        let reserved_temporary_words = temporaries.reserved_local_abi_words(base)?;
        Ok(Self {
            base,
            context: base.with_reserved_local_abi_words(reserved_temporary_words),
            reserved_temporary_words,
        })
    }

    pub(super) fn sync_temporaries(
        &mut self,
        temporaries: &TemporaryAllocator,
    ) -> Result<(), Vec<Diagnostic>> {
        let reserved = temporaries.reserved_local_abi_words(self.base)?;
        let additional = reserved
            .checked_sub(self.reserved_temporary_words)
            .ok_or_else(|| vec![Diagnostic::error("E8006", "call temporary state regressed")])?;
        if additional > 0 {
            self.context = self.context.with_reserved_local_abi_words(additional);
            self.reserved_temporary_words = reserved;
        }
        Ok(())
    }

    pub(super) fn context(&self) -> &LoweringContext<'context> {
        &self.context
    }

    pub(super) fn register_array_prefix(
        &mut self,
        slot_index: usize,
        layout: ValueLayout,
        drop_kind: AggregateDrop,
        initialized: UsizeLocation,
    ) -> bool {
        self.context.register_temporary_array_prefix_drop(
            slot_index,
            layout,
            drop_kind,
            initialized,
        )
    }

    pub(super) fn register_struct_fields(
        &mut self,
        slot_index: usize,
        layout: ValueLayout,
        drop_kind: AggregateDrop,
        fields: Vec<StructFieldDropState>,
    ) -> bool {
        self.context
            .register_temporary_struct_fields_drop(slot_index, layout, drop_kind, fields)
    }

    pub(super) fn complete_temporary(
        &mut self,
        slot_index: usize,
        layout: ValueLayout,
        drop_kind: AggregateDrop,
    ) -> bool {
        self.context
            .register_or_complete_temporary_aggregate_drop(slot_index, layout, drop_kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::lower::context::FunctionSignatures;

    #[test]
    fn syncing_reserves_allocator_words_before_failure_payloads() {
        let context = LoweringContext::empty(
            "test".to_string(),
            Type::Void,
            FunctionSignatures::default(),
        );
        let mut temporaries = TemporaryAllocator::new(&context).unwrap();
        let mut evaluation = CallEvaluationContext::new(&context, &temporaries).unwrap();
        assert_eq!(temporaries.next_usize().unwrap(), UsizeLocation::Local(0));

        evaluation.sync_temporaries(&temporaries).unwrap();

        assert_eq!(
            evaluation.context().next_error_local_locations().unwrap(),
            (StrLocation::Local(1), StrLocation::Local(3))
        );
    }
}
