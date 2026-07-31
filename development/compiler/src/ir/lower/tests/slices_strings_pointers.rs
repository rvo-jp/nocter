use super::*;

#[test]
fn indexes_slice_function_signature_parameter_types() {
    let analysis = analyze_text(
        r#"func main(): i32 {
    return 0
}

func consume(bytes: &[u8], scratch: &+[u8]): i32 {
    return 0
}
"#,
    );
    let root = analysis.root_file().unwrap();
    let index = FunctionIndex::new(&analysis, root.ast.span.source);
    let signatures = index.signatures();

    assert_eq!(
        signatures.parameter_types(&CallTarget::same_file("consume")),
        Some(vec![readonly_u8_slice_type(), readwrite_u8_slice_type()].as_slice())
    );
    assert_eq!(
        signatures.parameter_abi_word_count(&CallTarget::same_file("consume")),
        Some(4)
    );
}

#[test]
fn lowers_direct_aggregate_struct_literal_value_argument() {
    let aggregate_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
    let function = lower_named_function_with_signatures(
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func consume(header: Header): i32 {
    return header.code
}

func main(): i32 {
    let result = consume(Header { tag: 7, ok: true, code: 42, len: 11 })
    return result
}
"#,
        "main",
        function_signatures(vec![("consume", Type::I32, vec![aggregate_type])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: u8_const(7),
                },
                Instruction::StoreAggregateBool {
                    destination: AggregateLocation::Slot(0),
                    offset: 1,
                    value: BoolValue::Const(true),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: i32_const(42),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: usize_const(11),
                },
                Instruction::CallI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                        layout: ValueLayout::new(16, 8),
                        words: 2,
                    })],
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_local(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_indirect_aggregate_usize_struct_literal_return() {
    let function = lower_named_function(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func make(): Text {
    return Text { start: 1, len: 2, capacity: 3 }
}
"#,
        "make",
    );

    assert_eq!(
        function,
        Function {
            name: "make".to_string(),
            target: CallTarget::same_file("make"),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 0,
                    value: usize_const(1),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 8,
                    value: usize_const(2),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 16,
                    value: usize_const(3),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_indirect_aggregate_terminal_if_struct_literal_return() {
    let function = lower_named_function(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func choose(flag: bool): Text {
    if flag {
        return Text { start: 1, len: 2, capacity: 3 }
    } else {
        return Text { start: 4, len: 5, capacity: 6 }
    }
}
"#,
        "choose",
    );

    assert_eq!(
        function,
        Function {
            name: "choose".to_string(),
            target: CallTarget::same_file("choose"),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
            instructions: vec![Instruction::If {
                condition: BoolValue::Location(BoolLocation::Parameter(0)),
                then_instructions: vec![
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Return,
                        offset: 0,
                        value: usize_const(1),
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Return,
                        offset: 8,
                        value: usize_const(2),
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Return,
                        offset: 16,
                        value: usize_const(3),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Return,
                        offset: 0,
                        value: usize_const(4),
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Return,
                        offset: 8,
                        value: usize_const(5),
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Return,
                        offset: 16,
                        value: usize_const(6),
                    },
                    Instruction::Return,
                ],
            }],
        }
    );
}

#[test]
fn lowers_indirect_aggregate_scalar_struct_literal_return() {
    let function = lower_named_function(
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func make(): Header {
    return Header { tag: 7, ok: true, code: 42, len: 11, capacity: 12 }
}
"#,
        "make",
    );

    assert_eq!(
        function,
        Function {
            name: "make".to_string(),
            target: CallTarget::same_file("make"),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
            instructions: vec![
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Return,
                    offset: 0,
                    value: U8Value::Const(7),
                },
                Instruction::StoreAggregateBool {
                    destination: AggregateLocation::Return,
                    offset: 1,
                    value: BoolValue::Const(true),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Return,
                    offset: 4,
                    value: I32Value::Const(42),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 8,
                    value: usize_const(11),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 16,
                    value: usize_const(12),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_indirect_aggregate_struct_literal_binding_return() {
    let function = lower_named_function(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func forward(): Text {
    let value = Text { start: 1, len: 2, capacity: 3 }
    return move value
}
"#,
        "forward",
    );

    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    assert_eq!(
        function,
        Function {
            name: "forward".to_string(),
            target: CallTarget::same_file("forward"),
            return_type: aggregate_type,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(1),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: usize_const(2),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 16,
                    value: usize_const(3),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Return,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_indirect_aggregate_struct_literal_binding_move_return() {
    let function = lower_named_function(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func forward(): Text {
    let value = Text { start: 1, len: 2, capacity: 3 }
    return move value
}
"#,
        "forward",
    );

    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    assert_eq!(
        function,
        Function {
            name: "forward".to_string(),
            target: CallTarget::same_file("forward"),
            return_type: aggregate_type,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(1),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: usize_const(2),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 16,
                    value: usize_const(3),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Return,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_indirect_aggregate_struct_literal_binding_borrow_argument() {
    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func touch(value: &+Text): void {
    return
}

func forward(): Text {
    var value = Text { start: 1, len: 2, capacity: 3 }
    touch(&+value)
    return move value
}
"#,
        "forward",
        function_signatures(vec![(
            "touch",
            Type::Void,
            vec![Type::Borrow {
                is_readwrite: true,
                inner: Box::new(aggregate_type.clone()),
            }],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "forward".to_string(),
            target: CallTarget::same_file("forward"),
            return_type: aggregate_type,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(1),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: usize_const(2),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 16,
                    value: usize_const(3),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("touch"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlot(0),
                    })],
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Return,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_replacement_drop_for_aggregate_struct_literal_assignment() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    file = File { fd: 2 }
    return 0
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(1),
                offset: 0,
                value: i32_const(2),
            },
            drop_call.clone(),
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::Slot(1),
                layout: ValueLayout::new(4, 4),
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_call,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_replacement_drop_for_moved_aggregate_struct_literal_field_assignment() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Holder {
    file: File
}

impl Holder {
    drop &+self {
        return
    }
}

func main(): i32 {
    var source = File { fd: 1 }
    var holder = Holder { file: File { fd: 2 } }
    holder = Holder { file: move source }
    return holder.file.fd
}
"#,
    );

    let drop_holder = Instruction::CallVoid {
        target: CallTarget::same_file("Holder.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(1),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(1),
                offset: 0,
                value: i32_const(2),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 2,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(2),
                destination_offset: 0,
                source: AggregateLocation::Slot(0),
                source_offset: 0,
                layout: ValueLayout::new(4, 4),
            },
            drop_holder.clone(),
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(1),
                source: AggregateLocation::Slot(2),
                layout: ValueLayout::new(4, 4),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Local(0),
                source: AggregateLocation::Slot(1),
                offset: 0,
            },
            drop_holder,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_direct_aggregate_usize_struct_literal_return() {
    let function = lower_named_function(
        r#"struct Pair {
    first: usize
    second: usize
}

func main(): i32 {
    return 0
}

func make(): Pair {
    return Pair { first: 1, second: 2 }
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
                layout: ValueLayout::new(16, 8),
                words: 2,
            },
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::DirectReturn,
                    offset: 0,
                    value: usize_const(1),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::DirectReturn,
                    offset: 8,
                    value: usize_const(2),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_direct_aggregate_terminal_if_struct_literal_return() {
    let function = lower_named_function(
        r#"struct Pair {
    first: usize
    second: usize
}

func main(): i32 {
    return 0
}

func choose(flag: bool): Pair {
    if flag {
        return Pair { first: 1, second: 2 }
    } else {
        return Pair { first: 3, second: 4 }
    }
}
"#,
        "choose",
    );

    assert_eq!(
        function,
        Function {
            name: "choose".to_string(),
            target: CallTarget::same_file("choose"),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(16, 8),
                words: 2,
            },
            instructions: vec![Instruction::If {
                condition: BoolValue::Location(BoolLocation::Parameter(0)),
                then_instructions: vec![
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::DirectReturn,
                        offset: 0,
                        value: usize_const(1),
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::DirectReturn,
                        offset: 8,
                        value: usize_const(2),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::DirectReturn,
                        offset: 0,
                        value: usize_const(3),
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::DirectReturn,
                        offset: 8,
                        value: usize_const(4),
                    },
                    Instruction::Return,
                ],
            }],
        }
    );
}

#[test]
fn lowers_direct_aggregate_struct_literal_return_after_scope_drop() {
    let file_type = Type::DirectAggregate {
        layout: ValueLayout::new(4, 4),
        words: 1,
    };
    let choose = lower_named_function_with_signatures(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Pair {
    first: usize
    second: usize
}

func main(): i32 {
    return 0
}

func choose(): Pair {
    var file = File { fd: 3 }
    return Pair { first: 1, second: 2 }
}
"#,
        "choose",
        function_signatures(vec![(
            "File.drop",
            Type::Void,
            vec![Type::Borrow {
                is_readwrite: true,
                inner: Box::new(file_type),
            }],
        )]),
    )
    .unwrap();

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let pair_layout = ValueLayout::new(16, 8);
    assert_eq!(
        choose.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(3),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: pair_layout,
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(1),
                offset: 0,
                value: usize_const(1),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(1),
                offset: 8,
                value: usize_const(2),
            },
            drop_call,
            Instruction::CopyAggregate {
                destination: AggregateLocation::DirectReturn,
                source: AggregateLocation::Slot(1),
                layout: pair_layout,
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_direct_aggregate_terminal_if_struct_literal_return_after_scope_drop() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Pair {
    first: usize
    second: usize
}

func main(): i32 {
    let pair = choose(true)
    return 0
}

func choose(flag: bool): Pair {
    var file = File { fd: 3 }
    if flag {
        return Pair { first: 1, second: 2 }
    } else {
        return Pair { first: 3, second: 4 }
    }
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let pair_layout = ValueLayout::new(16, 8);
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
    assert_eq!(
        function,
        &Function {
            name: "choose".to_string(),
            target: CallTarget::same_file("choose"),
            return_type: Type::DirectAggregate {
                layout: pair_layout,
                words: 2,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: i32_const(3),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Parameter(0)),
                    then_instructions: vec![
                        Instruction::ReserveAggregateSlot {
                            slot_index: 1,
                            layout: pair_layout,
                        },
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::Slot(1),
                            offset: 0,
                            value: usize_const(1),
                        },
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::Slot(1),
                            offset: 8,
                            value: usize_const(2),
                        },
                        drop_call.clone(),
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::DirectReturn,
                            source: AggregateLocation::Slot(1),
                            layout: pair_layout,
                        },
                        Instruction::Return,
                    ],
                    else_instructions: vec![
                        Instruction::ReserveAggregateSlot {
                            slot_index: 2,
                            layout: pair_layout,
                        },
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::Slot(2),
                            offset: 0,
                            value: usize_const(3),
                        },
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::Slot(2),
                            offset: 8,
                            value: usize_const(4),
                        },
                        drop_call,
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::DirectReturn,
                            source: AggregateLocation::Slot(2),
                            layout: pair_layout,
                        },
                        Instruction::Return,
                    ],
                },
            ],
        }
    );
}

#[test]
fn lowers_small_direct_aggregate_struct_literal_return_through_slot() {
    let function = lower_named_function(
        r#"struct Code {
    value: i32
}

func main(): i32 {
    return 0
}

func make(): Code {
    return Code { value: 42 }
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
                layout: ValueLayout::new(4, 4),
                words: 1,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: I32Value::Const(42),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_concrete_generic_aggregate_struct_literal_return() {
    let function = lower_named_function(
        r#"struct Box<T> {
    value: T
}

func main(): i32 {
    return 0
}

func make(): Box<i32> {
    return Box<i32> { value: 42 }
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
                layout: ValueLayout::new(4, 4),
                words: 1,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: I32Value::Const(42),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::Return,
            ],
        }
    );
}

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
fn lowers_direct_aggregate_scalar_struct_literal_binding_return() {
    let function = lower_named_function(
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
}

func main(): i32 {
    return 0
}

func make(): Header {
    let value = Header { tag: 7, ok: false, code: 42 }
    return move value
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
                layout: ValueLayout::new(8, 4),
                words: 1,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: U8Value::Const(7),
                },
                Instruction::StoreAggregateBool {
                    destination: AggregateLocation::Slot(0),
                    offset: 1,
                    value: BoolValue::Const(false),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: I32Value::Const(42),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_direct_aggregate_scalar_struct_literal_return_through_slot() {
    let function = lower_named_function(
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
}

func main(): i32 {
    return 0
}

func make(): Header {
    return Header { tag: 7, ok: false, code: 42 }
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
                layout: ValueLayout::new(8, 4),
                words: 1,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: U8Value::Const(7),
                },
                Instruction::StoreAggregateBool {
                    destination: AggregateLocation::Slot(0),
                    offset: 1,
                    value: BoolValue::Const(false),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: I32Value::Const(42),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_moved_aggregate_struct_literal_field_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Holder {
    file: File
}

impl Holder {
    drop &+self {
        return
    }
}

func main(): i32 {
    let holder = make_holder()
    return holder.file.fd
}

func make_holder(): Holder {
    var file = File { fd: 42 }
    return Holder { file: move file }
}
"#,
    );

    let make_holder = ir
        .functions
        .iter()
        .find(|function| function.name == "make_holder")
        .unwrap();
    assert_eq!(
        make_holder.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(42),
            },
            Instruction::CopyAggregateRange {
                destination: AggregateLocation::DirectReturn,
                destination_offset: 0,
                source: AggregateLocation::Slot(0),
                source_offset: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_u16_aggregate_struct_literal_return() {
    let function = lower_named_function_with_signatures(
        r#"struct Header {
    tag: u8
    code: u16
}

func main(): i32 {
    return 0
}

func make(): Header {
    return Header { tag: 7, code: 42 }
}
"#,
        "make",
        context::FunctionSignatures::new(HashMap::new()),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "make".to_string(),
            target: CallTarget::same_file("make"),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(4, 2),
                words: 1,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(4, 2),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: U8Value::Const(7),
                },
                Instruction::StoreAggregateU16 {
                    destination: AggregateLocation::Slot(0),
                    offset: 2,
                    value: 42,
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(4, 2),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u32_aggregate_struct_literal_argument() {
    let ir = lower_text(
        r#"struct Header {
    tag: u8
    code: u32
}

func main(): i32 {
    consume(Header { tag: 7, code: 42 })
    return 0
}

func consume(header: Header): void {
    return
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();

    assert!(
        main.instructions.contains(&Instruction::StoreAggregateU32 {
            destination: AggregateLocation::Slot(0),
            offset: 4,
            value: 42,
        }),
        "{main:?}"
    );
    assert!(
        main.instructions.contains(&Instruction::CallVoid {
            target: CallTarget::same_file("consume"),
            arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                source: AggregateArgumentSource::Slot(0),
                layout: ValueLayout::new(8, 4),
                words: 1,
            })],
        }),
        "{main:?}"
    );
}

#[test]
fn lowers_direct_aggregate_struct_literal_return_field_call_through_distinct_slot() {
    let pair_type = Type::DirectAggregate {
        layout: ValueLayout::new(8, 4),
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Pair {
    first: i32
    second: i32
}

copy struct Wrap {
    pair: Pair
    code: i32
}

func main(): i32 {
    return 0
}

func make_pair(): Pair {
    return Pair { first: 1, second: 2 }
}

func make_wrap(): Wrap {
    return Wrap { pair: make_pair(), code: 42 }
}
"#,
        "make_wrap",
        function_signatures(vec![("make_pair", pair_type, vec![])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "make_wrap".to_string(),
            target: CallTarget::same_file("make_wrap"),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(12, 4),
                words: 2,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(12, 4),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::CallDirectAggregate {
                    destination: AggregateLocation::Slot(1),
                    target: CallTarget::same_file("make_pair"),
                    arguments: vec![],
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Slot(0),
                    destination_offset: 0,
                    source: AggregateLocation::Slot(1),
                    source_offset: 0,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: I32Value::Const(42),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(12, 4),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_direct_aggregate_struct_literal_binding_return() {
    let function = lower_named_function(
        r#"struct Allocator {
    state: usize
    kind: u64
}

func main(): i32 {
    return 0
}

func make(): Allocator {
    let allocator = Allocator { state: 1, kind: 2 }
    return move allocator
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
                layout: ValueLayout::new(16, 8),
                words: 2,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(1),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: usize_const(2),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_aggregate_struct_literal_assignment_borrow_argument() {
    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func touch(value: &+Text): void {
    return
}

func use_text(): i32 {
    var value = Text { start: 1, len: 2, capacity: 3 }
    value = Text { start: 4, len: 5, capacity: 6 }
    touch(&+value)
    return 0
}
"#,
        "use_text",
        function_signatures(vec![(
            "touch",
            Type::Void,
            vec![Type::Borrow {
                is_readwrite: true,
                inner: Box::new(aggregate_type),
            }],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "use_text".to_string(),
            target: CallTarget::same_file("use_text"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(1),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: usize_const(2),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 16,
                    value: usize_const(3),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(4),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: usize_const(5),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 16,
                    value: usize_const(6),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("touch"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlot(0),
                    })],
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_pointer_from_addr_aggregate_field_return() {
    let function = lower_imported_named_function_with_nocter_home_files(
        r#"use std/text.make

func main(): i32 {
    return 0
}
"#,
        "make",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
"#,
            ),
            (
                "std/text.nct",
                r#"use std/ptr.from_addr

pub struct Text {
    ptr: *u8
    len: usize
    capacity: usize
}

pub func make(): Text {
    return Text { ptr: from_addr(1), len: 2, capacity: 3 }
}
"#,
            ),
        ],
    );

    assert_eq!(function.name, "make");
    assert!(matches!(
        function.target,
        CallTarget::Imported { ref name, .. } if name == "make"
    ));
    assert_eq!(
        function.return_type,
        Type::Aggregate {
            layout: ValueLayout::new(24, 8),
        }
    );
    assert_eq!(
        function.instructions,
        vec![
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Return,
                offset: 0,
                value: usize_const(1),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Return,
                offset: 8,
                value: usize_const(2),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Return,
                offset: 16,
                value: usize_const(3),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_pointer_from_addr_aggregate_field_binding_return() {
    let function = lower_imported_named_function_with_nocter_home_files(
        r#"use std/text.make

func main(): i32 {
    return 0
}
"#,
        "make",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
"#,
            ),
            (
                "std/text.nct",
                r#"use std/ptr.from_addr

pub struct Text {
    ptr: *u8
    len: usize
    capacity: usize
}

pub func make(): Text {
    let value = Text { ptr: from_addr(1), len: 2, capacity: 3 }
    return move value
}
"#,
            ),
        ],
    );

    assert_eq!(function.name, "make");
    assert!(matches!(
        function.target,
        CallTarget::Imported { ref name, .. } if name == "make"
    ));
    assert_eq!(
        function.return_type,
        Type::Aggregate {
            layout: ValueLayout::new(24, 8),
        }
    );
    assert_eq!(
        function.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(24, 8),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: usize_const(1),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(0),
                offset: 8,
                value: usize_const(2),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(0),
                offset: 16,
                value: usize_const(3),
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Return,
                source: AggregateLocation::Slot(0),
                layout: ValueLayout::new(24, 8),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_pointer_from_ref_scalar_borrow_parameter_binding_return() {
    let function = lower_named_function_with_nocter_home_files(
        r#"use std/ptr.{addr, from_ref}

func address_of(value: &u8): usize {
    let pointer = from_ref(value)
    return addr(pointer)
}

func main(): i32 {
    return 0
}
"#,
        "address_of",
        &[(
            "std/ptr.nct",
            r#"pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
"#,
        )],
    );

    assert_eq!(
        function,
        Function {
            name: "address_of".to_string(),
            target: CallTarget::same_file("address_of"),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsizeFromBorrow {
                    destination: UsizeLocation::Local(0),
                    source: BorrowSource::BorrowParameter(0),
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::Location(UsizeLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_pointer_from_ref_local_borrow_binding() {
    let function = lower_named_function_with_nocter_home_files(
        r#"use std/ptr.{addr, from_ref}

func main(): i32 {
    let value: u8 = 1
    let pointer = from_ref(&value)
    let address: usize = addr(pointer)
    return 0
}
"#,
        "main",
        &[(
            "std/ptr.nct",
            r#"pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
"#,
        )],
    );

    assert_eq!(
        function,
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Local(0),
                    value: u8_const(1),
                },
                Instruction::SetUsizeFromBorrow {
                    destination: UsizeLocation::Local(1),
                    source: BorrowSource::U8(U8Location::Local(0)),
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(2),
                    value: UsizeValue::Location(UsizeLocation::Local(1)),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_pointer_from_ref_direct_addr_local_borrow() {
    let function = lower_named_function_with_nocter_home_files(
        r#"use std/ptr.{addr, from_ref}

func main(): i32 {
    let value: u8 = 1
    let address: usize = addr(from_ref(&value))
    return 0
}
"#,
        "main",
        &[(
            "std/ptr.nct",
            r#"pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
"#,
        )],
    );

    assert_eq!(
        function,
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Local(0),
                    value: u8_const(1),
                },
                Instruction::SetUsizeFromBorrow {
                    destination: UsizeLocation::Local(1),
                    source: BorrowSource::U8(U8Location::Local(0)),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_moved_aggregate_struct_literal_field() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Holder {
    file: File
}

impl Holder {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 42 }
    var holder = Holder { file: move file }
    return holder.file.fd
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(42),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 0,
                source: AggregateLocation::Slot(0),
                source_offset: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Local(0),
                source: AggregateLocation::Slot(1),
                offset: 0,
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("Holder.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(1),
                })],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_nested_aggregate_struct_literal_argument_field_call_through_distinct_slot() {
    let packet_type = Type::Aggregate {
        layout: ValueLayout::new(32, 8),
    };
    let header_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func make_header(): Header {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}

func consume(packet: Packet): i32 {
    return packet.header.code
}

func main(): i32 {
    return consume(Packet { prefix: 1, header: make_header(), tail: 2 })
}
"#,
        "main",
        function_signatures(vec![
            ("make_header", header_type, vec![]),
            ("consume", Type::I32, vec![packet_type]),
        ]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(32, 8),
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make_header"),
                arguments: vec![],
                layout: ValueLayout::new(16, 8),
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 8,
                source: AggregateLocation::Slot(1),
                source_offset: 0,
                layout: ValueLayout::new(16, 8),
            }),
        "{function:?}"
    );
    assert!(
        function.instructions.contains(&Instruction::CallI32 {
            destination: I32Location::Return,
            target: CallTarget::same_file("consume"),
            arguments: vec![ScalarArgument::AggregateIndirect(AggregateArgument {
                source: AggregateArgumentSource::Slot(0),
            })],
        }),
        "{function:?}"
    );
}

#[test]
fn lowers_nested_aggregate_field_struct_literal_assignment() {
    let function = lower_named_function(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    return 0
}

func update(): i32 {
    var packet = Packet { prefix: 1, header: Header { tag: 7, ok: false, code: 1, len: 11 }, tail: 2 }
    packet.header = Header { tag: 8, ok: true, code: 42, len: 12 }
    return packet.header.code
}
"#,
        "update",
    );

    assert_eq!(
        function,
        Function {
            name: "update".to_string(),
            target: CallTarget::same_file("update"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(32, 8),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(1),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: u8_const(7),
                },
                Instruction::StoreAggregateBool {
                    destination: AggregateLocation::Slot(0),
                    offset: 9,
                    value: BoolValue::Const(false),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 12,
                    value: i32_const(1),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 16,
                    value: usize_const(11),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 24,
                    value: usize_const(2),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: u8_const(8),
                },
                Instruction::StoreAggregateBool {
                    destination: AggregateLocation::Slot(0),
                    offset: 9,
                    value: BoolValue::Const(true),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 12,
                    value: i32_const(42),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 16,
                    value: usize_const(12),
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 12,
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_nested_aggregate_struct_literal_field_from_local() {
    let function = lower_named_function(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    return 0
}

func build(): i32 {
    let header = Header { tag: 7, ok: true, code: 42, len: 11 }
    let packet = Packet { prefix: 1, header: header, tail: 2 }
    return packet.header.code
}
"#,
        "build",
    );

    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 8,
                source: AggregateLocation::Slot(0),
                source_offset: 0,
                layout: ValueLayout::new(16, 8),
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::LoadAggregateI32 {
                destination: I32Location::Return,
                source: AggregateLocation::Slot(1),
                offset: 12,
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_nested_aggregate_struct_literal_field_from_call() {
    let header_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    return 0
}

func make_header(): Header {
    return Header { tag: 8, ok: true, code: 42, len: 12 }
}

func build(): i32 {
    let packet = Packet { prefix: 1, header: make_header(), tail: 2 }
    return packet.header.code
}
"#,
        "build",
        function_signatures(vec![("make_header", header_type, vec![])]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make_header"),
                arguments: vec![],
                layout: ValueLayout::new(16, 8),
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 8,
                source: AggregateLocation::Slot(1),
                source_offset: 0,
                layout: ValueLayout::new(16, 8),
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::LoadAggregateI32 {
                destination: I32Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 12,
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_nested_aggregate_struct_literal_field_from_call_result_member() {
    let packet_type = Type::Aggregate {
        layout: ValueLayout::new(32, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    return 0
}

func make(): Packet {
    return Packet { prefix: 1, header: Header { tag: 8, ok: true, code: 42, len: 12 }, tail: 2 }
}

func build(): i32 {
    let packet = Packet { prefix: 1, header: make().header, tail: 2 }
    return packet.header.code
}
"#,
        "build",
        function_signatures(vec![("make", packet_type, vec![])]),
    )
    .unwrap();

    assert!(
        function.instructions.contains(&Instruction::CallAggregate {
            destination: AggregateLocation::Slot(1),
            target: CallTarget::same_file("make"),
            arguments: vec![],
        }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 8,
                source: AggregateLocation::Slot(1),
                source_offset: 8,
                layout: ValueLayout::new(16, 8),
            }),
        "{function:?}"
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
fn lowers_aggregate_pointer_never_call_as_normal_call_then_trap() {
    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Big {
    first: usize
    second: usize
    code: usize
}

func main(): i32 {
    let value = Big { first: 1, second: 2, code: 42 }
    return abort(value)
}

func abort(value: Big): never {
    abort(value)
}
"#,
        "main",
        function_signatures(vec![("abort", Type::Never, vec![aggregate_type.clone()])]),
    )
    .unwrap();

    assert!(
        function.instructions.contains(&Instruction::CallVoid {
            target: CallTarget::same_file("abort"),
            arguments: vec![ScalarArgument::AggregateIndirect(AggregateArgument {
                source: AggregateArgumentSource::Slot(0),
            })],
        }),
        "{function:?}"
    );
    assert_eq!(function.instructions.last(), Some(&Instruction::Trap));
}

#[test]
fn lowers_str_literal_call_argument_as_two_abi_words() {
    let ir = lower_text(
        r#"func main(): i32 {
    return consume("Nocter", 42)
}

func consume(name: &str, code: i32): i32 {
    return code
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
                instructions: vec![Instruction::TailCall {
                    target: CallTarget::same_file("consume"),
                    arguments: vec![
                        str_static(b"Nocter"),
                        ScalarArgument::I32(I32Value::Const(42)),
                    ],
                }],
            },
            Function {
                name: "consume".to_string(),
                target: crate::ir::CallTarget::same_file("consume".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_param(2),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_readwrite_slice_index_borrow_call_argument() {
    let function = lower_named_function_with_signatures(
        r#"func touch(value: &+i32): void {
    return
}

func use_first(values: &+[i32]): void {
    touch(&+values[0])
    return
}

func main(): void {
    return
}
"#,
        "use_first",
        function_signatures(vec![(
            "touch",
            Type::Void,
            vec![Type::Borrow {
                is_readwrite: true,
                inner: Box::new(Type::I32),
            }],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "use_first".to_string(),
            target: CallTarget::same_file("use_first"),
            return_type: Type::Void,
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("touch"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: SliceElementIndex::Const(0),
                            element: SliceElementAddressKind::I32,
                        },
                    })],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_parameter_forwarding_call_argument() {
    let ir = lower_text(
        r#"func main(): i32 {
    return wrapper("Nocter")
}

func wrapper(name: &str): i32 {
    return consume(name, 42)
}

func consume(name: &str, code: i32): i32 {
    return code
}
"#,
    );

    assert_eq!(
        ir.functions[1],
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("consume"),
                arguments: vec![
                    ScalarArgument::Str(StrValue::Location(StrLocation::Parameter(0))),
                    ScalarArgument::I32(I32Value::Const(42)),
                ],
            }],
        }
    );
}

#[test]
fn lowers_str_literal_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func title(): &str {
    return "Nocter"
}
"#,
        "title",
    );

    assert_eq!(
        function,
        Function {
            name: "title".to_string(),
            target: crate::ir::CallTarget::same_file("title".to_string()),
            return_type: Type::Str,
            instructions: vec![
                Instruction::SetStr {
                    destination: StrLocation::Return,
                    value: str_static_value(b"Nocter"),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_parameter_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func echo(name: &str): &str {
    return name
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: Type::Str,
            instructions: vec![
                Instruction::SetStr {
                    destination: StrLocation::Return,
                    value: StrValue::Location(StrLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_alias_parameter_and_return() {
    let function = lower_named_function(
        r#"type Text = str

func main(): i32 {
    return 0
}

func echo(name: &Text): &Text {
    return name
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: Type::Str,
            instructions: vec![
                Instruction::SetStr {
                    destination: StrLocation::Return,
                    value: StrValue::Location(StrLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_alias_annotated_local_binding() {
    let function = lower_named_function(
        r#"type Text = str

func main(): i32 {
    return 0
}

func echo(name: &Text): &Text {
    let view: &Text = name
    return view
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: Type::Str,
            instructions: vec![
                Instruction::SetStr {
                    destination: StrLocation::Local(0),
                    value: StrValue::Location(StrLocation::Parameter(0)),
                },
                Instruction::SetStr {
                    destination: StrLocation::Return,
                    value: StrValue::Location(StrLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_view_alias_annotated_local_binding() {
    let function = lower_named_function(
        r#"type TextView = &str

func main(): i32 {
    return 0
}

func echo(name: TextView): TextView {
    let view: TextView = name
    return view
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: Type::Str,
            instructions: vec![
                Instruction::SetStr {
                    destination: StrLocation::Local(0),
                    value: StrValue::Location(StrLocation::Parameter(0)),
                },
                Instruction::SetStr {
                    destination: StrLocation::Return,
                    value: StrValue::Location(StrLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_tail_call_return() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func alias(): &str {
    return title()
}

func title(): &str {
    return "Nocter"
}
"#,
        "alias",
        context::FunctionSignatures::new(HashMap::from([("title".to_string(), Type::Str)])),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "alias".to_string(),
            target: crate::ir::CallTarget::same_file("alias".to_string()),
            return_type: Type::Str,
            instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("title"),
                arguments: vec![],
            }],
        }
    );
}

#[test]
fn lowers_str_normal_call_result_as_call_argument() {
    let ir = lower_text(
        r#"func main(): i32 {
    return consume(title(), 42)
}

func title(): &str {
    return "Nocter"
}

func consume(name: &str, code: i32): i32 {
    return code
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
                    call_str(StrLocation::Local(0), "title", vec![]),
                    Instruction::TailCall {
                        target: CallTarget::same_file("consume"),
                        arguments: vec![
                            ScalarArgument::Str(StrValue::Location(StrLocation::Local(0))),
                            ScalarArgument::I32(I32Value::Const(42)),
                        ],
                    },
                ],
            },
            Function {
                name: "title".to_string(),
                target: crate::ir::CallTarget::same_file("title".to_string()),
                return_type: Type::Str,
                instructions: vec![
                    Instruction::SetStr {
                        destination: StrLocation::Return,
                        value: str_static_value(b"Nocter"),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "consume".to_string(),
                target: crate::ir::CallTarget::same_file("consume".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_param(2),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_str_let_initializer_normal_call() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func wrapper(): &str {
    let text: &str = title()
    return text
}

func title(): &str {
    return "Nocter"
}
"#,
        "wrapper",
    );

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::Str,
            instructions: vec![
                call_str(StrLocation::Local(0), "title", vec![]),
                Instruction::SetStr {
                    destination: StrLocation::Return,
                    value: StrValue::Location(StrLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_inferred_str_let_initializer_normal_call() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func wrapper(): &str {
    let text = title()
    return text
}

func title(): &str {
    return "Nocter"
}
"#,
        "wrapper",
    );

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::Str,
            instructions: vec![
                call_str(StrLocation::Local(0), "title", vec![]),
                Instruction::SetStr {
                    destination: StrLocation::Return,
                    value: StrValue::Location(StrLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_slice_parameter_forwarding_call_argument() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func wrapper(bytes: &[u8]): i32 {
    return consume(bytes, 42)
}

func consume(bytes: &[u8], code: i32): i32 {
    return code
}
"#,
        "wrapper",
        function_signatures(vec![(
            "consume",
            Type::I32,
            vec![readonly_u8_slice_type(), Type::I32],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("consume"),
                arguments: vec![
                    ScalarArgument::Slice(SliceValue::Location(SliceLocation::Parameter(0))),
                    ScalarArgument::I32(I32Value::Const(42)),
                ],
            }],
        }
    );
}

#[test]
fn lowers_readwrite_u8_slice_parameter_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func echo(bytes: &+[u8]): &+[u8] {
    return bytes
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: readwrite_u8_slice_type(),
            instructions: vec![
                Instruction::SetSlice {
                    destination: SliceLocation::Return,
                    value: SliceValue::Location(SliceLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_u8_slice_index_assignment() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func fill(bytes: &+[u8]): void {
    bytes[0] = 7
    return
}
"#,
        "fill",
    );

    assert_eq!(
        function,
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreU8ToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: UsizeValue::Const(0),
                    value: U8Value::Const(7),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_u8_slice_index_compound_assignment() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func update(values: &+[u8]): void {
    values[1] += 2
    return
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
                Instruction::SetU8 {
                    destination: U8Location::Local(0),
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: usize_const(1),
                    },
                },
                Instruction::AddU8 {
                    destination: U8Location::Local(0),
                    left: u8_local(0),
                    right: u8_const(2),
                },
                Instruction::StoreU8ToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: usize_const(1),
                    value: u8_local(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_u8_call_result_slice_index_assignment() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func fill(bytes: &+[u8]): void {
    identity(bytes)[1] = 9
    return
}

func identity(bytes: &+[u8]): &+[u8] {
    return bytes
}
"#,
        "fill",
        function_signatures(vec![(
            "identity",
            readwrite_u8_slice_type(),
            vec![readwrite_u8_slice_type()],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::StoreU8ToSliceIndex {
                    destination: SliceLocation::Local(0),
                    index: UsizeValue::Const(1),
                    value: U8Value::Const(9),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_u8_call_result_slice_index_assignment_without_temporary_collision() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func fill(bytes: &+[u8], indices: &[usize]): void {
    identity(bytes)[indices[0]] = byte()
    return
}

func identity(bytes: &+[u8]): &+[u8] {
    return bytes
}

func byte(): u8 {
    return 7
}
"#,
        "fill",
        function_signatures(vec![
            (
                "identity",
                readwrite_u8_slice_type(),
                vec![readwrite_u8_slice_type()],
            ),
            ("byte", Type::U8, vec![]),
        ]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(2),
                    value: UsizeValue::SliceIndex {
                        source: SliceLocation::Parameter(2),
                        index: Box::new(usize_const(0)),
                    },
                },
                call_u8(U8Location::Local(3), "byte", vec![]),
                Instruction::StoreU8ToSliceIndex {
                    destination: SliceLocation::Local(0),
                    index: usize_local(2),
                    value: U8Value::Location(U8Location::Local(3)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_i32_slice_index_compound_assignment() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func update(values: &+[i32]): void {
    values[1] += 2
    return
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
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: I32Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: usize_const(1),
                    },
                },
                Instruction::AddI32 {
                    destination: I32Location::Local(0),
                    left: i32_local(0),
                    right: i32_const(2),
                },
                Instruction::StoreI32ToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: usize_const(1),
                    value: i32_local(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_usize_slice_index_compound_assignment() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func update(values: &+[usize]): void {
    values[0] %= 5
    return
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
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(0),
                    value: usize_slice_index(SliceLocation::Parameter(0), usize_const(0)),
                },
                Instruction::RemainderUsize {
                    destination: UsizeLocation::Local(0),
                    left: usize_local(0),
                    right: usize_const(5),
                },
                Instruction::StoreUsizeToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: usize_const(0),
                    value: usize_local(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_i32_call_result_slice_index_compound_assignment() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func update(): void {
    values()[1] += addend()
    return
}

func values(): &+[i32] {
    return values()
}

func addend(): i32 {
    return 2
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
                call_slice(SliceLocation::Local(0), "values", vec![]),
                call_i32(I32Location::Local(2), "addend", vec![]),
                Instruction::SetI32 {
                    destination: I32Location::Local(3),
                    value: I32Value::SliceIndex {
                        source: SliceLocation::Local(0),
                        index: usize_const(1),
                    },
                },
                Instruction::AddI32 {
                    destination: I32Location::Local(3),
                    left: i32_local(3),
                    right: i32_local(2),
                },
                Instruction::StoreI32ToSliceIndex {
                    destination: SliceLocation::Local(0),
                    index: usize_const(1),
                    value: i32_local(3),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_slice_alias_parameter_and_return() {
    let function = lower_named_function(
        r#"type Bytes = [u8]

func main(): i32 {
    return 0
}

func echo(bytes: &+Bytes): &+Bytes {
    return bytes
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: readwrite_u8_slice_type(),
            instructions: vec![
                Instruction::SetSlice {
                    destination: SliceLocation::Return,
                    value: SliceValue::Location(SliceLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_slice_alias_annotated_local_binding() {
    let function = lower_named_function(
        r#"type Bytes = [u8]

func main(): i32 {
    return 0
}

func echo(bytes: &+Bytes): &+Bytes {
    let view: &+Bytes = bytes
    return view
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: readwrite_u8_slice_type(),
            instructions: vec![
                Instruction::SetSlice {
                    destination: SliceLocation::Local(0),
                    value: SliceValue::Location(SliceLocation::Parameter(0)),
                },
                Instruction::SetSlice {
                    destination: SliceLocation::Return,
                    value: SliceValue::Location(SliceLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_inferred_u8_slice_local_binding() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func echo(bytes: &[u8]): &[u8] {
    let view = bytes
    return view
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: readonly_u8_slice_type(),
            instructions: vec![
                Instruction::SetSlice {
                    destination: SliceLocation::Local(0),
                    value: SliceValue::Location(SliceLocation::Parameter(0)),
                },
                Instruction::SetSlice {
                    destination: SliceLocation::Return,
                    value: SliceValue::Location(SliceLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_inferred_readwrite_u8_slice_local_binding() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func echo(bytes: &+[u8]): &+[u8] {
    let view = bytes
    return view
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: readwrite_u8_slice_type(),
            instructions: vec![
                Instruction::SetSlice {
                    destination: SliceLocation::Local(0),
                    value: SliceValue::Location(SliceLocation::Parameter(0)),
                },
                Instruction::SetSlice {
                    destination: SliceLocation::Return,
                    value: SliceValue::Location(SliceLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_slice_view_alias_annotated_local_binding() {
    let function = lower_named_function(
        r#"type BytesView = &+[u8]

func main(): i32 {
    return 0
}

func echo(bytes: BytesView): BytesView {
    let view: BytesView = bytes
    return view
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: readwrite_u8_slice_type(),
            instructions: vec![
                Instruction::SetSlice {
                    destination: SliceLocation::Local(0),
                    value: SliceValue::Location(SliceLocation::Parameter(0)),
                },
                Instruction::SetSlice {
                    destination: SliceLocation::Return,
                    value: SliceValue::Location(SliceLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_slice_normal_call_result_as_call_argument() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func wrapper(bytes: &[u8]): i32 {
    return consume(identity(bytes), 42)
}

func identity(bytes: &[u8]): &[u8] {
    return bytes
}

func consume(bytes: &[u8], code: i32): i32 {
    return code
}
"#,
        "wrapper",
        function_signatures(vec![
            (
                "identity",
                readonly_u8_slice_type(),
                vec![readonly_u8_slice_type()],
            ),
            (
                "consume",
                Type::I32,
                vec![readonly_u8_slice_type(), Type::I32],
            ),
        ]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::I32,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::TailCall {
                    target: CallTarget::same_file("consume"),
                    arguments: vec![
                        ScalarArgument::Slice(SliceValue::Location(SliceLocation::Local(0))),
                        ScalarArgument::I32(I32Value::Const(42)),
                    ],
                },
            ],
        }
    );
}

#[test]
fn lowers_u8_slice_len_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func size(bytes: &[u8]): usize {
    return bytes.len()
}
"#,
        "size",
    );

    assert_eq!(
        function,
        Function {
            name: "size".to_string(),
            target: crate::ir::CallTarget::same_file("size".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::SliceLen(SliceLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_u8_slice_len_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func size(bytes: &+[u8]): usize {
    return bytes.len()
}
"#,
        "size",
    );

    assert_eq!(
        function,
        Function {
            name: "size".to_string(),
            target: crate::ir::CallTarget::same_file("size".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::SliceLen(SliceLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_non_byte_slice_len_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func size(values: &[usize]): usize {
    return values.len()
}
"#,
        "size",
    );

    assert_eq!(
        function,
        Function {
            name: "size".to_string(),
            target: crate::ir::CallTarget::same_file("size".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::SliceLen(SliceLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_non_byte_slice_identifier_local_len_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func size(values: &[usize]): usize {
    let copy = values
    return copy.len()
}
"#,
        "size",
    );

    assert_eq!(
        function,
        Function {
            name: "size".to_string(),
            target: crate::ir::CallTarget::same_file("size".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetSlice {
                    destination: SliceLocation::Local(0),
                    value: SliceValue::Location(SliceLocation::Parameter(0)),
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::SliceLen(SliceLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_usize_slice_index_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(values: &[usize]): usize {
    return values[0]
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
                    value: usize_slice_index(SliceLocation::Parameter(0), usize_const(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_slice_index_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(values: &[&str]): &str {
    return values[0]
}
"#,
        "first",
    );

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::Str,
            instructions: vec![
                Instruction::SetStr {
                    destination: StrLocation::Return,
                    value: StrValue::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: usize_const(0),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_usize_slice_call_result_index_return() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func first(values: &[usize]): usize {
    return identity(values)[0]
}

func identity(values: &[usize]): &[usize] {
    return values
}
"#,
        "first",
        function_signatures(vec![(
            "identity",
            Type::Slice {
                is_readwrite: false,
            },
            vec![Type::Slice {
                is_readwrite: false,
            }],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: usize_slice_index(SliceLocation::Local(0), usize_const(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_usize_slice_index_comparison_condition() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func choose(values: &[usize]): i32 {
    if values[0] == 42 {
        return 1
    } else {
        return 2
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
            return_type: Type::I32,
            instructions: vec![Instruction::If {
                condition: BoolValue::UsizeComparison {
                    operator: I32ComparisonOperator::Equal,
                    left: usize_slice_index(SliceLocation::Parameter(0), usize_const(0)),
                    right: usize_const(42),
                },
                then_instructions: vec![set_return_i32(1), Instruction::Return],
                else_instructions: vec![set_return_i32(2), Instruction::Return],
            }],
        }
    );
}

#[test]
fn lowers_u8_slice_len_comparison_condition() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func choose(bytes: &[u8]): i32 {
    if bytes.len() == 0 {
        return 42
    } else {
        return 7
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
            return_type: Type::I32,
            instructions: vec![Instruction::If {
                condition: BoolValue::UsizeComparison {
                    operator: I32ComparisonOperator::Equal,
                    left: usize_slice_len(SliceLocation::Parameter(0)),
                    right: usize_const(0),
                },
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            }],
        }
    );
}

#[test]
fn lowers_u8_slice_call_result_len_comparison_condition() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func choose(bytes: &[u8]): i32 {
    if identity(bytes).len() != 0 {
        return 42
    } else {
        return 7
    }
}

func identity(bytes: &[u8]): &[u8] {
    return bytes
}
"#,
        "choose",
        function_signatures(vec![(
            "identity",
            readonly_u8_slice_type(),
            vec![readonly_u8_slice_type()],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "choose".to_string(),
            target: crate::ir::CallTarget::same_file("choose".to_string()),
            return_type: Type::I32,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::If {
                    condition: BoolValue::UsizeComparison {
                        operator: I32ComparisonOperator::NotEqual,
                        left: usize_slice_len(SliceLocation::Local(0)),
                        right: usize_const(0),
                    },
                    then_instructions: vec![set_return_i32(42), Instruction::Return],
                    else_instructions: vec![set_return_i32(7), Instruction::Return],
                },
            ],
        }
    );
}

#[test]
fn lowers_u8_slice_is_empty_condition() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func choose(bytes: &[u8]): i32 {
    if bytes.is_empty() {
        return 42
    } else {
        return 7
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
            return_type: Type::I32,
            instructions: vec![Instruction::If {
                condition: BoolValue::UsizeComparison {
                    operator: I32ComparisonOperator::Equal,
                    left: usize_slice_len(SliceLocation::Parameter(0)),
                    right: usize_const(0),
                },
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            }],
        }
    );
}

#[test]
fn lowers_non_byte_slice_call_result_is_empty_return() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func empty(values: &[usize]): bool {
    return identity(values).is_empty()
}

func identity(values: &[usize]): &[usize] {
    return values
}
"#,
        "empty",
        function_signatures(vec![(
            "identity",
            Type::Slice {
                is_readwrite: false,
            },
            vec![Type::Slice {
                is_readwrite: false,
            }],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "empty".to_string(),
            target: crate::ir::CallTarget::same_file("empty".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::UsizeComparison {
                        operator: I32ComparisonOperator::Equal,
                        left: usize_slice_len(SliceLocation::Local(0)),
                        right: usize_const(0),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_parameter_len_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func size(text: &str): usize {
    return text.len()
}
"#,
        "size",
    );

    assert_eq!(
        function,
        Function {
            name: "size".to_string(),
            target: crate::ir::CallTarget::same_file("size".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::StrLen(StrLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_literal_len_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func size(): usize {
    return "Nocter".len()
}
"#,
        "size",
    );

    assert_eq!(
        function,
        Function {
            name: "size".to_string(),
            target: crate::ir::CallTarget::same_file("size".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::Const(6),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_slice_call_result_len_return() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func size(bytes: &[u8]): usize {
    return identity(bytes).len()
}

func identity(bytes: &[u8]): &[u8] {
    return bytes
}
"#,
        "size",
        function_signatures(vec![(
            "identity",
            readonly_u8_slice_type(),
            vec![readonly_u8_slice_type()],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "size".to_string(),
            target: crate::ir::CallTarget::same_file("size".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::SliceLen(SliceLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_call_result_len_return() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func size(text: &str): usize {
    return identity(text).len()
}

func identity(text: &str): &str {
    return text
}
"#,
        "size",
        function_signatures(vec![("identity", Type::Str, vec![Type::Str])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "size".to_string(),
            target: crate::ir::CallTarget::same_file("size".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                call_str(
                    StrLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Str(StrValue::Location(
                        StrLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::StrLen(StrLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_call_result_is_empty_return() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func empty(text: &str): bool {
    return identity(text).is_empty()
}

func identity(text: &str): &str {
    return text
}
"#,
        "empty",
        function_signatures(vec![("identity", Type::Str, vec![Type::Str])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "empty".to_string(),
            target: crate::ir::CallTarget::same_file("empty".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_str(
                    StrLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Str(StrValue::Location(
                        StrLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::UsizeComparison {
                        operator: I32ComparisonOperator::Equal,
                        left: UsizeValue::StrLen(StrLocation::Local(0)),
                        right: usize_const(0),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_is_empty_bool_comparison_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func empty(text: &str): bool {
    return text.is_empty() == false
}
"#,
        "empty",
    );

    assert_eq!(
        function,
        Function {
            name: "empty".to_string(),
            target: crate::ir::CallTarget::same_file("empty".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::BoolComparison {
                        operator: BoolComparisonOperator::Equal,
                        left: Box::new(BoolValue::UsizeComparison {
                            operator: I32ComparisonOperator::Equal,
                            left: UsizeValue::StrLen(StrLocation::Parameter(0)),
                            right: usize_const(0),
                        }),
                        right: Box::new(BoolValue::Const(false)),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_slice_index_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(bytes: &[u8]): u8 {
    return bytes[0]
}
"#,
        "first",
    );

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: usize_const(0),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_u8_slice_index_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(bytes: &+[u8]): u8 {
    return bytes[1]
}
"#,
        "first",
    );

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: usize_const(1),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_i32_slice_index_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(numbers: &[i32]): i32 {
    return numbers[0]
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
                    value: I32Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: usize_const(0),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_bool_slice_index_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(flags: &[bool]): bool {
    return flags[1]
}
"#,
        "first",
    );

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: usize_const(1),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_parameter_index_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(text: &str): u8 {
    return text[2]
}
"#,
        "first",
    );

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::StrIndex {
                        source: StrLocation::Parameter(0),
                        index: usize_const(2),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_literal_index_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(): u8 {
    return "Nocter"[3]
}
"#,
        "first",
    );

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::StaticStrIndex {
                        bytes: b"Nocter".to_vec(),
                        index: usize_const(3),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_slice_call_result_index_return() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func first(bytes: &[u8]): u8 {
    return identity(bytes)[0]
}

func identity(bytes: &[u8]): &[u8] {
    return bytes
}
"#,
        "first",
        function_signatures(vec![(
            "identity",
            readonly_u8_slice_type(),
            vec![readonly_u8_slice_type()],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::U8,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Local(0),
                        index: usize_const(0),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_str_call_result_index_return() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func first(text: &str): u8 {
    return identity(text)[0]
}

func identity(text: &str): &str {
    return text
}
"#,
        "first",
        function_signatures(vec![("identity", Type::Str, vec![Type::Str])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::U8,
            instructions: vec![
                call_str(
                    StrLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Str(StrValue::Location(
                        StrLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::StrIndex {
                        source: StrLocation::Local(0),
                        index: usize_const(0),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_slice_call_result_index_bool_return_comparison() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func check(bytes: &[u8]): bool {
    return identity(bytes)[0] == 1
}

func identity(bytes: &[u8]): &[u8] {
    return bytes
}
"#,
        "check",
        function_signatures(vec![(
            "identity",
            readonly_u8_slice_type(),
            vec![readonly_u8_slice_type()],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "check".to_string(),
            target: crate::ir::CallTarget::same_file("check".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                            source: SliceLocation::Local(0),
                            index: usize_const(0),
                        })),
                        right: I32Value::U8ZeroExtend(Box::new(u8_const(1))),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_i32_slice_call_result_index_bool_return_comparison() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func check(numbers: &[i32]): bool {
    return identity(numbers)[0] == 11
}

func identity(numbers: &[i32]): &[i32] {
    return numbers
}
"#,
        "check",
        function_signatures(vec![(
            "identity",
            readonly_u8_slice_type(),
            vec![readonly_u8_slice_type()],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "check".to_string(),
            target: crate::ir::CallTarget::same_file("check".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::SliceIndex {
                            source: SliceLocation::Local(0),
                            index: usize_const(0),
                        },
                        right: i32_const(11),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_bool_slice_call_result_index_bool_return_comparison() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func check(flags: &[bool]): bool {
    return identity(flags)[0] == true
}

func identity(flags: &[bool]): &[bool] {
    return flags
}
"#,
        "check",
        function_signatures(vec![(
            "identity",
            readonly_u8_slice_type(),
            vec![readonly_u8_slice_type()],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "check".to_string(),
            target: crate::ir::CallTarget::same_file("check".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::BoolComparison {
                        operator: BoolComparisonOperator::Equal,
                        left: Box::new(BoolValue::SliceIndex {
                            source: SliceLocation::Local(0),
                            index: usize_const(0),
                        }),
                        right: Box::new(BoolValue::Const(true)),
                    },
                },
                Instruction::Return,
            ],
        }
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
fn lowers_u8_slice_index_bool_return_comparison() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func is_elf(bytes: &[u8]): bool {
    return bytes[0] == 0x7F
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
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: usize_const(0),
                        })),
                        right: I32Value::U8ZeroExtend(Box::new(u8_const(0x7F))),
                    },
                },
                Instruction::Return,
            ],
        }
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
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: usize_const(0),
                        })),
                        right: I32Value::U8ZeroExtend(Box::new(u8_const(0x7F))),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_str_index_terminal_if_comparison() {
    let ir = lower_text(
        r#"func main(): i32 {
    if "Nocter"[0] == 78 {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::If {
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Equal,
                    left: I32Value::U8ZeroExtend(Box::new(U8Value::StaticStrIndex {
                        bytes: b"Nocter".to_vec(),
                        index: usize_const(0),
                    })),
                    right: I32Value::U8ZeroExtend(Box::new(u8_const(78))),
                },
                then_instructions: vec![set_return_i32(0), Instruction::Return],
                else_instructions: vec![set_return_i32(1), Instruction::Return],
            }],
        }])
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
fn lowers_entry_static_str_index_conversion_to_i32() {
    let ir = lower_text(
        r#"func main(): i32 {
    return "A"[0] as i32
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::U8ZeroExtend(Box::new(U8Value::StaticStrIndex {
                        bytes: b"A".to_vec(),
                        index: usize_const(0),
                    })),
                },
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_ignored_str_call_expression_statement() {
    let ir = lower_text(
        r#"func main(): i32 {
    text()
    return 0
}

func text(): &str {
    return "ignored"
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
                    call_str(StrLocation::Local(0), "text", vec![]),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_const(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "text".to_string(),
                target: crate::ir::CallTarget::same_file("text".to_string()),
                return_type: Type::Str,
                instructions: vec![
                    Instruction::SetStr {
                        destination: StrLocation::Return,
                        value: str_static_value(b"ignored"),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_ignored_slice_call_expression_statement() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func wrapper(bytes: &[u8]): i32 {
    identity(bytes)
    return 0
}

func identity(bytes: &[u8]): &[u8] {
    return bytes
}
"#,
        "wrapper",
        function_signatures(vec![(
            "identity",
            readonly_u8_slice_type(),
            vec![readonly_u8_slice_type()],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::I32,
            instructions: vec![
                call_slice(
                    SliceLocation::Local(0),
                    "identity",
                    vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                ),
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_const(0),
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
fn lowers_close_fd_raw_call() {
    let close = lower_imported_named_function_with_nocter_home_files(
        r#"use std/io_close.close_raw

func main(): void {
    return
}
"#,
        "close_raw",
        &[
            std_io_file(),
            (
                "std/io_close.nct",
                r#"use std/io.close_fd_raw

pub func close_raw(fd: i32): void {
    close_fd_raw(fd)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        close.instructions,
        vec![
            Instruction::CloseFd {
                fd: I32Value::Location(I32Location::Parameter(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_u8_to_ptr_call() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_nul

func main(): void {
    return
}
"#,
        "store_nul",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_u8_to_ptr(destination: *u8, offset: usize, value: u8): void
"#,
            ),
            (
                "std/ptr_store.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_u8_to_ptr

pub func store_nul(address: usize, offset: usize): void {
    store_u8_to_ptr(from_addr(address), offset, 0)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        store.instructions,
        vec![
            Instruction::StoreU8ToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                value: U8Value::Const(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_copy_ptr_to_ptr_call() {
    let copy = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_copy.copy

func main(): void {
    return
}
"#,
        "copy",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive copy_ptr_to_ptr(destination: *u8, source: *u8, byte_count: usize): void
"#,
            ),
            (
                "std/ptr_copy.nct",
                r#"use std/ptr.copy_ptr_to_ptr
use std/ptr.from_addr

pub func copy(destination: usize, source: usize, byte_count: usize): void {
    copy_ptr_to_ptr(from_addr(destination), from_addr(source), byte_count)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        copy.instructions,
        vec![
            Instruction::CopyPointerBytes {
                destination: UsizeValue::Location(UsizeLocation::Parameter(0)),
                source: UsizeValue::Location(UsizeLocation::Parameter(1)),
                byte_count: UsizeValue::Location(UsizeLocation::Parameter(2)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_usize() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_word

func main(): void {
    return
}
"#,
        "store_word",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

pub func store_word(address: usize, offset: usize, value: usize): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        store.instructions,
        vec![
            Instruction::StoreUsizeToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                value: UsizeValue::Location(UsizeLocation::Parameter(2)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_i32() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_number

func main(): void {
    return
}
"#,
        "store_number",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

pub func store_number(address: usize, offset: usize, value: i32): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        store.instructions,
        vec![
            Instruction::StoreI32ToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                value: I32Value::Location(I32Location::Parameter(2)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_bool() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_flag

func main(): void {
    return
}
"#,
        "store_flag",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

pub func store_flag(address: usize, offset: usize, value: bool): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        store.instructions,
        vec![
            Instruction::StoreBoolToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                value: BoolValue::Location(BoolLocation::Parameter(2)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_str() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_text

func main(): void {
    return
}
"#,
        "store_text",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

pub func store_text(address: usize, offset: usize, value: &str): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        store.instructions,
        vec![
            Instruction::StoreStrToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                value: StrValue::Location(StrLocation::Parameter(2)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_copy_aggregate() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_pair

func main(): void {
    return
}
"#,
        "store_pair",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

copy struct Pair {
    value: i32
}

pub func store_pair(address: usize, offset: usize, value: Pair): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    let pair_layout = ValueLayout { size: 4, align: 4 };
    assert_eq!(
        store.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: pair_layout,
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::DirectParameter { start_index: 2 },
                layout: pair_layout,
            },
            Instruction::CopyAggregateToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                source: AggregateLocation::Slot(0),
                layout: pair_layout,
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_pointee_size_call_for_usize_pointer_field() {
    let size = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_size.size

func main(): void {
    return
}
"#,
        "size",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive pointee_size<T>(pointer: *T): usize
"#,
            ),
            (
                "std/ptr_size.nct",
                r#"use std/ptr.pointee_size

pub copy struct Holder {
    pub ptr: *usize
}

pub func size(holder: Holder): usize {
    return pointee_size(holder.ptr)
}
"#,
            ),
        ],
    );

    assert!(
        size.instructions.contains(&Instruction::SetUsize {
            destination: UsizeLocation::Return,
            value: UsizeValue::Const(8),
        }),
        "{:?}",
        size.instructions
    );
    assert!(
        !size.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallUsize { target, .. } if call_target_name_is(target, "pointee_size")
        )),
        "{:?}",
        size.instructions
    );
}

#[test]
fn lowers_pointee_size_call_for_u8_pointer_field() {
    let size = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_size.size

func main(): void {
    return
}
"#,
        "size",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive pointee_size<T>(pointer: *T): usize
"#,
            ),
            (
                "std/ptr_size.nct",
                r#"use std/ptr.pointee_size

pub copy struct Holder {
    pub ptr: *u8
}

pub func size(holder: Holder): usize {
    return pointee_size(holder.ptr)
}
"#,
            ),
        ],
    );

    assert!(
        size.instructions.contains(&Instruction::SetUsize {
            destination: UsizeLocation::Return,
            value: UsizeValue::Const(1),
        }),
        "{:?}",
        size.instructions
    );
}

#[test]
fn lowers_slice_from_raw_parts_call() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.view_mut

func main(): void {
    return
}
"#,
        "view_mut",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
            ),
            (
                "std/ptr_slice.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

pub func view_mut(address: usize, len: usize): &+[u8] {
    return slice_from_raw_parts_mut(from_addr(address), len)
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Return,
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_inferred_str_from_raw_parts_local_binding() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_str.view

func main(): void {
    return
}
"#,
        "view",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive str_from_raw_parts(pointer: *u8, len: usize): &str
"#,
            ),
            (
                "std/ptr_str.nct",
                r#"use std/ptr.from_addr
use std/ptr.str_from_raw_parts

pub func view(address: usize, len: usize): &str {
    let text = str_from_raw_parts(from_addr(address), len)
    return text
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetStrRawParts {
                destination: StrLocation::Local(0),
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::SetStr {
                destination: StrLocation::Return,
                value: StrValue::Location(StrLocation::Local(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_inferred_slice_from_raw_parts_local_binding() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.view_mut

func main(): void {
    return
}
"#,
        "view_mut",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
            ),
            (
                "std/ptr_slice.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

pub func view_mut(address: usize, len: usize): &+[u8] {
    let view = slice_from_raw_parts_mut(from_addr(address), len)
    return view
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Local(0),
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::SetSlice {
                destination: SliceLocation::Return,
                value: SliceValue::Location(SliceLocation::Local(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_slice_from_raw_parts_value_call() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.view

func main(): void {
    return
}
"#,
        "view",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_value<T>(pointer: *T, len: usize): &[T]
"#,
            ),
            (
                "std/ptr_slice.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_value

pub func view(address: usize, len: usize): &[u8] {
    return slice_from_raw_parts_value(from_addr(address), len)
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Return,
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_inferred_slice_from_raw_parts_value_local_binding() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.view

func main(): void {
    return
}
"#,
        "view",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_value<T>(pointer: *T, len: usize): &[T]
"#,
            ),
            (
                "std/ptr_slice.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_value

pub func view(address: usize, len: usize): &[u8] {
    let slice = slice_from_raw_parts_value(from_addr(address), len)
    return slice
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Local(0),
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::SetSlice {
                destination: SliceLocation::Return,
                value: SliceValue::Location(SliceLocation::Local(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_slice_from_raw_parts_value_mut_call() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.view_mut

func main(): void {
    return
}
"#,
        "view_mut",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_value_mut<T>(pointer: *T, len: usize): &+[T]
"#,
            ),
            (
                "std/ptr_slice.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_value_mut

pub func view_mut(address: usize, len: usize): &+[u8] {
    return slice_from_raw_parts_value_mut(from_addr(address), len)
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Return,
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_str_from_raw_parts_call_len_return() {
    let size = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_str.size

func main(): void {
    return
}
"#,
        "size",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive str_from_raw_parts(pointer: *u8, len: usize): &str
"#,
            ),
            (
                "std/ptr_str.nct",
                r#"use std/ptr.from_addr
use std/ptr.str_from_raw_parts

pub func size(address: usize, len: usize): usize {
    return str_from_raw_parts(from_addr(address), len).len()
}
"#,
            ),
        ],
    );

    assert_eq!(
        size.instructions,
        vec![
            Instruction::SetStrRawParts {
                destination: StrLocation::Local(0),
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::SetUsize {
                destination: UsizeLocation::Return,
                value: UsizeValue::StrLen(StrLocation::Local(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_slice_from_raw_parts_call_index_return() {
    let first = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.first

func main(): void {
    return
}
"#,
        "first",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
            ),
            (
                "std/ptr_slice.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

pub func first(address: usize, len: usize): u8 {
    return slice_from_raw_parts_mut(from_addr(address), len)[0]
}
"#,
            ),
        ],
    );

    assert_eq!(
        first.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Local(0),
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::SetU8 {
                destination: U8Location::Return,
                value: U8Value::SliceIndex {
                    source: SliceLocation::Local(0),
                    index: usize_const(0),
                },
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_string_bytes_to_slice_view() {
    let bytes = lower_imported_named_function_with_nocter_home_files(
        r#"use std/string.bytes

func main(): void {
    return
}
"#,
        "bytes",
        &[std_string_bytes_file()],
    );

    assert_eq!(
        bytes.instructions,
        vec![
            Instruction::SetSlice {
                destination: SliceLocation::Return,
                value: SliceValue::StrBytes(StrValue::Location(StrLocation::Parameter(0))),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_bytes_from_str_call_len_return() {
    let size = lower_imported_named_function_with_nocter_home_files(
        r#"use std/string.size

func main(): void {
    return
}
"#,
        "size",
        &[(
            "std/string.nct",
            r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func size(value: &str): usize {
    return bytes_from_str(value).len()
}
"#,
        )],
    );

    assert_eq!(
        size.instructions,
        vec![
            Instruction::SetUsize {
                destination: UsizeLocation::Return,
                value: UsizeValue::StrLen(StrLocation::Parameter(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_bytes_from_str_call_index_return() {
    let first = lower_imported_named_function_with_nocter_home_files(
        r#"use std/string.first

func main(): void {
    return
}
"#,
        "first",
        &[(
            "std/string.nct",
            r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func first(value: &str): u8 {
    return bytes_from_str(value)[1]
}
"#,
        )],
    );

    assert_eq!(
        first.instructions,
        vec![
            Instruction::SetU8 {
                destination: U8Location::Return,
                value: U8Value::StrIndex {
                    source: StrLocation::Parameter(0),
                    index: usize_const(1),
                },
            },
            Instruction::Return,
        ]
    );
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
            instructions: vec![Instruction::If {
                condition: BoolValue::Location(BoolLocation::Parameter(0)),
                then_instructions: vec![
                    Instruction::SetU8 {
                        destination: U8Location::Return,
                        value: u8_const(7),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![
                    Instruction::SetU8 {
                        destination: U8Location::Return,
                        value: u8_const(9),
                    },
                    Instruction::Return,
                ],
            }],
        }
    );
}

#[test]
fn lowers_str_returning_function_with_terminal_if() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func choose(flag: bool): &str {
    if flag {
        return "yes"
    } else {
        return "no"
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
            return_type: Type::Str,
            instructions: vec![Instruction::If {
                condition: BoolValue::Location(BoolLocation::Parameter(0)),
                then_instructions: vec![
                    Instruction::SetStr {
                        destination: StrLocation::Return,
                        value: str_static_value(b"yes"),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![
                    Instruction::SetStr {
                        destination: StrLocation::Return,
                        value: str_static_value(b"no"),
                    },
                    Instruction::Return,
                ],
            }],
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

#[test]
fn lowers_slice_returning_function_with_terminal_if() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func choose(left: &[u8], right: &[u8], flag: bool): &[u8] {
    if flag {
        return left
    } else {
        return right
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
            return_type: readonly_u8_slice_type(),
            instructions: vec![Instruction::If {
                condition: BoolValue::Location(BoolLocation::Parameter(4)),
                then_instructions: vec![
                    Instruction::SetSlice {
                        destination: SliceLocation::Return,
                        value: SliceValue::Location(SliceLocation::Parameter(0)),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![
                    Instruction::SetSlice {
                        destination: SliceLocation::Return,
                        value: SliceValue::Location(SliceLocation::Parameter(2)),
                    },
                    Instruction::Return,
                ],
            }],
        }
    );
}
