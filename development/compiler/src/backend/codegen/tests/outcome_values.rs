use super::*;
use crate::outcomes::{
    OutcomeLayer,
    storage::{OutcomeLayerStorage, OutcomeStorageLayout},
};

fn optional_i32_storage() -> OutcomeStorageLayout {
    OutcomeStorageLayout {
        layout: ValueLayout::new(16, 8),
        layers: vec![OutcomeLayerStorage {
            layer: OutcomeLayer::Optional,
            tag_offset: 0,
            success_offset: 8,
            failure_offset: None,
        }],
        payload_offset: 8,
        payload_layout: ValueLayout::new(4, 4),
    }
}

#[test]
fn emits_callable_to_storage_bridge_and_later_optional_consumption() {
    let storage = optional_i32_storage();
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: storage.layout,
                },
                Instruction::CallStoredOutcome {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("maybe"),
                    arguments: Vec::new(),
                    storage,
                    payload_type: Type::I32,
                },
                Instruction::IfStoredOutcomeTag {
                    source: AggregateLocation::Slot(0),
                    tag_offset: 0,
                    success_instructions: vec![Instruction::LoadStoredOutcomePayload {
                        destination: crate::ir::ComposedOutcomeDestination::I32(
                            I32Location::Return,
                        ),
                        source: AggregateLocation::Slot(0),
                        offset: 8,
                    }],
                    outcome_instructions: vec![set_return_i32(7)],
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "maybe".to_string(),
            target: CallTarget::same_file("maybe"),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![set_return_i32(42), Instruction::ReturnFallibleSuccess],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();
    assert!(!code.text.is_empty());
}
