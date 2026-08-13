use super::*;

#[test]
fn lowers_indirect_aggregate_local_value_argument() {
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

func consume(text: Text): usize {
    return text.len
}

func caller(): usize {
    let text = Text { start: 1, len: 2, capacity: 3 }
    let result: usize = consume(move text)
    return result
}
"#,
        "caller",
        function_signatures(vec![("consume", Type::Usize, vec![aggregate_type])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "caller".to_string(),
            target: CallTarget::same_file("caller"),
            return_type: Type::Usize,
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
                Instruction::CallUsize {
                    destination: UsizeLocation::Local(1),
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateIndirect(AggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                    })],
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: usize_local(1),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_indirect_aggregate_call_return() {
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

func make(): Text {
    return Text { start: 1, len: 2, capacity: 3 }
}

func forward(): Text {
    return make()
}
"#,
        "forward",
        function_signatures(vec![("make", aggregate_type.clone(), vec![])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "forward".to_string(),
            target: CallTarget::same_file("forward"),
            return_type: aggregate_type,
            instructions: vec![
                Instruction::CallAggregate {
                    destination: AggregateLocation::Return,
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_indirect_aggregate_call_binding_return() {
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

func make(): Text {
    return Text { start: 1, len: 2, capacity: 3 }
}

func forward(): Text {
    let value = make()
    return move value
}
"#,
        "forward",
        function_signatures(vec![("make", aggregate_type.clone(), vec![])]),
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
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
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
fn lowers_aggregate_call_binding_with_aggregate_argument_without_slot_conflict() {
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

func main(): i32 {
    return 0
}

func wrap(header: Header): Packet {
    return Packet { prefix: 1, header: header, tail: 2 }
}

func build(): i32 {
    let packet = wrap(Header { tag: 7, ok: true, code: 42, len: 11 })
    return packet.header.code
}
"#,
        "build",
        function_signatures(vec![("wrap", packet_type, vec![header_type])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "build".to_string(),
            target: CallTarget::same_file("build"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(32, 8),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(1),
                    offset: 0,
                    value: u8_const(7),
                },
                Instruction::StoreAggregateBool {
                    destination: AggregateLocation::Slot(1),
                    offset: 1,
                    value: BoolValue::Const(true),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(1),
                    offset: 4,
                    value: i32_const(42),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(1),
                    offset: 8,
                    value: usize_const(11),
                },
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("wrap"),
                    arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                        source: AggregateArgumentSource::Slot(1),
                        layout: ValueLayout::new(16, 8),
                        words: 2,
                    })],
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
fn lowers_readwrite_indirect_aggregate_call_binding_borrow_argument() {
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

func make(): Text {
    return Text { start: 1, len: 2, capacity: 3 }
}

func touch(value: &+Text): void {
    return
}

func forward(): Text {
    var value = make()
    touch(&+value)
    return move value
}
"#,
        "forward",
        function_signatures(vec![
            ("make", aggregate_type.clone(), vec![]),
            (
                "touch",
                Type::Void,
                vec![Type::Borrow {
                    is_readwrite: true,
                    inner: Box::new(aggregate_type.clone()),
                }],
            ),
        ]),
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
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
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
fn lowers_direct_aggregate_call_binding_borrow_argument() {
    let aggregate_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
    let function = lower_named_function_with_signatures(
        r#"struct Allocator {
    state: usize
    kind: u64
}

func main(): i32 {
    return 0
}

func page_allocator(): Allocator {
    return Allocator { state: 0, kind: 0 }
}

func touch(allocator: &+Allocator): void {
    return
}

func use_allocator(): i32 {
    var allocator = page_allocator()
    touch(&+allocator)
    return 0
}
"#,
        "use_allocator",
        function_signatures(vec![
            ("page_allocator", aggregate_type.clone(), vec![]),
            (
                "touch",
                Type::Void,
                vec![Type::Borrow {
                    is_readwrite: true,
                    inner: Box::new(aggregate_type.clone()),
                }],
            ),
        ]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "use_allocator".to_string(),
            target: CallTarget::same_file("use_allocator"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("page_allocator"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 8),
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
fn lowers_return_call_with_aggregate_borrow_argument_as_normal_call() {
    let packet_type = Type::Aggregate {
        layout: ValueLayout::new(32, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    return 0
}

func caller(): i32 {
    let packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: true, code: 42, len: 11 },
        tail: 99,
    }
    return read_code(&packet)
}

func read_code(packet: &Packet): i32 {
    return packet.header.code
}
"#,
        "caller",
        function_signatures(vec![(
            "read_code",
            Type::I32,
            vec![Type::Borrow {
                is_readwrite: false,
                inner: Box::new(packet_type),
            }],
        )]),
    )
    .unwrap();

    assert!(
        function.instructions.contains(&Instruction::CallI32 {
            destination: I32Location::Return,
            target: CallTarget::same_file("read_code"),
            arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                source: BorrowSource::AggregateSlot(0),
            })],
        }),
        "{function:?}"
    );
    assert_eq!(function.instructions.last(), Some(&Instruction::Return));
    assert!(
        !function
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::TailCall { .. })),
        "{function:?}"
    );
}
