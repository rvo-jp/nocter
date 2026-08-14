use super::*;

#[test]
fn lowers_recursive_drop_fixed_array_elements_in_reverse_offset_order() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

type Files = [File; 3]

func main(): i32 {
    let files: Files = [
        File { fd: 1 },
        File { fd: 2 },
        File { fd: 3 }
    ]
    return 0
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    let drop_sources = main
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::CallVoid { target, arguments }
                if target == &CallTarget::same_file("File.drop") =>
            {
                arguments.first().and_then(|argument| match argument {
                    ScalarArgument::Borrow(borrow) => Some(borrow.source),
                    _ => None,
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        drop_sources,
        vec![
            BorrowSource::AggregateSlotField {
                slot_index: 0,
                offset: 8,
            },
            BorrowSource::AggregateSlotField {
                slot_index: 0,
                offset: 4,
            },
            BorrowSource::AggregateSlot(0),
        ]
    );
}

#[test]
fn lowers_partial_fixed_array_initialization_cleanup_from_runtime_prefix() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func make_file(fd: i32): File! {
    return File { fd: fd }
}

func main(): i32! {
    let files: [File; 2] = [File { fd: 1 }, make_file(2)?]
    return 0
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    assert!(main.instructions.contains(&Instruction::SetUsize {
        destination: UsizeLocation::Local(0),
        value: UsizeValue::Const(0),
    }));
    assert!(main.instructions.contains(&Instruction::SetUsize {
        destination: UsizeLocation::Local(0),
        value: UsizeValue::Const(1),
    }));

    let cleanup = main.instructions.iter().find_map(|instruction| {
        let failure_mode = match instruction {
            Instruction::CallOutcomeAggregate { failure_mode, .. }
            | Instruction::CallOutcomeDirectAggregate { failure_mode, .. } => failure_mode,
            _ => return None,
        };
        match failure_mode {
            OutcomeFailureMode::PropagateWithCleanup {
                code,
                message,
                instructions,
            } => {
                assert_eq!(*code, StrLocation::Local(1));
                assert_eq!(*message, StrLocation::Local(3));
                Some(instructions)
            }
            _ => None,
        }
    });
    let cleanup = cleanup.expect("expected fallible element initialization cleanup");
    assert_eq!(cleanup.len(), 2);
    assert!(matches!(
        &cleanup[0],
        Instruction::If {
            condition: BoolValue::UsizeComparison {
                operator: I32ComparisonOperator::Greater,
                left: UsizeValue::Location(UsizeLocation::Local(0)),
                right: UsizeValue::Const(1),
            },
            ..
        }
    ));
    assert!(matches!(
        &cleanup[1],
        Instruction::If {
            condition: BoolValue::UsizeComparison {
                operator: I32ComparisonOperator::Greater,
                left: UsizeValue::Location(UsizeLocation::Local(0)),
                right: UsizeValue::Const(0),
            },
            then_instructions,
            ..
        } if then_instructions == &vec![Instruction::CallVoid {
            target: CallTarget::same_file("File.drop"),
            arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                source: BorrowSource::AggregateSlotField {
                    slot_index: 0,
                    offset: 0,
                },
            })],
        }]
    ));
}

#[test]
fn tracks_completed_fixed_array_struct_fields_independently() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

struct Bundle {
    code: i32
    files: [File; 2]
}

func make_files(): [File; 2]! {
    return [File { fd: 1 }, File { fd: 2 }]
}

func main(): i32! {
    let bundle = Bundle { code: 42, files: make_files()? }
    return 0
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
    assert!(main.instructions.contains(&Instruction::SetBool {
        destination: BoolLocation::Local(0),
        value: BoolValue::Const(true),
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
            } if target == &CallTarget::same_file("make_files") => Some(instructions),
            _ => None,
        });

    assert!(matches!(
        cleanup.map(Vec::as_slice),
        Some([Instruction::If {
            condition: BoolValue::Location(BoolLocation::Local(0)),
            then_instructions,
            else_instructions,
        }]) if else_instructions.is_empty()
            && matches!(
                then_instructions.as_slice(),
                [Instruction::CallVoid { arguments: second, .. }, Instruction::CallVoid { arguments: first, .. }]
                    if matches!(
                        second.as_slice(),
                        [ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 8 }
                        })]
                    ) && matches!(
                        first.as_slice(),
                        [ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 4 }
                        })]
                    )
            )
    ));
}

#[test]
fn tracks_partial_fixed_array_literals_inside_struct_fields() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

struct Bundle {
    code: i32
    files: [File; 2]
}

