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
fn propagates_a_stored_optional_value_as_none() {
    let module = lower_text(
        r#"func main(): i32 {
    return forward() otherwise { 7 }
}

func forward(): i32? {
    let saved = maybe()
    let value = saved?
    return value
}

func maybe(): i32? {
    return none
}
"#,
    );
    let forward = module
        .functions
        .iter()
        .find(|function| function.name == "forward")
        .unwrap();
    assert!(
        forward.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::IfStoredOutcomeTag {
                source: AggregateLocation::Slot(0),
                success_instructions,
                outcome_instructions,
                ..
            } if matches!(
                success_instructions.as_slice(),
                [Instruction::LoadStoredOutcomePayload {
                    destination: crate::ir::ComposedOutcomeDestination::I32(I32Location::Local(0)),
                    ..
                }]
            ) && outcome_instructions == &[Instruction::ReturnOptionalNone]
        )),
        "{forward:?}"
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
            failure_mode: OutcomeFailureMode::Propagate,
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
            failure_mode: OutcomeFailureMode::Catch { .. },
            ..
        }
    )));
}

#[test]
fn copies_and_replaces_copyable_stored_outcomes() {
    let module = lower_text(
        r#"func main(): i32 {
    let first = maybe(1)
    var second = first
    second = maybe(2)
    let value = second otherwise { 7 }
    return value
}

func maybe(value: i32): i32? {
    return value
}
"#,
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CopyAggregate {
            destination: AggregateLocation::Slot(1),
            source: AggregateLocation::Slot(0),
            layout: ValueLayout { size: 16, align: 8 },
        }
    )));
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CallStoredOutcome {
            destination: AggregateLocation::Slot(_),
            ..
        }
    )));
}

#[test]
fn passes_stored_outcomes_through_value_parameters() {
    let module = lower_text(
        r#"func main(): i32 {
    let saved = maybe()
    let result = inspect(saved)
    return result
}

func maybe(): i32? {
    return 42
}

func inspect(value: i32?): i32 {
    return value otherwise { 7 }
}
"#,
    );
    let inspect = module
        .functions
        .iter()
        .find(|function| function.name == "inspect")
        .unwrap();
    assert!(matches!(
        inspect.instructions.as_slice(),
        [
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout { size: 16, align: 8 },
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::DirectParameter { start_index: 0 },
                ..
            },
            ..
        ]
    ));
    assert!(
        inspect
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::IfStoredOutcomeTag { .. }))
    );
}

#[test]
fn returns_stored_outcomes_through_callable_abi_unpacking() {
    let module = lower_text(
        r#"func main(): i32 {
    let optional = make_optional()
    let forwarded = forward(optional)
    let composed = make_composed()
    let forwarded_composed = forward_composed(composed)
    return 0
}

func make_optional(): i32? {
    return 1
}

func make_composed(): i32?! {
    return 2
}

func forward(value: i32?): i32? {
    return value
}

func forward_composed(value: i32?!): i32?! {
    return value
}
"#,
    );
    for name in ["forward", "forward_composed"] {
        let function = module
            .functions
            .iter()
            .find(|function| function.name == name)
            .unwrap();
        assert!(matches!(
            function.instructions.last(),
            Some(Instruction::ReturnStoredOutcome { .. })
        ));
    }
}

#[test]
fn drops_only_the_active_owned_outcome_payload() {
    let module = lower_text(
        r#"struct Resource {
    code: i32
}

impl Resource {
    drop &+self {
        return
    }
}

func main(): i32 {
    let saved = make()
    return 0
}

func make(): Resource? {
    return Resource { code: 1 }
}
"#,
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::IfStoredOutcomeTag {
            success_instructions,
            outcome_instructions,
            ..
        } if outcome_instructions.is_empty()
            && success_instructions.iter().any(|nested| matches!(nested,
                Instruction::CallVoid { target: CallTarget::SameFile(name), .. }
                    if name == "Resource.drop"))
    )));
}

#[test]
fn stores_and_extracts_outcomes_in_struct_fields() {
    let module = lower_text(
        r#"struct Holder {
    value: i32?
}

func main(): i32 {
    let saved = maybe()
    let holder = Holder { value: saved }
    let extracted = holder.value
    return extracted otherwise { 7 }
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
    assert!(
        main.instructions
            .iter()
            .filter(|instruction| matches!(
                instruction,
                Instruction::CopyAggregateRange {
                    layout: ValueLayout { size: 16, align: 8 },
                    ..
                }
            ))
            .count()
            >= 2
    );
    assert!(
        main.instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::IfStoredOutcomeTag { .. }))
    );
}

