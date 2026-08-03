use super::*;
use crate::outcomes::{OutcomeLayer, storage::OutcomeStorageLayout};

#[test]
fn lowers_optional_call_result_into_a_recursive_storage_slot() {
    let module = lower_text(
        r#"func main(): i32 {
    let value = maybe()
    return 0
}

func maybe(): i32? {
    return 42
}
"#,
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let Some(Instruction::CallStoredOutcome {
        destination: AggregateLocation::Slot(0),
        target,
        storage:
            OutcomeStorageLayout {
                layout,
                layers,
                payload_offset,
                ..
            },
        payload_type: Type::I32,
        ..
    }) = main
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallStoredOutcome { .. }))
    else {
        panic!("{main:?}");
    };
    assert_eq!(*target, CallTarget::same_file("maybe"));
    assert_eq!(*layout, ValueLayout::new(16, 8));
    assert_eq!(*payload_offset, 8);
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].layer, OutcomeLayer::Optional);
}

#[test]
fn lowers_composed_outcome_call_result_into_one_storage_slot() {
    let module = lower_text(
        r#"func main(): i32 {
    let value = lookup()
    return 0
}

func lookup(): i32?! {
    return none
}
"#,
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let call = main.instructions.iter().find_map(|instruction| {
        let Instruction::CallStoredOutcome { storage, .. } = instruction else {
            return None;
        };
        Some(storage)
    });
    let storage = call.unwrap_or_else(|| panic!("{main:?}"));
    assert_eq!(storage.layers.len(), 2);
    assert_eq!(storage.layers[0].layer, OutcomeLayer::Fallible);
    assert_eq!(storage.layers[1].layer, OutcomeLayer::Optional);
    assert_eq!(storage.layout, ValueLayout::new(40, 8));
}

#[test]
fn consumes_a_stored_optional_after_the_call_statement() {
    let module = lower_text(
        r#"func main(): i32 {
    let saved = maybe()
    let value = saved otherwise { 7 }
    return value
}

func maybe(): i32? {
    return 42
}
"#,
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let Some(Instruction::IfStoredOutcomeTag {
        source: AggregateLocation::Slot(0),
        tag_offset: 0,
        success_instructions,
        outcome_instructions,
    }) = main
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::IfStoredOutcomeTag { .. }))
    else {
        panic!("{main:?}");
    };
    assert_eq!(
        success_instructions,
        &[Instruction::LoadStoredOutcomePayload {
            destination: crate::ir::ComposedOutcomeDestination::I32(I32Location::Local(0)),
            source: AggregateLocation::Slot(0),
            offset: 8,
        }]
    );
    assert_eq!(
        outcome_instructions,
        &[Instruction::SetI32 {
            destination: I32Location::Local(0),
            value: I32Value::Const(7),
        }]
    );
}

#[test]
fn propagates_and_catches_stored_fallible_values() {
    let propagated = lower_text(
        r#"func main(): i32! {
    let saved = attempt()
    let value = saved?
    return value
}

func attempt(): i32! {
    return 42
}
"#,
    );
    let main = propagated
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CheckStoredFallible {
            failure_mode: FallibleFailureMode::Propagate,
            ..
        }
    )));

    let caught = lower_text(
        r#"func main(): i32 {
    let saved = attempt()
    let value = saved catch error { return 9 }
    return value
}

func attempt(): i32! {
    return 42
}
"#,
    );
    let main = caught
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CheckStoredFallible {
            failure_mode: FallibleFailureMode::Catch { .. },
            ..
        }
    )));
}