func make_file(): File! {
    return File { fd: 2 }
}

func main(): i32! {
    let bundle = Bundle {
        code: 42,
        files: [File { fd: 1 }, make_file()?]
    }
    return 0
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
fn lowers_partial_fixed_array_replacement_cleanup_before_preserving_old_value() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func make_file(fd: i32): File! {
    return File { fd: fd }
}

func main(): i32! {
    var files: [File; 2] = [File { fd: 1 }, File { fd: 2 }]
    files = [File { fd: 3 }, make_file(4)?]
    return 0
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    let cleanup = main.instructions.iter().find_map(|instruction| {
        let failure_mode = match instruction {
            Instruction::CallOutcomeAggregate { failure_mode, .. }
            | Instruction::CallOutcomeDirectAggregate { failure_mode, .. } => failure_mode,
            _ => return None,
        };
        match failure_mode {
            OutcomeFailureMode::PropagateWithCleanup { instructions, .. } => Some(instructions),
            _ => None,
        }
    });
    let cleanup = cleanup.expect("expected replacement initialization cleanup");

    assert_eq!(cleanup.len(), 4);
    assert!(matches!(
        &cleanup[1],
        Instruction::If {
            condition: BoolValue::UsizeComparison {
                operator: I32ComparisonOperator::Greater,
                left: UsizeValue::Location(UsizeLocation::Local(0)),
                right: UsizeValue::Const(0),
            },
            then_instructions,
            ..
        } if matches!(
            then_instructions.as_slice(),
            [Instruction::CallVoid { arguments, .. }]
                if matches!(
                    arguments.as_slice(),
                    [ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlotField { slot_index: 1, offset: 0 }
                    })]
                )
        )
    ));
    assert!(matches!(
        &cleanup[2..],
        [
            Instruction::CallVoid { arguments: second, .. },
            Instruction::CallVoid { arguments: first, .. },
        ] if matches!(
            second.as_slice(),
            [ScalarArgument::Borrow(BorrowArgument {
                source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 4 }
            })]
        ) && matches!(
            first.as_slice(),
            [ScalarArgument::Borrow(BorrowArgument {
                source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 0 }
            })]
        )
    ));
}

#[test]
fn lowers_fixed_array_variable_index_compound_assignment() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func update(): void {
    var values: [i32; 2] = [1, 2]
    let index: usize = 1
    values[index] += addend()
    return
}

func addend(): i32 {
    return 7
}
"#,
        "update",
    );

    assert_eq!(
        function,
        Function {
            name: "update".to_string(),
            target: crate::ir::CallTarget::same_file("update".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: i32_const(1),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: i32_const(2),
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(0),
                    value: usize_const(1),
                },
                call_i32(I32Location::Local(1), "addend", vec![]),
                Instruction::LoadAggregateI32Indexed {
                    destination: I32Location::Local(2),
                    source: AggregateLocation::Slot(0),
                    base_offset: 0,
                    index: usize_local(0),
                    length: 2,
                    stride: 4,
                },
                Instruction::AddI32 {
                    destination: I32Location::Local(3),
                    left: i32_local(2),
                    right: i32_local(1),
                },
                Instruction::StoreAggregateI32Indexed {
                    destination: AggregateLocation::Slot(0),
                    base_offset: 0,
                    index: usize_local(0),
                    length: 2,
                    stride: 4,
                    value: i32_local(3),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_scalar_returning_fixed_array_access_through_mir_projection() {
    let function = lower_named_function(
        r#"func access(index: usize): i32 {
    var values: [i32; 2] = [7, 9]
    values[index] += 3
    return values[index]
}

func main(): i32 {
    return access(1)
}
"#,
        "access",
    );

    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::LoadAggregateI32Indexed {
            source: AggregateLocation::Slot(0),
            base_offset: 0,
            index: UsizeValue::Location(UsizeLocation::Parameter(0)),
            length: 2,
            stride: 4,
            ..
        }
    )));
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::StoreAggregateI32Indexed {
            destination: AggregateLocation::Slot(0),
            base_offset: 0,
            index: UsizeValue::Location(UsizeLocation::Parameter(0)),
            length: 2,
            stride: 4,
            ..
        }
    )));
}