#[test]
fn consumes_both_layers_of_a_stored_fallible_optional() {
    let module = lower_text(
        r#"func main(): i32! {
    let saved = lookup()
    let value = saved? otherwise { return 7 }
    return value
}

func lookup(): i32?! {
    return 42
}
"#,
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CheckStoredFallible {
            success_instructions,
            ..
        } if success_instructions.iter().any(|nested| matches!(nested,
            Instruction::IfStoredOutcomeTag { .. }))
    )));
}

#[test]
fn stores_and_extracts_outcomes_in_fixed_arrays() {
    let module = lower_text(
        r#"func main(): i32 {
    let saved = maybe()
    let values: [i32?; 1] = [saved]
    let extracted = values[0]
    return extracted otherwise { 7 }
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
    assert!(
        main.instructions
            .iter()
            .filter(|instruction| matches!(
                instruction,
                Instruction::CopyAggregateRange {
                    layout: ValueLayout { size: 16, align: 8 },
                    ..
                }
            ))
            .count()
            >= 2
    );
}

#[test]
fn moving_owned_outcomes_transfers_one_drop_obligation() {
    let module = lower_text(
        r#"struct Resource { code: i32 }

impl Resource {
    drop &+self { return }
}

func main(): i32 {
    let first = make()
    let second = move first
    return 0
}

func make(): Resource? {
    return Resource { code: 1 }
}
"#,
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::IfStoredOutcomeTag { .. }))
            .count(),
        1
    );
}

#[test]
fn extracts_indirect_aggregate_payload_from_stored_outcome() {
    let module = lower_text(
        r#"struct Wide {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let saved = make()
    let value = saved!
    if value.second == 42 { return 42 }
    return 1
}

func make(): Wide? {
    return Wide { first: 1, second: 42, third: 3 }
}
"#,
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::IfStoredOutcomeTag {
            success_instructions,
            ..
        } if success_instructions.iter().any(|nested| matches!(nested,
            Instruction::CopyAggregateRange {
                layout: ValueLayout { size: 24, align: 8 },
                ..
            }))
    )));
}

#[test]
fn consumes_both_layers_of_a_stored_optional_fallible() {
    let module = lower_text(
        r#"func main(): i32! {
    let saved = lookup()
    let value = (saved otherwise { return 7 })?
    return value
}

func lookup(): i32!? {
    return 42
}
"#,
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::IfStoredOutcomeTag {
            success_instructions,
            ..
        } if success_instructions.iter().any(|nested| matches!(nested,
            Instruction::CheckStoredFallible { .. }))
    )));
}

#[test]
fn specializes_alias_method_and_generic_stored_outcomes() {
    let module = lower_text(
        r#"type MaybeInt = i32?

copy struct Holder<T> { value: T }

impl<T> Holder<T> {
    method &self.get(): T? {
        return self.value
    }
}

func forward<T>(value: T?): T? {
    return move value
}

func main(): i32 {
    let holder = Holder<i32> { value: 42 }
    let from_method = holder.get()
    let saved: MaybeInt = from_method
    let forwarded = forward(saved)
    let result = forwarded otherwise { 1 }
    return result
}
"#,
    );
    assert!(module.functions.iter().any(|function| {
        function.name.starts_with("forward<")
            && matches!(
                function.instructions.last(),
                Some(Instruction::ReturnStoredOutcome { .. })
            )
    }));
}

#[test]
fn evaluates_owned_replacement_before_dropping_the_old_payload() {
    let module = lower_text(
        r#"struct Resource { code: i32 }

impl Resource {
    drop &+self { return }
}

func main(): i32 {
    var saved = make(1)
    saved = make(2)
    return 0
}

func make(code: i32): Resource? {
    return Resource { code: code }
}
"#,
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let replacement_call = main
        .instructions
        .iter()
        .rposition(|instruction| matches!(instruction, Instruction::CallStoredOutcome { .. }))
        .unwrap();
    let old_drop = main
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                Instruction::IfStoredOutcomeTag { success_instructions, .. }
                    if success_instructions.iter().any(|nested| matches!(nested,
                        Instruction::CallVoid { target: CallTarget::SameFile(name), .. }
                            if name == "Resource.drop"))
            )
        })
        .unwrap();
    let publication = main
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    ..
                }
            )
        })
        .unwrap();
    assert!(replacement_call < old_drop && old_drop < publication);
}
