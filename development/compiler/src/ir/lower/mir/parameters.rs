//! Projection from MIR source-parameter ordinals to machine-IR storage.
//!
//! MIR deliberately records source ordinals. ABI word positions and aggregate
//! staging slots are selected once here from the already validated parameter
//! layout, including parameters that consume multiple words.

use crate::abi::{ValueClassification, ValueLayout};
use crate::ast::Parameter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ParameterStorage {
    I32 {
        abi_index: usize,
    },
    U8 {
        abi_index: usize,
    },
    Usize {
        abi_index: usize,
    },
    Integer {
        kind: crate::integer::IntegerType,
        abi_index: usize,
    },
    Bool {
        abi_index: usize,
    },
    Str {
        abi_index: usize,
    },
    Slice {
        abi_index: usize,
    },
    Borrow {
        abi_index: usize,
    },
    Error {
        abi_index: usize,
    },
    Aggregate {
        slot_index: usize,
        layout: ValueLayout,
        classification: ValueClassification,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParameterProjection {
    storage: Vec<ParameterStorage>,
    first_local_aggregate_slot: usize,
}

impl ParameterProjection {
    pub(super) fn from_slots(
        parameters: &[Parameter],
        slots: &super::super::context::LoweringParameterSlots,
    ) -> Option<Self> {
        let storage = parameters
            .iter()
            .map(|parameter| storage_for_name(&parameter.name, slots))
            .collect::<Option<Vec<_>>>()?;
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

fn storage_for_name(
    name: &str,
    slots: &super::super::context::LoweringParameterSlots,
) -> Option<ParameterStorage> {
    if let Some(abi_index) = named_word(&slots.i32, name) {
        return Some(ParameterStorage::I32 { abi_index });
    }
    if let Some(abi_index) = named_word(&slots.u8, name) {
        return Some(ParameterStorage::U8 { abi_index });
    }
    if let Some(abi_index) = named_word(&slots.usize, name) {
        return Some(ParameterStorage::Usize { abi_index });
    }
    if let Some((abi_index, kind)) = slots.integer.iter().enumerate().find_map(|(index, value)| {
        value
            .as_ref()
            .filter(|(candidate, _)| candidate == name)
            .map(|(_, kind)| (index, *kind))
    }) {
        return Some(ParameterStorage::Integer { kind, abi_index });
    }
    if let Some(abi_index) = named_word(&slots.bool, name) {
        return Some(ParameterStorage::Bool { abi_index });
    }
    if let Some(abi_index) = named_word(&slots.str, name) {
        return Some(ParameterStorage::Str { abi_index });
    }
    if let Some(abi_index) = slots.slice.iter().position(|candidate| {
        candidate
            .as_ref()
            .is_some_and(|candidate| candidate.name == name)
    }) {
        return Some(ParameterStorage::Slice { abi_index });
    }
    if let Some(parameter) = slots
        .borrow_parameters
        .iter()
        .find(|parameter| parameter.name == name)
    {
        return Some(ParameterStorage::Borrow {
            abi_index: parameter.parameter_index,
        });
    }
    if let Some(abi_index) = named_word(&slots.error, name) {
        return Some(ParameterStorage::Error { abi_index });
    }
    if let Some(parameter) = slots
        .aggregate_borrows
        .iter()
        .find(|parameter| parameter.name == name)
    {
        return Some(ParameterStorage::Borrow {
            abi_index: parameter.parameter_index,
        });
    }
    slots
        .aggregates
        .iter()
        .find(|parameter| parameter.name == name)
        .map(|parameter| ParameterStorage::Aggregate {
            slot_index: parameter.slot_index,
            layout: parameter.layout,
            classification: match parameter.source {
                super::super::context::AggregateParameterSource::Indirect { .. } => {
                    ValueClassification::Indirect
                }
                super::super::context::AggregateParameterSource::Direct { words, .. } => {
                    ValueClassification::Direct { words }
                }
            },
        })
        .or_else(|| {
            slots
                .outcomes
                .iter()
                .find(|parameter| parameter.name == name)
                .map(|parameter| ParameterStorage::Aggregate {
                    slot_index: parameter.slot_index,
                    layout: parameter.storage.layout,
                    classification: match parameter.source {
                        super::super::context::AggregateParameterSource::Indirect { .. } => {
                            ValueClassification::Indirect
                        }
                        super::super::context::AggregateParameterSource::Direct {
                            words, ..
                        } => ValueClassification::Direct { words },
                    },
                })
        })
}

fn named_word(words: &[Option<String>], name: &str) -> Option<usize> {
    words
        .iter()
        .position(|candidate| candidate.as_deref() == Some(name))
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut slots = super::super::super::context::LoweringParameterSlots::default();
        slots.push_empty_abi_word();
        slots.push_i32_parameter("value".to_string());

        let projection = ParameterProjection::from_slots(&[parameter("value")], &slots).unwrap();

        assert_eq!(
            projection.get(0),
            Some(ParameterStorage::I32 { abi_index: 1 })
        );
    }

    #[test]
    fn u8_parameter_uses_its_dedicated_abi_slot() {
        let mut slots = super::super::super::context::LoweringParameterSlots::default();
        slots.push_u8_parameter("value".to_string());

        let projection = ParameterProjection::from_slots(&[parameter("value")], &slots).unwrap();

        assert_eq!(
            projection.get(0),
            Some(ParameterStorage::U8 { abi_index: 0 })
        );
    }

    #[test]
    fn stored_outcome_parameter_uses_aggregate_staging_slot() {
        let mut slots = super::super::super::context::LoweringParameterSlots::default();
        let layout = crate::outcomes::storage::outcome_storage_layout(
            &[crate::outcomes::OutcomeLayer::Optional],
            ValueLayout::new(4, 4),
        );
        slots
            .outcomes
            .push(super::super::super::context::LoweringOutcomeParameter {
                name: "value".to_string(),
                storage: layout.clone(),
                slot_index: 0,
                source: super::super::super::context::AggregateParameterSource::Indirect {
                    parameter_index: 0,
                },
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