#[test]
fn lowers_fixed_array_aggregate_field_indexing() {
    let function = lower_named_function(
        r#"struct Bag {
    values: [i32; 3]
    flags: [bool; 1]
    words: [&str; 2]
}

func main(): i32 {
    var bag = Bag {
        values: [1, 2, 3],
        flags: [false],
        words: ["bad", "bad"]
    }
    let index: usize = 1
    bag.values[0] = 20
    bag.values[index] += 20
    bag.flags[0] = true
    bag.words[index] = "Nocter"
    let total: i32 = bag.values[0] + bag.values[index]
    let flag: bool = bag.flags[0]
    let word: &str = bag.words[index]
    if total == 42 {
        if flag {
            if word.len() == 6 {
                return 42
            }
        }
    }
    return 1
}
"#,
        "main",
    );

    assert!(
        function
            .instructions
            .contains(&Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(48, 8),
            })
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(20),
            })
    );
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::StoreAggregateI32Indexed {
            destination: AggregateLocation::Slot(0),
            base_offset: 0,
            index,
            length: 3,
            stride: 4,
            ..
        } if index == &usize_local(0)
    )));
    assert!(
        function
            .instructions
            .contains(&Instruction::StoreAggregateBool {
                destination: AggregateLocation::Slot(0),
                offset: 12,
                value: BoolValue::Const(true),
            })
    );
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::StoreAggregateUsizeIndexed {
            destination: AggregateLocation::Slot(0),
            base_offset: 16,
            length: 2,
            stride: 16,
            ..
        }
    )));
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::LoadAggregateUsizeIndexed {
            source: AggregateLocation::Slot(0),
            base_offset: 16,
            length: 2,
            stride: 16,
            ..
        }
    )));
}

#[test]
fn lowers_fixed_array_aggregate_field_values() {
    let function = lower_named_function(
        r#"copy struct Bag {
    tag: i32
    values: [i32; 2]
}

func main(): i32 {
    return extract()[0]
}

func extract(): [i32; 2] {
    let bag = Bag { tag: 7, values: [20, 22] }
    let copied: [i32; 2] = bag.values
    return copied
}
"#,
        "extract",
    );

    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 0,
                source: AggregateLocation::Slot(0),
                source_offset: 4,
                layout: ValueLayout::new(8, 4),
            })
    );
    assert!(function.instructions.contains(&Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(1),
        layout: ValueLayout::new(8, 4),
    }));
}

#[test]
fn lowers_fixed_array_aggregate_field_assignments() {
    let function = lower_named_function(
        r#"copy struct Bag {
    tag: i32
    values: [i32; 2]
}

func main(): i32 {
    var bag = Bag { tag: 7, values: [1, 2] }
    let replacement: [i32; 2] = [20, 22]
    let other = Bag { tag: 8, values: [3, 4] }
    bag.values = [5, 6]
    bag.values = replacement
    bag.values = other.values
    return bag.values[0] + bag.values[1]
}
"#,
        "main",
    );

    assert!(
        function
            .instructions
            .contains(&Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 8,
                value: i32_const(6),
            })
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 4,
                source: AggregateLocation::Slot(1),
                source_offset: 0,
                layout: ValueLayout::new(8, 4),
            })
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 4,
                source: AggregateLocation::Slot(2),
                source_offset: 4,
                layout: ValueLayout::new(8, 4),
            })
    );
}

