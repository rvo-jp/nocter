//! Projection from MIR source-parameter ordinals to machine-IR storage.
//!
//! MIR deliberately records source ordinals. ABI word positions and aggregate
//! staging slots are selected once here from the already validated parameter
//! layout, including parameters that consume multiple words.

use crate::ast::Parameter;

pub(super) use super::super::parameter_slots::ParameterStorage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParameterProjection {
    storage: Vec<ParameterStorage>,
    first_local_aggregate_slot: usize,
}

impl ParameterProjection {
    pub(super) fn from_slots(
        parameters: &[Parameter],
        slots: &super::super::parameter_slots::LoweringParameterSlots,
    ) -> Option<Self> {
        if parameters.len() != slots.source_storage().len() {
            return None;
        }
        let storage = slots.source_storage().to_vec();
        let first_local_aggregate_slot = slots
            .aggregates
            .iter()
            .map(|parameter| parameter.slot_index + 1)
            .chain(
                slots
                    .outcomes
                    .iter()
                    .map(|parameter| parameter.slot_index + 1),
            )
            .max()
            .unwrap_or(0);
        Some(Self {
            storage,
            first_local_aggregate_slot,
        })
    }

    pub(super) fn get(&self, ordinal: usize) -> Option<ParameterStorage> {
        self.storage.get(ordinal).copied()
    }

    pub(super) fn first_local_aggregate_slot(&self) -> usize {
        self.first_local_aggregate_slot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{ValueClassification, ValueLayout};
    use crate::ast::{TypeExpr, TypeReference};
    use crate::source::{ByteSpan, SourceId};

    fn parameter(name: &str) -> Parameter {
        let span = ByteSpan::new(SourceId::new(0), 0, 1);
        Parameter {
            span,
            name: name.to_string(),
            name_span: span,
            ty: TypeExpr::Reference(TypeReference {
                span,
                name: "i32".to_string(),
            }),
        }
    }

    #[test]
    fn scalar_parameter_uses_abi_word_position_not_source_ordinal() {
        let mut slots = super::super::super::parameter_slots::LoweringParameterSlots::default();
        slots.push_empty_abi_word();
        slots.push_i32_parameter();

        let projection = ParameterProjection::from_slots(&[parameter("value")], &slots).unwrap();

        assert_eq!(
            projection.get(0),
            Some(ParameterStorage::I32 { abi_index: 1 })
        );
    }

    #[test]
    fn u8_parameter_uses_its_dedicated_abi_slot() {
        let mut slots = super::super::super::parameter_slots::LoweringParameterSlots::default();
        slots.push_u8_parameter();

        let projection = ParameterProjection::from_slots(&[parameter("value")], &slots).unwrap();

        assert_eq!(
            projection.get(0),
            Some(ParameterStorage::U8 { abi_index: 0 })
        );
    }

    #[test]
    fn stored_outcome_parameter_uses_aggregate_staging_slot() {
        let mut slots = super::super::super::parameter_slots::LoweringParameterSlots::default();
        let layout = crate::outcomes::storage::outcome_storage_layout(
            &[crate::outcomes::OutcomeLayer::Optional],
            ValueLayout::new(4, 4),
        );
        slots.outcomes.push(
            super::super::super::parameter_slots::LoweringOutcomeParameter {
                name: "value".to_string(),
                storage: layout.clone(),
                slot_index: 0,
                source: super::super::super::parameter_slots::AggregateParameterSource::Indirect {
                    parameter_index: 0,
                },
            },
        );
        slots.push_source_storage(ParameterStorage::Aggregate {
            slot_index: 0,
            layout: layout.layout,
            classification: ValueClassification::Indirect,
        });

        let projection = ParameterProjection::from_slots(&[parameter("value")], &slots).unwrap();

        assert_eq!(
            projection.get(0),
            Some(ParameterStorage::Aggregate {
                slot_index: 0,
                layout: layout.layout,
                classification: ValueClassification::Indirect,
            })
        );
    }
}
