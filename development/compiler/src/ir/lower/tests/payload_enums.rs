use super::*;

#[test]
fn tracks_partial_fixed_array_payload_construction() {
    let ir = lower_text(
        r#"struct File {
    code: i32
}

instance File {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: [File; 2])
    failed
}

func make_file(): File! {
    return File { code: 22 }
}

func main(): i32! {
    let result = Result.ok([File { code: 20 }, make_file()?])
    return 42
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    assert!(main.instructions.contains(&Instruction::SetBool {
        destination: BoolLocation::Local(0),
        value: BoolValue::Const(false),
    }));
    assert!(main.instructions.contains(&Instruction::SetUsize {
        destination: UsizeLocation::Local(1),
        value: UsizeValue::Const(1),
    }));
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            }
            | Instruction::CallOutcomeDirectAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("make_file") => Some(instructions),
            _ => None,
        });

    assert!(matches!(
        cleanup.map(Vec::as_slice),
        Some([Instruction::If {
            condition: BoolValue::Location(BoolLocation::Local(0)),
            else_instructions,
            ..
        }]) if matches!(
            else_instructions.as_slice(),
            [
                Instruction::If {
                    condition: BoolValue::UsizeComparison {
                        operator: I32ComparisonOperator::Greater,
                        left: UsizeValue::Location(UsizeLocation::Local(1)),
                        right: UsizeValue::Const(1),
                    },
                    ..
                },
                Instruction::If {
                    condition: BoolValue::UsizeComparison {
                        operator: I32ComparisonOperator::Greater,
                        left: UsizeValue::Location(UsizeLocation::Local(1)),
                        right: UsizeValue::Const(0),
                    },
                    then_instructions,
                    ..
                },
            ] if matches!(
                then_instructions.as_slice(),
                [Instruction::CallVoid { arguments, .. }]
                    if matches!(
                        arguments.as_slice(),
                        [ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 4 }
                        })]
                    )
            )
        )
    ));
}

#[test]
fn tracks_partial_payload_construction_as_a_call_argument() {
    let ir = lower_text(
        r#"struct File {
    code: i32
}

instance File {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: [File; 2])
    failed
}

func make_file(): File! {
    return File { code: 22 }
}

func consume(result: Result): i32 {
    return 42
}

func main(): i32! {
    return consume(Result.ok([File { code: 20 }, make_file()?]))
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            }
            | Instruction::CallOutcomeDirectAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("make_file") => Some(instructions),
            _ => None,
        })
        .expect("expected payload argument initialization cleanup");

    assert!(payload_array_prefix_cleanup_targets_slot(cleanup, 0));
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CallI32 { target, arguments, .. }
            if target == &CallTarget::same_file("consume")
                && matches!(
                    arguments.as_slice(),
                    [ScalarArgument::AggregateIndirect(AggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                    })]
                        | [ScalarArgument::AggregateDirect(DirectAggregateArgument {
                            source: AggregateArgumentSource::Slot(0),
                            ..
                        })]
                )
    )));
}

#[test]
fn tracks_partial_payload_construction_in_return_storage() {
    let ir = lower_text(
        r#"struct File {
    code: i32
}

instance File {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: [File; 2])
    failed
}

func make_file(): File! {
    return File { code: 22 }
}

func make_result(): Result! {
    return Result.ok([File { code: 20 }, make_file()?])
}

func main(): i32 {
    make_result()!
    return 0
}
"#,
    );
    let make_result = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("make_result"))
        .expect("expected lowered make_result function");
    let cleanup = make_result
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            }
            | Instruction::CallOutcomeDirectAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("make_file") => Some(instructions),
            _ => None,
        })
        .expect("expected payload return initialization cleanup");

    assert!(payload_array_prefix_cleanup_targets_slot(cleanup, 0));
    assert!(
        make_result
            .instructions
            .contains(&Instruction::CopyAggregate {
                destination: AggregateLocation::DirectReturn,
                source: AggregateLocation::Slot(0),
                layout: ValueLayout::new(12, 4),
            }),
        "{make_result:?}"
    );
}