#[test]
fn lowers_zero_length_fixed_array_literal_binding() {
    let function = lower_named_function(
        r#"func main(): i32 {
    let empty: [u8; 0] = []
    return 42
}
"#,
        "main",
    );

    assert_eq!(
        function,
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(0, 1),
                },
                set_return_i32(42),
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn mir_projects_fixed_array_literal_leaf_paths_to_abi_offsets() {
    let function = lower_named_function(
        r#"func main(): i32 {
    let values: [i32; 3] = [10, 20, 30]
    return 0
}
"#,
        "main",
    );

    assert_eq!(
        function.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(12, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(10),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 4,
                value: i32_const(20),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 8,
                value: i32_const(30),
            },
            set_return_i32(0),
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_zero_length_fixed_array_copy_binding_and_assignment() {
    let function = lower_named_function(
        r#"func main(): i32 {
    var empty: [u8; 0] = []
    let copied: [u8; 0] = empty
    empty = []
    empty = copied
    return 42
}
"#,
        "main",
    );

    assert_eq!(
        function,
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(0, 1),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(0, 1),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(1),
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(0, 1),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::Slot(1),
                    layout: ValueLayout::new(0, 1),
                },
                set_return_i32(42),
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_fixed_array_call_result_assignments_to_existing_slots() {
    let source = r#"func main(): i32 {
    var values: [i32; 2] = [1, 2]
    var empty: [u8; 0] = []
    values = make_pair()
    values = make_fallible_pair()!
    empty = make_empty()
    empty = make_fallible_empty()!
    return values[0] + values[1]
}

func make_pair(): [i32; 2] {
    return [3, 4]
}

func make_fallible_pair(): [i32; 2]! {
    return [20, 22]
}

func make_empty(): [u8; 0] {
    return []
}

func make_fallible_empty(): [u8; 0]! {
    return []
}
"#;
    let pair_layout = ValueLayout::new(8, 4);
    let pair_type = Type::DirectAggregate {
        layout: pair_layout,
        words: 1,
    };
    let empty_layout = ValueLayout::new(0, 1);
    let empty_type = Type::DirectAggregate {
        layout: empty_layout,
        words: 0,
    };

    let main = lower_named_function_with_signatures(
        source,
        "main",
        function_signatures(vec![
            ("make_pair", pair_type.clone(), vec![]),
            (
                "make_fallible_pair",
                Type::Fallible(Box::new(pair_type)),
                vec![],
            ),
            ("make_empty", empty_type.clone(), vec![]),
            (
                "make_fallible_empty",
                Type::Fallible(Box::new(empty_type)),
                vec![],
            ),
        ]),
    )
    .unwrap();

    assert!(
        main.instructions
            .contains(&Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make_pair"),
                arguments: vec![],
                layout: pair_layout,
            })
    );
    assert!(
        main.instructions
            .contains(&Instruction::CallOutcomeDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make_fallible_pair"),
                arguments: vec![],
                layout: pair_layout,
                failure_mode: OutcomeFailureMode::Trap,
            })
    );
    assert!(
        main.instructions
            .contains(&Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make_empty"),
                arguments: vec![],
                layout: empty_layout,
            })
    );
    assert!(
        main.instructions
            .contains(&Instruction::CallOutcomeDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make_fallible_empty"),
                arguments: vec![],
                layout: empty_layout,
                failure_mode: OutcomeFailureMode::Trap,
            })
    );
}

#[test]
fn lowers_fixed_array_literal_value_arguments() {
    let source = r#"func main(): i32 {
    let answer: i32 = consume([20, 22], ["bad", "Nocter", "lang"], [])
    return answer
}

func consume(pair: [i32; 2], words: [&str; 3], empty: [u8; 0]): i32 {
    return pair[0] + pair[1]
}
"#;
    let pair_layout = ValueLayout::new(8, 4);
    let words_layout = ValueLayout::new(48, 8);
    let empty_layout = ValueLayout::new(0, 1);

    let main = lower_named_function_with_signatures(
        source,
        "main",
        function_signatures(vec![(
            "consume",
            Type::I32,
            vec![
                Type::DirectAggregate {
                    layout: pair_layout,
                    words: 1,
                },
                Type::DirectAggregate {
                    layout: words_layout,
                    words: 6,
                },
                Type::DirectAggregate {
                    layout: empty_layout,
                    words: 0,
                },
            ],
        )]),
    )
    .unwrap();

    assert!(
        main.instructions
            .contains(&Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: pair_layout,
            })
    );
    assert!(main.instructions.contains(&Instruction::StoreAggregateI32 {
        destination: AggregateLocation::Slot(0),
        offset: 4,
        value: i32_const(22),
    }));
    assert!(
        main.instructions
            .contains(&Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: words_layout,
            })
    );
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::SetStr { value, .. } if value == &str_static_value(b"Nocter")
    )));
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::StoreAggregateUsize {
            destination: AggregateLocation::Slot(1),
            offset: 16,
            ..
        }
    )));
    assert!(
        main.instructions
            .contains(&Instruction::ReserveAggregateSlot {
                slot_index: 2,
                layout: empty_layout,
            })
    );
    assert!(main.instructions.contains(&Instruction::CallI32 {
        destination: I32Location::Local(0),
        target: CallTarget::same_file("consume"),
        arguments: vec![
            ScalarArgument::AggregateDirect(DirectAggregateArgument {
                source: AggregateArgumentSource::Slot(0),
                layout: pair_layout,
                words: 1,
            }),
            ScalarArgument::AggregateDirect(DirectAggregateArgument {
                source: AggregateArgumentSource::Slot(1),
                layout: words_layout,
                words: 6,
            }),
            ScalarArgument::AggregateDirect(DirectAggregateArgument {
                source: AggregateArgumentSource::Slot(2),
                layout: empty_layout,
                words: 0,
            }),
        ],
    }));
}

#[test]
fn keeps_completed_owned_array_argument_live_until_call_starts() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func open(fd: i32): File! {
    return File { fd: fd }
}

func code(): i32! {
    return 4
}

func consume(files: [File; 2], value: i32): void {
    return
}

