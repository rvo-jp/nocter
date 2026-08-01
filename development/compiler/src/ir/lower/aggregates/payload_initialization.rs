use std::collections::HashMap;

use crate::abi::{AbiEnum, AbiType, layout_struct};
use crate::ast::Expr;
use crate::diagnostics::Diagnostic;
use crate::ir::lower::context::{
    AggregateDrop, DropObligation, PayloadFieldDropState, StructFieldDropState,
};
use crate::ir::{BoolValue, Instruction};

use super::{
    ArrayInitializationProgress, DropStateAllocator, StructInitializationProgress,
    partial_drop_obligation_for_initializer, payload_enum_constructor_member_and_arguments,
};

/// Runtime ownership state for the selected payload enum variant while its
/// arguments are initialized from left to right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::ir::lower) struct PayloadInitializationProgress {
    tag: u8,
    fields: Vec<PayloadFieldDropState>,
}

impl PayloadInitializationProgress {
    pub(in crate::ir::lower) fn with_allocator(
        expression: &Expr,
        enum_: &AbiEnum,
        drop_kind: &AggregateDrop,
        allocator: &mut impl DropStateAllocator,
    ) -> Result<Self, Vec<Diagnostic>> {
        let Some((member, arguments)) = payload_enum_constructor_member_and_arguments(expression)
        else {
            return Err(invalid_payload_initialization_state_diagnostic());
        };
        let Some(variant) = enum_
            .variants
            .iter()
            .find(|variant| variant.name == member.member)
        else {
            return Err(invalid_payload_initialization_state_diagnostic());
        };
        let AggregateDrop::PayloadEnum(drop_) = drop_kind else {
            return Err(invalid_payload_initialization_state_diagnostic());
        };
        let owned_fields = drop_
            .variants
            .iter()
            .find(|drop_variant| drop_variant.tag == variant.tag)
            .map(|variant| {
                variant
                    .fields
                    .iter()
                    .map(|field| (field.payload_offset, field.drop_kind.as_ref()))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let fields_and_offsets = payload_fields_and_offsets(
            variant.payload.as_ref(),
            arguments.len(),
            enum_.payload_offset,
        )?;

        let mut fields = Vec::with_capacity(owned_fields.len());
        for ((field_type, payload_offset), argument) in
            fields_and_offsets.into_iter().zip(arguments.iter())
        {
            let Some(field_drop_kind) = owned_fields.get(&payload_offset) else {
                continue;
            };
            let initialized = allocator.next_drop_bool()?;
            let partial = Box::new(partial_drop_obligation_for_initializer(
                argument,
                field_type,
                field_drop_kind,
                allocator,
            )?);
            fields.push(PayloadFieldDropState {
                payload_offset,
                initialized,
                partial,
            });
        }
        if fields.len() != owned_fields.len() {
            return Err(invalid_payload_initialization_state_diagnostic());
        }
        Ok(Self {
            tag: variant.tag,
            fields,
        })
    }

    pub(in crate::ir::lower) fn tag(&self) -> u8 {
        self.tag
    }

    pub(in crate::ir::lower) fn drop_states(&self) -> Vec<PayloadFieldDropState> {
        self.fields.clone()
    }

    pub(in crate::ir::lower) fn from_drop_states(
        tag: u8,
        fields: Vec<PayloadFieldDropState>,
    ) -> Self {
        Self { tag, fields }
    }

    pub(in crate::ir::lower) fn drop_obligation(&self) -> DropObligation {
        DropObligation::PayloadFields {
            tag: self.tag,
            fields: self.drop_states(),
        }
    }

    pub(in crate::ir::lower) fn initialize(&self) -> Vec<Instruction> {
        initialize_payload_field_states(&self.fields)
    }

    pub(in crate::ir::lower) fn complete_field(&self, offset: u32) -> Option<Instruction> {
        self.fields
            .iter()
            .find(|field| field.payload_offset == offset)
            .map(|field| Instruction::SetBool {
                destination: field.initialized,
                value: BoolValue::Const(true),
            })
    }

    pub(in crate::ir::lower) fn array_field_progress(
        &self,
        offset: u32,
    ) -> Option<ArrayInitializationProgress> {
        let field = self
            .fields
            .iter()
            .find(|field| field.payload_offset == offset)?;
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

    pub(in crate::ir::lower) fn struct_field_progress(
        &self,
        offset: u32,
    ) -> Option<StructInitializationProgress> {
        let field = self
            .fields
            .iter()
            .find(|field| field.payload_offset == offset)?;
        let DropObligation::StructFields { fields } = field.partial.as_ref() else {
            return None;
        };
        Some(StructInitializationProgress::from_drop_states(
            fields.clone(),
        ))
    }

    pub(in crate::ir::lower) fn payload_field_progress(&self, offset: u32) -> Option<Self> {
        let field = self
            .fields
            .iter()
            .find(|field| field.payload_offset == offset)?;
        let DropObligation::PayloadFields { tag, fields } = field.partial.as_ref() else {
            return None;
        };
        Some(Self::from_drop_states(*tag, fields.clone()))
    }
}

pub(in crate::ir::lower) fn initialize_payload_field_states(
    fields: &[PayloadFieldDropState],
) -> Vec<Instruction> {
    super::initialize_drop_obligation(&DropObligation::PayloadFields {
        tag: 0,
        fields: fields.to_vec(),
    })
}

pub(in crate::ir::lower) fn initialize_struct_field_states(
    fields: &[StructFieldDropState],
) -> Vec<Instruction> {
    super::initialize_drop_obligation(&DropObligation::StructFields {
        fields: fields.to_vec(),
    })
}

fn payload_fields_and_offsets(
    payload: Option<&AbiType>,
    argument_count: usize,
    payload_offset: u64,
) -> Result<Vec<(&AbiType, u32)>, Vec<Diagnostic>> {
    let Some(payload) = payload else {
        return if argument_count == 0 {
            Ok(Vec::new())
        } else {
            Err(invalid_payload_initialization_state_diagnostic())
        };
    };
    let base_offset = u32::try_from(payload_offset)
        .map_err(|_error| invalid_payload_initialization_state_diagnostic())?;
    if argument_count == 1 {
        return Ok(vec![(payload, base_offset)]);
    }
    let AbiType::Struct(fields) = payload else {
        return Err(invalid_payload_initialization_state_diagnostic());
    };
    if fields.len() != argument_count {
        return Err(invalid_payload_initialization_state_diagnostic());
    }
    let layout = layout_struct(fields)
        .map_err(|_error| invalid_payload_initialization_state_diagnostic())?;
    fields
        .iter()
        .zip(layout.fields.iter())
        .map(|(field, layout)| {
            base_offset
                .checked_add(
                    u32::try_from(layout.offset)
                        .map_err(|_error| invalid_payload_initialization_state_diagnostic())?,
                )
                .map(|offset| (&field.ty, offset))
                .ok_or_else(invalid_payload_initialization_state_diagnostic)
        })
        .collect()
}

fn invalid_payload_initialization_state_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "IR v0 cannot establish payload field initialization state",
    )]
}
