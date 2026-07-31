use super::*;

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
                    destination: I32Location::Local(2),
                    left: i32_local(2),
                    right: i32_local(1),
                },
                Instruction::StoreAggregateI32Indexed {
                    destination: AggregateLocation::Slot(0),
                    base_offset: 0,
                    index: usize_local(0),
                    length: 2,
                    stride: 4,
                    value: i32_local(2),
                },
                Instruction::Return,
            ],
        }
    );
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
            .contains(&Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make_fallible_pair"),
                arguments: vec![],
                layout: pair_layout,
                failure_mode: FallibleFailureMode::Trap,
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
            .contains(&Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make_fallible_empty"),
                arguments: vec![],
                layout: empty_layout,
                failure_mode: FallibleFailureMode::Trap,
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
    assert!(main.instructions.contains(&Instruction::CallI32 {
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
    }));
}
