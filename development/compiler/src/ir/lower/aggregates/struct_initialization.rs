use std::collections::{HashMap, HashSet};

use crate::abi::{AbiField, layout_struct};
use crate::ast::StructLiteralExpr;
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolValue, Instruction};

use crate::ir::lower::context::{AggregateDrop, LoweringContext, StructFieldDropFlag};

/// Runtime ownership state for a struct while its fields are initialized.
///
/// Flags follow source initialization order rather than declaration order.
/// Failure cleanup can therefore unwind precisely the completed initializer
/// prefix even when a literal names its fields in a different order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::ir::lower) struct StructInitializationProgress {
    fields: Vec<StructFieldDropFlag>,
}

impl StructInitializationProgress {
    pub(in crate::ir::lower) fn new(
        abi_fields: &[AbiField],
        literal: &StructLiteralExpr,
        drop_kind: &AggregateDrop,
        context: &mut LoweringContext,
    ) -> Result<Self, Vec<Diagnostic>> {
        let owned_offsets = match drop_kind {
            AggregateDrop::Direct(_) => HashSet::new(),
            AggregateDrop::Struct(drop_) => drop_
                .fields
                .iter()
                .map(|field| field.offset)
                .collect::<HashSet<_>>(),
            AggregateDrop::Array(_) | AggregateDrop::PayloadEnum(_) => {
                return Err(invalid_struct_initialization_state_diagnostic());
            }
        };
        let layout = layout_struct(abi_fields)
            .map_err(|_error| invalid_struct_initialization_state_diagnostic())?;
        let offsets = abi_fields
            .iter()
            .zip(layout.fields.iter())
            .map(|(field, layout)| {
                u32::try_from(layout.offset)
                    .map(|offset| (field.name.as_str(), offset))
                    .map_err(|_error| invalid_struct_initialization_state_diagnostic())
            })
            .collect::<Result<HashMap<_, _>, _>>()?;

        let mut fields = Vec::with_capacity(owned_offsets.len());
        for field in &literal.fields {
            let Some(offset) = offsets.get(field.name.as_str()).copied() else {
                return Err(invalid_struct_initialization_state_diagnostic());
            };
            if owned_offsets.contains(&offset) {
                fields.push(StructFieldDropFlag {
                    offset,
                    initialized: context.reserve_drop_state_bool_local()?,
                });
            }
        }
        if fields.len() != owned_offsets.len() {
            return Err(invalid_struct_initialization_state_diagnostic());
        }
        Ok(Self { fields })
    }

    pub(in crate::ir::lower) fn drop_flags(&self) -> Vec<StructFieldDropFlag> {
        self.fields.clone()
    }

    pub(in crate::ir::lower) fn initialize(&self) -> Vec<Instruction> {
        self.fields
            .iter()
            .map(|field| Instruction::SetBool {
                destination: field.initialized,
                value: BoolValue::Const(false),
            })
            .collect()
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
            fields: vec![StructFieldDropFlag {
                offset: 8,
                initialized: BoolLocation::Local(3),
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
