use super::{OutcomeLayer, OutcomeShape};
use crate::abi::ValueLayout;

const TAG_LAYOUT: ValueLayout = ValueLayout { size: 8, align: 8 };
const ERROR_LAYOUT: ValueLayout = ValueLayout { size: 32, align: 8 };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OutcomeLayerStorage {
    pub(crate) layer: OutcomeLayer,
    pub(crate) tag_offset: u64,
    pub(crate) success_offset: u64,
    pub(crate) failure_offset: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutcomeStorageLayout {
    pub(crate) layout: ValueLayout,
    pub(crate) layers: Vec<OutcomeLayerStorage>,
    pub(crate) payload_offset: u64,
    pub(crate) payload_layout: ValueLayout,
}

impl OutcomeShape {
    pub(crate) fn storage_layout(
        &self,
        payload_layout: ValueLayout,
    ) -> Option<OutcomeStorageLayout> {
        self.is_supported_callable_shape()
            .then(|| build_storage_layout(&self.layers, payload_layout))
    }
}

pub(crate) fn outcome_storage_layout(
    layers: &[OutcomeLayer],
    payload_layout: ValueLayout,
) -> OutcomeStorageLayout {
    build_storage_layout(layers, payload_layout)
}

fn build_storage_layout(
    layers: &[OutcomeLayer],
    payload_layout: ValueLayout,
) -> OutcomeStorageLayout {
    let mut entries = Vec::with_capacity(layers.len());
    let (layout, payload_offset) = build_layer_layout(layers, payload_layout, 0, &mut entries);
    OutcomeStorageLayout {
        layout,
        layers: entries,
        payload_offset,
        payload_layout,
    }
}

fn build_layer_layout(
    layers: &[OutcomeLayer],
    payload_layout: ValueLayout,
    base_offset: u64,
    entries: &mut Vec<OutcomeLayerStorage>,
) -> (ValueLayout, u64) {
    let Some((layer, remaining)) = layers.split_first() else {
        return (payload_layout, base_offset);
    };

    let branch_alignment = nested_alignment(remaining, payload_layout.align);
    let union_offset = align_up(TAG_LAYOUT.size, branch_alignment);
    let entry_index = entries.len();
    entries.push(OutcomeLayerStorage {
        layer: *layer,
        tag_offset: base_offset,
        success_offset: base_offset + union_offset,
        failure_offset: (*layer == OutcomeLayer::Fallible).then_some(base_offset + union_offset),
    });

    let (success_layout, payload_offset) = build_layer_layout(
        remaining,
        payload_layout,
        base_offset + union_offset,
        entries,
    );
    let union_layout = match layer {
        OutcomeLayer::Optional => success_layout,
        OutcomeLayer::Fallible => ValueLayout::new(
            success_layout.size.max(ERROR_LAYOUT.size),
            success_layout.align.max(ERROR_LAYOUT.align),
        ),
    };
    let alignment = TAG_LAYOUT.align.max(union_layout.align);
    let size = align_up(union_offset + union_layout.size, alignment);

    debug_assert_eq!(entries[entry_index].tag_offset, base_offset);
    (ValueLayout::new(size, alignment), payload_offset)
}

fn nested_alignment(layers: &[OutcomeLayer], payload_alignment: u64) -> u64 {
    if layers.is_empty() {
        payload_alignment
    } else {
        TAG_LAYOUT.align.max(payload_alignment)
    }
}

fn align_up(value: u64, alignment: u64) -> u64 {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{TypeExpr, TypeReference};
    use crate::source::{ByteSpan, SourceId};

    fn shape(layers: Vec<OutcomeLayer>) -> OutcomeShape {
        let span = ByteSpan::new(SourceId::new(0), 0, 1);
        OutcomeShape {
            layers,
            payload: TypeExpr::Reference(TypeReference {
                span,
                name: "i32".to_string(),
            }),
        }
    }

    #[test]
    fn lays_out_single_optional_and_fallible_values() {
        let optional = shape(vec![OutcomeLayer::Optional])
            .storage_layout(ValueLayout::new(4, 4))
            .unwrap();
        assert_eq!(optional.layout, ValueLayout::new(16, 8));
        assert_eq!(optional.layers[0].tag_offset, 0);
        assert_eq!(optional.payload_offset, 8);

        let fallible = shape(vec![OutcomeLayer::Fallible])
            .storage_layout(ValueLayout::new(4, 4))
            .unwrap();
        assert_eq!(fallible.layout, ValueLayout::new(40, 8));
        assert_eq!(fallible.layers[0].failure_offset, Some(8));
        assert_eq!(fallible.payload_offset, 8);
    }

    #[test]
    fn preserves_nested_layer_order_and_union_offsets() {
        let fallible_optional = shape(vec![OutcomeLayer::Fallible, OutcomeLayer::Optional])
            .storage_layout(ValueLayout::new(4, 4))
            .unwrap();
        assert_eq!(fallible_optional.layout, ValueLayout::new(40, 8));
        assert_eq!(fallible_optional.layers[0].tag_offset, 0);
        assert_eq!(fallible_optional.layers[0].failure_offset, Some(8));
        assert_eq!(fallible_optional.layers[1].tag_offset, 8);
        assert_eq!(fallible_optional.payload_offset, 16);

        let optional_fallible = shape(vec![OutcomeLayer::Optional, OutcomeLayer::Fallible])
            .storage_layout(ValueLayout::new(4, 4))
            .unwrap();
        assert_eq!(optional_fallible.layout, ValueLayout::new(48, 8));
        assert_eq!(optional_fallible.layers[1].tag_offset, 8);
        assert_eq!(optional_fallible.layers[1].failure_offset, Some(16));
        assert_eq!(optional_fallible.payload_offset, 16);
    }
}