#[test]
fn partial_payload_replacement_preserves_the_old_value() {
    let ir = lower_text(
        r#"struct File {
    code: i32
}

instance File {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: [File; 2])
    failed
}

func make_file(): File! {
    return File { code: 22 }
}

func main(): i32! {
    var result: Result = Result.failed
    result = Result.ok([File { code: 20 }, make_file()?])
    return 42
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            }
            | Instruction::CallOutcomeDirectAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("make_file") => Some(instructions),
            _ => None,
        })
        .expect("expected replacement initialization cleanup");

    assert!(
        payload_array_prefix_cleanup_targets_slot(cleanup, 1),
        "{main:?}"
    );
    let partial_cleanup_index = cleanup
        .iter()
        .position(|instruction| {
            payload_array_prefix_cleanup_targets_slot(std::slice::from_ref(instruction), 1)
        })
        .expect("expected partial replacement cleanup");
    let old_value_cleanup_index = cleanup
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                Instruction::LoadAggregateU8 {
                    source: AggregateLocation::Slot(0),
                    ..
                }
            )
        })
        .expect("expected old value cleanup on propagated function exit");
    assert!(partial_cleanup_index < old_value_cleanup_index, "{main:?}");
    assert!(main.instructions.contains(&Instruction::CopyAggregate {
        destination: AggregateLocation::Slot(0),
        source: AggregateLocation::Slot(1),
        layout: ValueLayout::new(12, 4),
    }));
}

#[test]
fn tracks_nested_payload_constructor_initialization_recursively() {
    let ir = lower_text(
        r#"struct File {
    code: i32
}

instance File {
    drop &+self {
        return
    }
}

enum Inner {
    some(value: [File; 2])
    empty
}

enum Outer {
    ok(value: Inner)
    failed
}

func make_file(): File! {
    return File { code: 22 }
}

func main(): i32! {
    let result = Outer.ok(Inner.some([File { code: 20 }, make_file()?]))
    return 42
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            }
            | Instruction::CallOutcomeDirectAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("make_file") => Some(instructions),
            _ => None,
        })
        .expect("expected nested payload initialization cleanup");

    assert!(nested_payload_array_prefix_cleanup_targets_slot(cleanup, 0));
    assert!(main.instructions.contains(&Instruction::StoreAggregateU8 {
        destination: AggregateLocation::Slot(0),
        offset: 0,
        value: U8Value::Const(0),
    }));
    assert!(main.instructions.contains(&Instruction::StoreAggregateU8 {
        destination: AggregateLocation::Slot(0),
        offset: 4,
        value: U8Value::Const(0),
    }));
}

#[test]
fn tracks_payload_constructor_nested_in_a_struct_field() {
    let ir = lower_text(
        r#"struct File {
    code: i32
}

instance File {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: [File; 2])
    failed
}

struct Wrapper {
    prefix: i32
    result: Result
}

func make_file(): File! {
    return File { code: 22 }
}

func main(): i32! {
    let wrapper = Wrapper {
        prefix: 1,
        result: Result.ok([File { code: 20 }, make_file()?]),
    }
    return 42
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            }
            | Instruction::CallOutcomeDirectAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("make_file") => Some(instructions),
            _ => None,
        })
        .expect("expected nested struct payload initialization cleanup");

    assert!(main.instructions.contains(&Instruction::StoreAggregateU8 {
        destination: AggregateLocation::Slot(0),
        offset: 4,
        value: U8Value::Const(0),
    }));
    assert!(
        nested_payload_array_prefix_cleanup_targets_slot(cleanup, 0),
        "{main:?}"
    );
}

#[test]
fn partial_multi_field_payload_cleanup_drops_completed_fields_in_reverse_order() {
    let ir = lower_text(
        r#"struct File {
    code: i32
}

instance File {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: File, second: File)
    failed
}

func make_file(): File! {
    return File { code: 22 }
}

