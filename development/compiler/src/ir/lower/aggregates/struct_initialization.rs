use std::collections::HashMap;

use crate::abi::{AbiField, layout_struct};
use crate::ast::StructLiteralExpr;
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolValue, Instruction};

use crate::ir::lower::context::{
    AggregateDrop, DropObligation, LoweringContext, StructFieldDropState,
};
use crate::ir::lower::expressions::TemporaryAllocator;
use crate::ir::{BoolLocation, UsizeLocation};

use super::{ArrayInitializationProgress, partial_drop_obligation_for_initializer};
use super::{PayloadInitializationProgress, initialize_struct_field_states};

/// Runtime ownership state for a struct while its fields are initialized.
///
/// Flags follow source initialization order rather than declaration order.
/// Failure cleanup can therefore unwind precisely the completed initializer
/// prefix even when a literal names its fields in a different order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::ir::lower) struct StructInitializationProgress {
    fields: Vec<StructFieldDropState>,
}

impl StructInitializationProgress {
    pub(in crate::ir::lower) fn new(
        abi_fields: &[AbiField],
        literal: &StructLiteralExpr,
        drop_kind: &AggregateDrop,
        context: &mut LoweringContext,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::with_allocator(abi_fields, literal, drop_kind, context)
    }

    pub(in crate::ir::lower) fn new_with_temporaries(
        abi_fields: &[AbiField],
        literal: &StructLiteralExpr,
        drop_kind: &AggregateDrop,
        temporaries: &mut TemporaryAllocator,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::with_allocator(abi_fields, literal, drop_kind, temporaries)
    }

    pub(in crate::ir::lower) fn with_allocator(
        abi_fields: &[AbiField],
        literal: &StructLiteralExpr,
        drop_kind: &AggregateDrop,
        allocator: &mut impl DropStateAllocator,
    ) -> Result<Self, Vec<Diagnostic>> {
        let owned_fields = match drop_kind {
            AggregateDrop::Direct(_) => HashMap::new(),
            AggregateDrop::Struct(drop_) => drop_
                .fields
                .iter()
                .map(|field| (field.offset, field.drop_kind.as_ref()))
                .collect::<HashMap<_, _>>(),
            AggregateDrop::Array(_) | AggregateDrop::PayloadEnum(_) => {
                return Err(invalid_struct_initialization_state_diagnostic());
            }
        };
        let layout = layout_struct(abi_fields)
            .map_err(|_error| invalid_struct_initialization_state_diagnostic())?;
        let field_types_and_offsets = abi_fields
            .iter()
            .zip(layout.fields.iter())
            .map(|(field, layout)| {
                u32::try_from(layout.offset)
                    .map(|offset| (field.name.as_str(), (&field.ty, offset)))
                    .map_err(|_error| invalid_struct_initialization_state_diagnostic())
            })
            .collect::<Result<HashMap<_, _>, _>>()?;

        let mut fields = Vec::with_capacity(owned_fields.len());
        for field in &literal.fields {
            let Some((field_type, offset)) = field_types_and_offsets
                .get(field.name.as_str())
                .map(|(field_type, offset)| (*field_type, *offset))
            else {
                return Err(invalid_struct_initialization_state_diagnostic());
            };
            if let Some(drop_kind) = owned_fields.get(&offset) {
                let initialized = allocator.next_drop_bool()?;
                let partial = Box::new(partial_drop_obligation_for_initializer(
                    &field.value,
                    field_type,
                    drop_kind,
                    allocator,
                )?);
                fields.push(StructFieldDropState {
                    offset,
                    initialized,
                    partial,
                });
            }
        }
        if fields.len() != owned_fields.len() {
            return Err(invalid_struct_initialization_state_diagnostic());
        }
        Ok(Self { fields })
    }

    pub(in crate::ir::lower) fn drop_states(&self) -> Vec<StructFieldDropState> {
        self.fields.clone()
    }

    pub(in crate::ir::lower) fn from_drop_states(fields: Vec<StructFieldDropState>) -> Self {
        Self { fields }
    }

    pub(in crate::ir::lower) fn initialize(&self) -> Vec<Instruction> {
        initialize_struct_field_states(&self.fields)
    }

    pub(in crate::ir::lower) fn complete_field(&self, offset: u32) -> Option<Instruction> {
        self.fields
            .iter()
            .find(|field| field.offset == offset)
            .map(|field| Instruction::SetBool {
                destination: field.initialized,
                value: BoolValue::Const(true),
            })
    }

    pub(in crate::ir::lower) fn array_field_progress(
        &self,
        offset: u32,
    ) -> Option<ArrayInitializationProgress> {
        let field = self.fields.iter().find(|field| field.offset == offset)?;
        let DropObligation::ArrayPrefix {
            initialized,
            elements,
        } = field.partial.as_ref()
        else {
            return None;
        };
        Some(ArrayInitializationProgress::from_drop_state(
            *initialized,
            elements.clone(),
        ))
    }

    pub(in crate::ir::lower) fn struct_field_progress(&self, offset: u32) -> Option<Self> {
        let field = self.fields.iter().find(|field| field.offset == offset)?;
        let DropObligation::StructFields { fields } = field.partial.as_ref() else {
            return None;
        };
        Some(Self {
            fields: fields.clone(),
        })
    }

    pub(in crate::ir::lower) fn payload_field_progress(
        &self,
        offset: u32,
    ) -> Option<PayloadInitializationProgress> {
        let field = self.fields.iter().find(|field| field.offset == offset)?;
        let DropObligation::PayloadFields { tag, fields } = field.partial.as_ref() else {
            return None;
        };
        Some(PayloadInitializationProgress::from_drop_states(
            *tag,
            fields.clone(),
        ))
    }
}

pub(in crate::ir::lower) trait DropStateAllocator {
    fn next_drop_bool(&mut self) -> Result<BoolLocation, Vec<Diagnostic>>;
    fn next_drop_usize(&mut self) -> Result<UsizeLocation, Vec<Diagnostic>>;
}

impl DropStateAllocator for LoweringContext<'_> {
    fn next_drop_bool(&mut self) -> Result<BoolLocation, Vec<Diagnostic>> {
        self.reserve_drop_state_bool_local()
    }

    fn next_drop_usize(&mut self) -> Result<UsizeLocation, Vec<Diagnostic>> {
        self.reserve_drop_state_usize_local()
    }
}

impl DropStateAllocator for TemporaryAllocator {
    fn next_drop_bool(&mut self) -> Result<BoolLocation, Vec<Diagnostic>> {
        self.next_bool()
    }

    fn next_drop_usize(&mut self) -> Result<UsizeLocation, Vec<Diagnostic>> {
        self.next_usize()
    }
}

fn invalid_struct_initialization_state_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "IR v0 cannot establish struct field initialization state",
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::BoolLocation;

    #[test]
    fn completed_fields_are_marked_independently() {
        let progress = StructInitializationProgress {
            fields: vec![StructFieldDropState {
                offset: 8,
                initialized: BoolLocation::Local(3),
                partial: Box::new(DropObligation::Inactive),
            }],
        };

        assert_eq!(
            progress.initialize(),
            vec![Instruction::SetBool {
                destination: BoolLocation::Local(3),
                value: BoolValue::Const(false),
            }]
        );
        assert_eq!(
            progress.complete_field(8),
            Some(Instruction::SetBool {
                destination: BoolLocation::Local(3),
                value: BoolValue::Const(true),
            })
        );
        assert_eq!(progress.complete_field(4), None);
    }
}
