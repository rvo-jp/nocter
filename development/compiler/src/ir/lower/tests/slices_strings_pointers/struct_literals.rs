use super::*;

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