func main(): i32! {
    let result = Result.ok(File { code: 20 }, make_file()?)
    return 42
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            }
            | Instruction::CallOutcomeDirectAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("make_file") => Some(instructions),
            _ => None,
        })
        .expect("expected multi-field payload initialization cleanup");

    assert!(
        matches!(
            cleanup.as_slice(),
            [
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(1)),
                    ..
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions,
                    ..
                }
            ] if matches!(
                then_instructions.as_slice(),
                [Instruction::CallVoid { arguments, .. }]
                    if matches!(
                        arguments.as_slice(),
                        [ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 4 },
                        })]
                    )
            )
        ),
        "{main:?}"
    );
}

#[test]
fn tracks_partial_payload_inside_the_current_fixed_array_element() {
    let ir = lower_text(
        r#"struct File {
    code: i32
}

instance File {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: File, second: File)
    failed
}

struct Wrapper {
    result: Result
}

func make_file(): File! {
    return File { code: 22 }
}

func main(): i32! {
    let wrappers: [Wrapper; 2] = [
        Wrapper { result: Result.ok(File { code: 10 }, File { code: 11 }) },
        Wrapper { result: Result.ok(File { code: 20 }, make_file()?) },
    ]
    return 42
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            }
            | Instruction::CallOutcomeDirectAggregate {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("make_file") => Some(instructions),
            _ => None,
        })
        .expect("expected current array element cleanup");

    let current_cleanup = cleanup.iter().find_map(|instruction| match instruction {
        Instruction::If {
            condition:
                BoolValue::UsizeComparison {
                    operator: I32ComparisonOperator::Equal,
                    right: UsizeValue::Const(1),
                    ..
                },
            then_instructions,
            ..
        } => Some(then_instructions),
        _ => None,
    });
    assert!(
        current_cleanup
            .is_some_and(|instructions| { contains_slot_field_drop(instructions, 0, 16) }),
        "{main:?}"
    );
    assert!(
        cleanup.iter().any(|instruction| matches!(
            instruction,
            Instruction::If {
                condition: BoolValue::UsizeComparison {
                    operator: I32ComparisonOperator::Greater,
                    right: UsizeValue::Const(0),
                    ..
                },
                ..
            }
        )),
        "{main:?}"
    );
}

fn contains_slot_field_drop(
    instructions: &[Instruction],
    expected_slot: usize,
    expected_offset: u32,
) -> bool {
    instructions.iter().any(|instruction| match instruction {
        Instruction::CallVoid { arguments, .. } => matches!(
            arguments.as_slice(),
            [ScalarArgument::Borrow(BorrowArgument {
                source: BorrowSource::AggregateSlotField { slot_index, offset },
            })] if *slot_index == expected_slot && *offset == expected_offset
        ),
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => {
            contains_slot_field_drop(then_instructions, expected_slot, expected_offset)
                || contains_slot_field_drop(else_instructions, expected_slot, expected_offset)
        }
        _ => false,
    })
}

fn payload_array_prefix_cleanup_targets_slot(
    instructions: &[Instruction],
    slot_index: usize,
) -> bool {
    instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::If { else_instructions, .. }
                if else_instructions.iter().any(|instruction| matches!(
                    instruction,
                    Instruction::If { then_instructions, .. }
                        if then_instructions.iter().any(|instruction| matches!(
                            instruction,
                            Instruction::CallVoid { arguments, .. }
                                if matches!(
                                    arguments.as_slice(),
                                    [ScalarArgument::Borrow(BorrowArgument {
                                        source: BorrowSource::AggregateSlotField {
                                            slot_index: actual_slot,
                                            offset: 4,
                                        },
                                    })] if *actual_slot == slot_index
                                )
                        ))
                ))
        )
    })
}

