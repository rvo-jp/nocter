use super::*;

#[test]
fn lowers_five_byte_direct_aggregate_struct_literal_return_through_slot() {
    let function = lower_named_function(
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
    fourth: u8
    fifth: u8
}

func main(): i32 {
    return 0
}

func make(): Bytes {
    return Bytes { first: 1, second: 2, third: 3, fourth: 4, fifth: 42 }
}
"#,
        "make",
    );

    assert_eq!(
        function,
        Function {
            name: "make".to_string(),
            target: CallTarget::same_file("make"),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(5, 1),
                words: 1,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(5, 1),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: U8Value::Const(1),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 1,
                    value: U8Value::Const(2),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 2,
                    value: U8Value::Const(3),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 3,
                    value: U8Value::Const(4),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: U8Value::Const(42),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(5, 1),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_aggregate_u8_bool_and_usize_field_returns_from_local_slot() {
    let text = r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func read_tag(): u8 {
    let value = Header { tag: 7, ok: true, code: 42, len: 11 }
    return value.tag
}

func read_ok(): bool {
    let value = Header { tag: 7, ok: true, code: 42, len: 11 }
    return value.ok
}

func read_len(): usize {
    let value = Header { tag: 7, ok: true, code: 42, len: 11 }
    return value.len
}
"#;

    let tag = lower_named_function(text, "read_tag");
    let ok = lower_named_function(text, "read_ok");
    let len = lower_named_function(text, "read_len");

    assert!(
        tag.instructions.contains(&Instruction::LoadAggregateU8 {
            destination: U8Location::Return,
            source: AggregateLocation::Slot(0),
            offset: 0,
        }),
        "{tag:?}"
    );
    assert!(
        ok.instructions.contains(&Instruction::LoadAggregateBool {
            destination: BoolLocation::Return,
            source: AggregateLocation::Slot(0),
            offset: 1,
        }),
        "{ok:?}"
    );
    assert!(
        len.instructions.contains(&Instruction::LoadAggregateUsize {
            destination: UsizeLocation::Return,
            source: AggregateLocation::Slot(0),
            offset: 8,
        }),
        "{len:?}"
    );
}

#[test]
fn lowers_u8_parameter_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func echo(byte: u8): u8 {
    return byte
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: u8_param(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_arithmetic_and_shifts() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func calculate(left: u8, right: u8): u8 {
    let sum: u8 = left + right
    let difference: u8 = sum - 1
    let product: u8 = difference * 2
    let quotient: u8 = product / right
    let remainder: u8 = quotient % 5
    let shifted_left: u8 = remainder << 1
    return shifted_left >> 1
}
"#,
        "calculate",
    );

    assert_eq!(
        function,
        Function {
            name: "calculate".to_string(),
            target: crate::ir::CallTarget::same_file("calculate".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::AddU8 {
                    destination: U8Location::Local(0),
                    left: u8_param(0),
                    right: u8_param(1),
                },
                Instruction::SubtractU8 {
                    destination: U8Location::Local(1),
                    left: u8_local(0),
                    right: u8_const(1),
                },
                Instruction::MultiplyU8 {
                    destination: U8Location::Local(2),
                    left: u8_local(1),
                    right: u8_const(2),
                },
                Instruction::DivideU8 {
                    destination: U8Location::Local(3),
                    left: u8_local(2),
                    right: u8_param(1),
                },
                Instruction::RemainderU8 {
                    destination: U8Location::Local(4),
                    left: u8_local(3),
                    right: u8_const(5),
                },
                Instruction::ShiftLeftU8 {
                    destination: U8Location::Local(5),
                    left: u8_local(4),
                    right: u8_const(1),
                },
                Instruction::ShiftRightU8 {
                    destination: U8Location::Return,
                    left: u8_local(5),
                    right: u8_const(1),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_compound_assignment_with_call_rhs() {
    let ir = lower_text(
        r#"func main(): i32 {
    var total: u8 = 40
    total += answer()
    return total as i32
}

func answer(): u8 {
    return 2
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetU8 {
                        destination: U8Location::Local(0),
                        value: u8_const(40),
                    },
                    call_u8(U8Location::Local(1), "answer", vec![]),
                    Instruction::AddU8 {
                        destination: U8Location::Local(0),
                        left: u8_local(0),
                        right: u8_local(1),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: I32Value::U8ZeroExtend(Box::new(u8_local(0))),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::U8,
                instructions: vec![
                    Instruction::SetU8 {
                        destination: U8Location::Return,
                        value: u8_const(2),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_u8_local_binding_and_normal_call() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func wrapper(): u8 {
    let byte: u8 = identity(7)
    return byte
}

func identity(byte: u8): u8 {
    return byte
}
"#,
        "wrapper",
        function_signatures(vec![("identity", Type::U8, vec![Type::U8])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::U8,
            instructions: vec![
                call_u8(
                    U8Location::Local(0),
                    "identity",
                    vec![ScalarArgument::U8(u8_const(7))],
                ),
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: u8_local(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_entry_u8_let_initializer_call_with_indexed_signature() {
    let ir = lower_text(
        r#"func main(): i32 {
    let byte: u8 = identity(7)
    return 0
}

func identity(byte: u8): u8 {
    return byte
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    call_u8(
                        U8Location::Local(0),
                        "identity",
                        vec![ScalarArgument::U8(u8_const(7))],
                    ),
                    set_return_i32(0),
                    Instruction::Return,
                ],
            },
            Function {
                name: "identity".to_string(),
                target: crate::ir::CallTarget::same_file("identity".to_string()),
                return_type: Type::U8,
                instructions: vec![
                    Instruction::SetU8 {
                        destination: U8Location::Return,
                        value: u8_param(0),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_byte_literal_bool_return_comparison() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func is_a(): bool {
    return b'\x41' == b'A'
}
"#,
        "is_a",
    );

    assert_eq!(
        function,
        Function {
            name: "is_a".to_string(),
            target: crate::ir::CallTarget::same_file("is_a".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(u8_const(65))),
                        right: I32Value::U8ZeroExtend(Box::new(u8_const(65))),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_alias_conversion_bool_return_comparison() {
    let function = lower_named_function(
        r#"type Byte = u8

func main(): i32 {
    return 0
}

func is_elf(bytes: &[u8]): bool {
    return (bytes[0] as Byte) == (0x7F as Byte)
}
"#,
        "is_elf",
    );

    assert_eq!(
        function,
        Function {
            name: "is_elf".to_string(),
            target: crate::ir::CallTarget::same_file("is_elf".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Local(0),
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: usize_const(0),
                    },
                },
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(
                            U8Location::Local(0),
                        ))),
                        right: I32Value::U8ZeroExtend(Box::new(u8_const(0x7F))),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_normal_call_comparison() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func check(byte: u8): bool {
    return identity(byte) != 0
}

func identity(byte: u8): u8 {
    return byte
}
"#,
        "check",
        function_signatures(vec![("identity", Type::U8, vec![Type::U8])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "check".to_string(),
            target: crate::ir::CallTarget::same_file("check".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_u8(
                    U8Location::Local(0),
                    "identity",
                    vec![ScalarArgument::U8(u8_param(0))],
                ),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::NotEqual,
                        left: I32Value::U8ZeroExtend(Box::new(u8_local(0))),
                        right: I32Value::U8ZeroExtend(Box::new(u8_const(0))),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_index_conversion_to_i32_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(text: &str): i32 {
    return text[0] as i32
}
"#,
        "first",
    );

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::U8ZeroExtend(Box::new(U8Value::StrIndex {
                        source: StrLocation::Parameter(0),
                        index: usize_const(0),
                    })),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_index_conversion_to_i32_alias_return() {
    let function = lower_named_function(
        r#"type Exit = i32

func main(): i32 {
    return 0
}

func first(text: &str): Exit {
    return text[0] as Exit
}
"#,
        "first",
    );

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::U8ZeroExtend(Box::new(U8Value::StrIndex {
                        source: StrLocation::Parameter(0),
                        index: usize_const(0),
                    })),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_index_conversion_to_usize_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(bytes: &[u8]): usize {
    return bytes[1] as usize
}
"#,
        "first",
    );

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: usize_const(1),
                    })),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_index_conversion_to_usize_alias_return() {
    let function = lower_named_function(
        r#"type Index = usize

func main(): i32 {
    return 0
}

func first(bytes: &[u8]): Index {
    return bytes[1] as Index
}
"#,
        "first",
    );

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: usize_const(1),
                    })),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_aggregate_field_compound_assignment_with_call_rhs() {
    let ir = lower_text(
        r#"struct Counter {
    pad: u8
    value: u8
}

func main(): i32 {
    var counter = Counter { pad: 0, value: 40 }
    counter.value += answer()
    return counter.value as i32
}

func answer(): u8 {
    return 2
}
"#,
    );

    let instructions = &ir.functions[0].instructions;
    assert!(instructions.contains(&call_u8(U8Location::Local(0), "answer", vec![])));
    assert!(instructions.contains(&Instruction::LoadAggregateU8 {
        destination: U8Location::Local(1),
        source: AggregateLocation::Slot(0),
        offset: 1,
    }));
    assert!(instructions.contains(&Instruction::AddU8 {
        destination: U8Location::Local(1),
        left: u8_local(1),
        right: u8_local(0),
    }));
    assert!(instructions.contains(&Instruction::StoreAggregateU8 {
        destination: AggregateLocation::Slot(0),
        offset: 1,
        value: u8_local(1),
    }));
}

#[test]
fn lowers_u8_returning_function_with_terminal_if() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func choose(flag: bool): u8 {
    if flag {
        return 7
    } else {
        return 9
    }
}
"#,
        "choose",
    );

    assert_eq!(
        function,
        Function {
            name: "choose".to_string(),
            target: crate::ir::CallTarget::same_file("choose".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Parameter(0)),
                    then_instructions: vec![Instruction::SetU8 {
                        destination: U8Location::Return,
                        value: u8_const(7),
                    }],
                    else_instructions: vec![Instruction::SetU8 {
                        destination: U8Location::Return,
                        value: u8_const(9),
                    }],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_returning_function_with_byte_literal() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func byte(): u8 {
    return b'\x41'
}
"#,
        "byte",
    );

    assert_eq!(
        function,
        Function {
            name: "byte".to_string(),
            target: crate::ir::CallTarget::same_file("byte".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: u8_const(65),
                },
                Instruction::Return,
            ],
        }
    );
}