func main(): void! {
    consume([File { fd: 1 }, open(2)?], code()?)
    return
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    let code_cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeI32 {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("code") => Some(instructions),
            _ => None,
        });
    let code_cleanup = code_cleanup.expect("expected later argument cleanup");

    assert!(matches!(
        code_cleanup.as_slice(),
        [
            Instruction::CallVoid { arguments: second, .. },
            Instruction::CallVoid { arguments: first, .. },
        ] if matches!(
            second.as_slice(),
            [ScalarArgument::Borrow(BorrowArgument {
                source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 4 }
            })]
        ) && matches!(
            first.as_slice(),
            [ScalarArgument::Borrow(BorrowArgument {
                source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 0 }
            })]
        )
    ));
}

#[test]
fn lowers_partial_fixed_array_return_through_tracked_temporary() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func open(fd: i32): File! {
    return File { fd: fd }
}

func make(): [File; 2]! {
    return [File { fd: 1 }, open(2)?]
}

func main(): i32 {
    make()!
    return 0
}
"#,
    );
    let make = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("make"))
        .expect("expected lowered make function");
    let cleanup = make.instructions.iter().find_map(|instruction| {
        let failure_mode = match instruction {
            Instruction::CallOutcomeAggregate { failure_mode, .. }
            | Instruction::CallOutcomeDirectAggregate { failure_mode, .. } => failure_mode,
            _ => return None,
        };
        match failure_mode {
            OutcomeFailureMode::PropagateWithCleanup { instructions, .. } => Some(instructions),
            _ => None,
        }
    });
    let cleanup = cleanup.expect("expected return initialization cleanup");
    assert_eq!(cleanup.len(), 2);
    assert!(matches!(
        &cleanup[1],
        Instruction::If {
            then_instructions,
            ..
        } if matches!(
            then_instructions.as_slice(),
            [Instruction::CallVoid { arguments, .. }]
                if matches!(
                    arguments.as_slice(),
                    [ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 0 }
                    })]
                )
        )
    ));
    assert!(make.instructions.contains(&Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(0),
        layout: ValueLayout::new(8, 4),
    }));
}

#[test]
fn lowers_zero_length_fixed_array_parameters_calls_and_returns() {
    let source = r#"func main(): i32 {
    let empty: [u8; 0] = []
    let copied: [u8; 0] = identity(empty)
    let made: [u8; 0] = make_empty()
    let answer: i32 = consume(copied, made)
    return answer
}

func identity(values: [u8; 0]): [u8; 0] {
    return values
}

func make_empty(): [u8; 0] {
    return []
}

func consume(left: [u8; 0], right: [u8; 0]): i32 {
    return 42
}
"#;
    let layout = ValueLayout::new(0, 1);
    let aggregate_type = Type::DirectAggregate { layout, words: 0 };

    let identity = lower_named_function(source, "identity");
    assert_eq!(
        identity,
        Function {
            name: "identity".to_string(),
            target: crate::ir::CallTarget::same_file("identity".to_string()),
            return_type: aggregate_type.clone(),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    layout,
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout,
                },
                Instruction::Return,
            ],
        }
    );

    let make_empty = lower_named_function(source, "make_empty");
    assert_eq!(
        make_empty,
        Function {
            name: "make_empty".to_string(),
            target: crate::ir::CallTarget::same_file("make_empty".to_string()),
            return_type: aggregate_type.clone(),
            instructions: vec![Instruction::Return],
        }
    );

    let main = lower_named_function_with_signatures(
        source,
        "main",
        function_signatures(vec![
            (
                "identity",
                aggregate_type.clone(),
                vec![aggregate_type.clone()],
            ),
            ("make_empty", aggregate_type.clone(), vec![]),
            (
                "consume",
                Type::I32,
                vec![aggregate_type.clone(), aggregate_type.clone()],
            ),
        ]),
    )
    .unwrap();
    assert!(
        main.instructions
            .contains(&Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("identity"),
                arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout,
                    words: 0,
                })],
                layout,
            })
    );
    assert!(
        main.instructions
            .contains(&Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(2),
                target: CallTarget::same_file("make_empty"),
                arguments: vec![],
                layout,
            })
    );
    assert!(
        main.instructions.contains(&Instruction::CallI32 {
            destination: I32Location::Local(0),
            target: CallTarget::same_file("consume"),
            arguments: vec![
                ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(1),
                    layout,
                    words: 0,
                }),
                ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(2),
                    layout,
                    words: 0,
                }),
            ],
        }),
        "{main:?}"
    );
}