fn nested_payload_array_prefix_cleanup_targets_slot(
    instructions: &[Instruction],
    slot_index: usize,
) -> bool {
    matches!(
        instructions,
        [Instruction::If { else_instructions, .. }]
            if matches!(
                else_instructions.as_slice(),
                [Instruction::If { else_instructions, .. }]
                    if else_instructions.iter().any(|instruction| matches!(
                        instruction,
                        Instruction::If { then_instructions, .. }
                            if then_instructions.iter().any(|instruction| matches!(
                                instruction,
                                Instruction::CallVoid { arguments, .. }
                                    if matches!(
                                        arguments.as_slice(),
                                        [ScalarArgument::Borrow(BorrowArgument {
                                            source: BorrowSource::AggregateSlotField {
                                                slot_index: actual_slot,
                                                offset: 8,
                                            },
                                        })] if *actual_slot == slot_index
                                    )
                            ))
                    ))
            )
    )
}

#[test]
fn lowers_direct_payload_enum_value_argument() {
    let aggregate_type = Type::DirectAggregate {
        layout: ValueLayout::new(8, 4),
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"enum Result {
    ok(value: i32)
    failed
}

func accept(result: Result): i32 {
    return 1
}

func make_ok(): Result {
    return Result.ok(20)
}

func main(): i32 {
    let local = Result.ok(10)
    let returned = make_ok()
    return accept(move local) + accept(move returned)
}
"#,
        "main",
        function_signatures(vec![
            ("accept", Type::I32, vec![aggregate_type.clone()]),
            ("make_ok", aggregate_type.clone(), vec![]),
        ]),
    )
    .unwrap();

    assert_eq!(
        function.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(8, 4),
            },
            Instruction::StoreAggregateU8 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: u8_const(0),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 4,
                value: i32_const(10),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(8, 4),
            },
            Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make_ok"),
                arguments: vec![],
                layout: ValueLayout::new(8, 4),
            },
            Instruction::CallI32 {
                destination: I32Location::Local(0),
                target: CallTarget::same_file("accept"),
                arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(8, 4),
                    words: 1,
                })],
            },
            Instruction::CallI32 {
                destination: I32Location::Local(1),
                target: CallTarget::same_file("accept"),
                arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(1),
                    layout: ValueLayout::new(8, 4),
                    words: 1,
                })],
            },
            Instruction::AddI32 {
                destination: I32Location::Return,
                left: i32_local(0),
                right: i32_local(1),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_payload_enum_copy_aggregate_payload_binding() {
    let ir = lower_text(
        r#"copy struct Detail {
    code: i32
    bonus: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.ok(Detail { code: 42, bonus: 1 })
    if result is Result.ok(value) {
        return value.code
    }

    return 0
}
"#,
    );

    let main = &ir.functions[0];
    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::If {
                    then_instructions,
                    ..
                } if then_instructions.contains(&Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(8, 4),
                }) && then_instructions.contains(&Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Slot(1),
                    destination_offset: 0,
                    source: AggregateLocation::Slot(0),
                    source_offset: 4,
                    layout: ValueLayout::new(8, 4),
                }) && then_instructions.contains(&Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::Slot(1),
                    offset: 0,
                })
            )
        }),
        "{main:?}"
    );
}

#[test]
fn lowers_owned_direct_drop_payload_binding_with_conditional_target_cleanup() {
    let ir = lower_text(
        r#"struct Payload {
    code: i32
}

instance Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { code: 42 })
    if move result is Result.ok(value) {
        let code = value.code
    }
    return 0
}
"#,
    );

    let main = &ir.functions[0];
    assert!(
        main.instructions.contains(&Instruction::SetBool {
            destination: BoolLocation::Local(0),
            value: BoolValue::Const(true),
        }),
        "{main:?}"
    );

    let pattern_branch = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::If {
                then_instructions, ..
            } if then_instructions.contains(&Instruction::SetBool {
                destination: BoolLocation::Local(1),
                value: BoolValue::Const(false),
            }) =>
            {
                Some(then_instructions)
            }
            _ => None,
        })
        .expect("expected move-binding pattern branch");
    assert!(pattern_branch.contains(&Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(2),
        destination_offset: 0,
        source: AggregateLocation::Slot(1),
        source_offset: 4,
        layout: ValueLayout::new(4, 4),
    }));
    assert!(pattern_branch.contains(&Instruction::CallVoid {
        target: CallTarget::same_file("Payload.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(2),
        })],
    }));

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(1)),
                    then_instructions,
                    else_instructions,
                } if else_instructions.is_empty()
                    && then_instructions.iter().any(|instruction| matches!(
                        instruction,
                        Instruction::LoadAggregateU8 {
                            source: AggregateLocation::Slot(1),
                            offset: 0,
                            ..
                        }
                    ))
            )
        }),
        "{main:?}"
    );
}

#[test]
fn lowers_live_enum_cleanup_on_propagated_pattern_target_failure() {
    let ir = lower_text(
        r#"struct File {
    code: i32
}

instance File {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: [File; 2])
    failed
}

func fail(): Result! {
    return Result.failed
}

func main(): void! {
    let guard = Result.ok([File { code: 1 }, File { code: 2 }])
    match (fail()?) {
        Result.ok(files) { return }
        _ { return }
    }
}
"#,
    );

    let main = &ir.functions[0];
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeAggregate {
                failure_mode:
                    OutcomeFailureMode::PropagateWithCleanup {
                        code,
                        message,
                        instructions,
                    },
                ..
            }
            | Instruction::CallOutcomeDirectAggregate {
                failure_mode:
                    OutcomeFailureMode::PropagateWithCleanup {
                        code,
                        message,
                        instructions,
                    },
                ..
            } => Some((code, message, instructions)),
            _ => None,
        });

    let Some((StrLocation::Local(code), StrLocation::Local(message), instructions)) = cleanup
    else {
        panic!("expected propagated cleanup with local error payload: {main:?}");
    };
    let Some(Instruction::LoadAggregateU8 {
        destination: U8Location::Local(cleanup_temporary),
        ..
    }) = instructions.first()
    else {
        panic!("expected enum cleanup discriminator load: {instructions:?}");
    };

    assert!(!instructions.is_empty());
    assert!(
        ![*code, *code + 1, *message, *message + 1].contains(cleanup_temporary),
        "cleanup temporary must not overlap the preserved error payload: {main:?}"
    );
}

#[test]
fn lowers_payload_enum_slice_payload_binding() {
    let score = lower_named_function(
        r#"enum Result {
    ok(value: &[u8])
    failed
}

func main(): void {
    return
}

func score(bytes: &[u8]): usize {
    let result = Result.ok(bytes)
    if result is Result.ok(value) {
        return value.len()
    }

    return 0
}
"#,
        "score",
    );

    assert!(
        score.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::If {
                    then_instructions,
                    ..
                } if then_instructions.contains(&Instruction::LoadAggregateUsize {
                    destination: UsizeLocation::Local(1),
                    source: AggregateLocation::Slot(0),
                    offset: 8,
                }) && then_instructions.contains(&Instruction::LoadAggregateUsize {
                    destination: UsizeLocation::Local(2),
                    source: AggregateLocation::Slot(0),
                    offset: 16,
                }) && then_instructions.iter().any(|instruction| matches!(
                    instruction,
                    Instruction::TailCall { target, arguments }
                        if target == &builtin_slice_method_target("u8", "len")
                            && arguments == &vec![ScalarArgument::Slice(SliceValue::Location(
                                SliceLocation::Local(1),
                            ))]
                ))
            )
        }),
        "{score:?}"
    );
}

#[test]
fn lowers_scope_end_drop_for_active_payload_enum_payload() {
    let ir = lower_text(
        r#"struct Payload {
    code: i32
}

instance Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { code: 42 })
    return 0
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: CallTarget::same_file("main"),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(8, 4),
                    },
                    Instruction::SetBool {
                        destination: BoolLocation::Local(0),
                        value: BoolValue::Const(false),
                    },
                    Instruction::StoreAggregateU8 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: u8_const(0),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 4,
                        value: i32_const(42),
                    },
                    Instruction::SetBool {
                        destination: BoolLocation::Local(0),
                        value: BoolValue::Const(true),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Local(1),
                        value: i32_const(0),
                    },
                    Instruction::LoadAggregateU8 {
                        destination: U8Location::Local(2),
                        source: AggregateLocation::Slot(0),
                        offset: 0,
                    },
                    Instruction::If {
                        condition: BoolValue::I32Comparison {
                            operator: I32ComparisonOperator::Equal,
                            left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(
                                U8Location::Local(2),
                            ))),
                            right: I32Value::U8ZeroExtend(Box::new(u8_const(0))),
                        },
                        then_instructions: vec![Instruction::CallVoid {
                            target: CallTarget::same_file("Payload.drop"),
                            arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                                source: BorrowSource::AggregateSlotField {
                                    slot_index: 0,
                                    offset: 4,
                                },
                            })],
                        }],
                        else_instructions: Vec::new(),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(1),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "Payload.drop".to_string(),
                target: CallTarget::same_file("Payload.drop"),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_scope_end_drop_for_multi_field_active_payload_enum_payload() {
    let ir = lower_text(
        r#"struct Payload {
    code: i32
}

instance Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: Payload, second: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { code: 10 }, Payload { code: 20 })
    return 0
}
"#,
    );
    let main = &ir.functions[0];

    let drop_then_instructions = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::If {
                then_instructions, ..
            } if !then_instructions.is_empty()
                && then_instructions.iter().all(|then_instruction| {
                    matches!(
                        then_instruction,
                        Instruction::CallVoid {
                            target,
                            ..
                        } if target == &CallTarget::same_file("Payload.drop")
                    )
                }) =>
            {
                Some(then_instructions)
            }
            _ => None,
        })
        .expect("expected active payload drop branch");

    assert_eq!(
        drop_then_instructions,
        &vec![
            Instruction::CallVoid {
                target: CallTarget::same_file("Payload.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlotField {
                        slot_index: 0,
                        offset: 8,
                    },
                })],
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("Payload.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlotField {
                        slot_index: 0,
                        offset: 4,
                    },
                })],
            },
        ]
    );
}

#[test]
fn lowers_scope_end_drop_for_inactive_payload_enum_payload() {
    let ir = lower_text(
        r#"struct Payload {
    code: i32
}

instance Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.failed
    return 42
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: CallTarget::same_file("main"),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(8, 4),
                    },
                    Instruction::StoreAggregateU8 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: u8_const(1),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(42),
                    },
                    Instruction::LoadAggregateU8 {
                        destination: U8Location::Local(1),
                        source: AggregateLocation::Slot(0),
                        offset: 0,
                    },
                    Instruction::If {
                        condition: BoolValue::I32Comparison {
                            operator: I32ComparisonOperator::Equal,
                            left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(
                                U8Location::Local(1),
                            ))),
                            right: I32Value::U8ZeroExtend(Box::new(u8_const(0))),
                        },
                        then_instructions: vec![Instruction::CallVoid {
                            target: CallTarget::same_file("Payload.drop"),
                            arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                                source: BorrowSource::AggregateSlotField {
                                    slot_index: 0,
                                    offset: 4,
                                },
                            })],
                        }],
                        else_instructions: Vec::new(),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "Payload.drop".to_string(),
                target: CallTarget::same_file("Payload.drop"),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_wildcard_only_payloadless_match_statement_without_branch() {
    let ir = lower_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.no
    match choice {
        _ {
            return 7
        }
    }
}
"#,
    );

    let instructions = &ir.functions[0].instructions;
    assert!(
        instructions
            .iter()
            .all(|instruction| !matches!(instruction, Instruction::If { .. })),
        "{instructions:?}"
    );
    assert!(
        instructions.contains(&set_return_i32(7)),
        "{instructions:?}"
    );
}

#[test]
fn lowers_wildcard_only_payloadless_match_expression_without_branch() {
    let ir = lower_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    let result = match choice {
        _ {
            7
        }
    }
    return result
}
"#,
    );

    let instructions = &ir.functions[0].instructions;
    assert!(
        instructions
            .iter()
            .all(|instruction| !matches!(instruction, Instruction::If { .. })),
        "{instructions:?}"
    );
    assert!(
        instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::SetI32 {
                destination: I32Location::Local(_),
                value: I32Value::Const(7)
            }
        )),
        "{instructions:?}"
    );
}
