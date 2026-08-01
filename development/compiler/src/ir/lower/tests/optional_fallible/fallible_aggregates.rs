use super::*;

#[test]
fn lowers_propagated_indirect_aggregate_call_value_argument() {
    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func make(): Text! {
    return Text { start: 1, len: 2, capacity: 3 }
}

func consume(text: Text): i32 {
    return 42
}

func main(): i32! {
    return consume(make()?)
}
"#,
        "main",
        function_signatures(vec![
            (
                "make",
                Type::Fallible(Box::new(aggregate_type.clone())),
                vec![],
            ),
            ("consume", Type::I32, vec![aggregate_type]),
        ]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CallFallibleAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::CallI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateIndirect(AggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                    })],
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_propagated_direct_aggregate_call_value_argument() {
    let aggregate_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
    let function = lower_named_function_with_signatures(
        r#"struct Allocator {
    state: usize
    kind: usize
}

func make(): Allocator! {
    return Allocator { state: 1, kind: 2 }
}

func consume(allocator: Allocator): i32 {
    return 42
}

func main(): i32! {
    return consume(make()?)
}
"#,
        "main",
        function_signatures(vec![
            (
                "make",
                Type::Fallible(Box::new(aggregate_type.clone())),
                vec![],
            ),
            ("consume", Type::I32, vec![aggregate_type]),
        ]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallFallibleDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 8),
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::CallI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                        layout: ValueLayout::new(16, 8),
                        words: 2,
                    })],
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_pending_aggregate_drop_for_fallible_propagation_cleanup() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): void! {
    var file = File { fd: 3 }
    fail()?
}

func fail(): void! {
    return Error.new("app.fail", "failed")
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
                value: i32_const(3),
            },
            Instruction::CallFallibleVoid {
                target: CallTarget::same_file("fail"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::PropagateWithCleanup {
                    code: StrLocation::Local(0),
                    message: StrLocation::Local(2),
                    instructions: vec![drop_call.clone()],
                },
            },
            drop_call,
            Instruction::ReturnFallibleSuccess,
        ],
    );
}

#[test]
fn lowers_replacement_drop_for_fallible_aggregate_assignment() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): void! {
    var file = File { fd: 1 }
    file = make()?
    return
}

func make(): File! {
    return File { fd: 2 }
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
            Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(4, 4),
                failure_mode: FallibleFailureMode::PropagateWithCleanup {
                    code: StrLocation::Local(0),
                    message: StrLocation::Local(2),
                    instructions: vec![drop_call.clone()],
                },
            },
            drop_call.clone(),
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::Slot(1),
                layout: ValueLayout::new(4, 4),
            },
            drop_call,
            Instruction::ReturnFallibleSuccess,
        ],
    );
}

#[test]
fn lowers_propagated_indirect_aggregate_call_binding_return() {
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

func make(): Text! {
    return Text { start: 1, len: 2, capacity: 3 }
}

func forward(): Text! {
    var value = make()?
    return move value
}
"#,
        "forward",
        function_signatures(vec![(
            "make",
            Type::Fallible(Box::new(aggregate_type.clone())),
            vec![],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "forward".to_string(),
            target: CallTarget::same_file("forward"),
            return_type: Type::Fallible(Box::new(aggregate_type)),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CallFallibleAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Return,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_propagated_indirect_aggregate_call_return() {
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

func make(): Text! {
    return Text { start: 1, len: 2, capacity: 3 }
}

func forward(): Text! {
    return make()?
}
"#,
        "forward",
        function_signatures(vec![(
            "make",
            Type::Fallible(Box::new(aggregate_type.clone())),
            vec![],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "forward".to_string(),
            target: CallTarget::same_file("forward"),
            return_type: Type::Fallible(Box::new(aggregate_type)),
            instructions: vec![
                Instruction::CallFallibleAggregate {
                    destination: AggregateLocation::Return,
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_direct_aggregate_fallible_struct_literal_return_after_scope_drop() {
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

func choose(): Pair! {
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
            Instruction::ReturnFallibleSuccess,
        ],
    );
}

#[test]
fn lowers_propagated_direct_aggregate_call_return() {
    let aggregate_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
    let function = lower_named_function_with_signatures(
        r#"struct Pair {
    first: usize
    second: usize
}

func main(): i32 {
    return 0
}

func make(): Pair! {
    return Pair { first: 1, second: 2 }
}

func forward(): Pair! {
    return make()?
}
"#,
        "forward",
        function_signatures(vec![(
            "make",
            Type::Fallible(Box::new(aggregate_type.clone())),
            vec![],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "forward".to_string(),
            target: CallTarget::same_file("forward"),
            return_type: Type::Fallible(Box::new(aggregate_type)),
            instructions: vec![
                Instruction::CallFallibleDirectAggregate {
                    destination: AggregateLocation::DirectReturn,
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 8),
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_fallible_direct_aggregate_call_binding_borrow_argument() {
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

func page_allocator(): Allocator! {
    return Allocator { state: 0, kind: 0 }
}

func touch(allocator: &+Allocator): void {
    return
}

func use_allocator(): i32! {
    var allocator = page_allocator()?
    touch(&+allocator)
    return 0
}
"#,
        "use_allocator",
        function_signatures(vec![
            (
                "page_allocator",
                Type::Fallible(Box::new(aggregate_type.clone())),
                vec![],
            ),
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
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallFallibleDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("page_allocator"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 8),
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("touch"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlot(0),
                    })],
                },
                set_return_i32(0),
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_fallible_direct_aggregate_call_assignment_borrow_argument() {
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

func reset_allocator(): Allocator! {
    return Allocator { state: 1, kind: 2 }
}

func touch(allocator: &+Allocator): void {
    return
}

func use_allocator(): i32! {
    var allocator = page_allocator()
    allocator = reset_allocator()?
    touch(&+allocator)
    return 0
}
"#,
        "use_allocator",
        function_signatures(vec![
            ("page_allocator", aggregate_type.clone(), vec![]),
            (
                "reset_allocator",
                Type::Fallible(Box::new(aggregate_type.clone())),
                vec![],
            ),
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
            return_type: Type::Fallible(Box::new(Type::I32)),
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
                Instruction::CallFallibleDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("reset_allocator"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 8),
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("touch"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlot(0),
                    })],
                },
                set_return_i32(0),
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_propagated_indirect_aggregate_call_assignment_borrow_argument() {
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

func make(): Text! {
    return Text { start: 4, len: 5, capacity: 6 }
}

func touch(value: &+Text): void {
    return
}

func use_text(): i32! {
    var value = Text { start: 1, len: 2, capacity: 3 }
    value = make()?
    touch(&+value)
    return 0
}
"#,
        "use_text",
        function_signatures(vec![
            (
                "make",
                Type::Fallible(Box::new(aggregate_type.clone())),
                vec![],
            ),
            (
                "touch",
                Type::Void,
                vec![Type::Borrow {
                    is_readwrite: true,
                    inner: Box::new(aggregate_type),
                }],
            ),
        ]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "use_text".to_string(),
            target: CallTarget::same_file("use_text"),
            return_type: Type::Fallible(Box::new(Type::I32)),
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
                Instruction::CallFallibleAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("touch"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlot(0),
                    })],
                },
                set_return_i32(0),
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_copy_aggregate_field_binding_from_non_copy_fallible_call_result() {
    let packet_type = Type::Fallible(Box::new(Type::DirectAggregate {
        layout: ValueLayout::new(16, 4),
        words: 2,
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    code: i32
    len: i32
}

struct Packet {
    prefix: i32
    header: Header
    tail: i32
}

func make_packet(): Packet! {
    return Packet { prefix: 1, header: Header { code: 40, len: 2 }, tail: 3 }
}

func main(): i32 {
    return 0
}

func read_code(): i32! {
    let header = make_packet()?.header
    let again = header
    return again.code + again.len
}
"#,
        "read_code",
        function_signatures(vec![("make_packet", packet_type, vec![])]),
    )
    .unwrap();

    assert_eq!(function.return_type, Type::Fallible(Box::new(Type::I32)));
    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make_packet"),
                arguments: vec![],
                layout: ValueLayout::new(16, 4),
                failure_mode: FallibleFailureMode::Propagate,
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 0,
                source: AggregateLocation::Slot(1),
                source_offset: 4,
                layout: ValueLayout::new(8, 4),
            }),
        "{function:?}"
    );
    assert!(
        function.instructions.contains(&Instruction::CopyAggregate {
            destination: AggregateLocation::Slot(2),
            source: AggregateLocation::Slot(0),
            layout: ValueLayout::new(8, 4),
        }),
        "{function:?}"
    );
}

#[test]
fn lowers_nested_aggregate_struct_literal_field_from_fallible_call_result_member() {
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

func make(): Packet! {
    return Packet { prefix: 1, header: Header { tag: 8, ok: true, code: 42, len: 12 }, tail: 2 }
}

func build(): i32! {
    let packet = Packet { prefix: 1, header: make()?.header, tail: 2 }
    return packet.header.code
}
"#,
        "build",
        function_signatures(vec![(
            "make",
            Type::Fallible(Box::new(packet_type)),
            vec![],
        )]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Propagate,
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
fn lowers_nested_aggregate_struct_literal_field_from_fallible_call() {
    let header_type = Type::Fallible(Box::new(Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    }));
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

func make_header(): Header! {
    return Header { tag: 8, ok: true, code: 42, len: 12 }
}

func build(): i32! {
    let packet = Packet { prefix: 1, header: make_header()?, tail: 2 }
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
            .contains(&Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make_header"),
                arguments: vec![],
                layout: ValueLayout::new(16, 8),
                failure_mode: FallibleFailureMode::Propagate,
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
}

#[test]
fn lowers_nested_aggregate_field_binding_from_fallible_call_result() {
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

func make(): Packet! {
    return Packet { prefix: 1, header: Header { tag: 7, ok: true, code: 42, len: 11 }, tail: 2 }
}

func read_code(): i32! {
    let header = make()?.header
    return header.code
}
"#,
        "read_code",
        function_signatures(vec![(
            "make",
            Type::Fallible(Box::new(packet_type)),
            vec![],
        )]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Propagate,
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 0,
                source: AggregateLocation::Slot(1),
                source_offset: 8,
                layout: ValueLayout::new(16, 8),
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_nested_aggregate_field_value_argument_from_fallible_call_result() {
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

func make(): Packet! {
    return Packet { prefix: 1, header: Header { tag: 7, ok: true, code: 42, len: 11 }, tail: 2 }
}

func consume(header: Header): i32 {
    return header.code
}

func main(): i32! {
    return consume(make()?.header)
}
"#,
        "main",
        function_signatures(vec![
            ("make", Type::Fallible(Box::new(packet_type)), vec![]),
            ("consume", Type::I32, vec![header_type]),
        ]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Propagate,
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 0,
                source: AggregateLocation::Slot(0),
                source_offset: 8,
                layout: ValueLayout::new(16, 8),
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_nested_aggregate_field_return_from_fallible_call_result() {
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

func make(): Packet! {
    return Packet { prefix: 1, header: Header { tag: 7, ok: true, code: 42, len: 11 }, tail: 2 }
}

func pick(): Header! {
    return make()?.header
}
"#,
        "pick",
        function_signatures(vec![(
            "make",
            Type::Fallible(Box::new(packet_type)),
            vec![],
        )]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Propagate,
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::DirectReturn,
                destination_offset: 0,
                source: AggregateLocation::Slot(0),
                source_offset: 8,
                layout: ValueLayout::new(16, 8),
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_nested_aggregate_field_assignment_from_fallible_call_result() {
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

func make(): Packet! {
    return Packet { prefix: 1, header: Header { tag: 8, ok: true, code: 42, len: 12 }, tail: 2 }
}

func update(): i32! {
    var packet = Packet { prefix: 1, header: Header { tag: 7, ok: false, code: 1, len: 11 }, tail: 2 }
    packet.header = make()?.header
    return packet.header.code
}
"#,
        "update",
        function_signatures(vec![(
            "make",
            Type::Fallible(Box::new(packet_type)),
            vec![],
        )]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Propagate,
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
fn lowers_readwrite_usize_fallible_call_result_slice_index_compound_assignment() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func update(values: &+[usize], indices: &[usize]): void! {
    maybe_values(values)?[indices[0]] %= value()
    return
}

func maybe_values(values: &+[usize]): &+[usize]! {
    return values
}

func value(): usize {
    return 5
}
"#,
        "update",
        function_signatures(vec![
            (
                "maybe_values",
                Type::Fallible(Box::new(Type::Slice { is_readwrite: true })),
                vec![Type::Slice { is_readwrite: true }],
            ),
            ("value", Type::Usize, vec![]),
        ]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "update".to_string(),
            target: crate::ir::CallTarget::same_file("update".to_string()),
            return_type: Type::Fallible(Box::new(Type::Void)),
            instructions: vec![
                Instruction::CallFallibleSlice {
                    destination: SliceLocation::Local(0),
                    target: CallTarget::same_file("maybe_values"),
                    arguments: vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(2),
                    value: usize_slice_index(SliceLocation::Parameter(2), usize_const(0)),
                },
                call_usize(UsizeLocation::Local(3), "value", vec![]),
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(4),
                    value: usize_slice_index(SliceLocation::Local(0), usize_local(2)),
                },
                Instruction::RemainderUsize {
                    destination: UsizeLocation::Local(4),
                    left: usize_local(4),
                    right: usize_local(3),
                },
                Instruction::StoreUsizeToSliceIndex {
                    destination: SliceLocation::Local(0),
                    index: usize_local(2),
                    value: usize_local(4),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_ignored_fallible_direct_aggregate_call_expression_statement_with_drop() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32! {
    make()?
    return 0
}

func make(): File! {
    return File { fd: 1 }
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(4, 4),
                failure_mode: FallibleFailureMode::Propagate,
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("File.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
                })],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::ReturnFallibleSuccess,
        ]
    );
}

#[test]
fn lowers_fallible_aggregate_force_unwrap_call_binding_as_trapping_fallible_call() {
    let aggregate_type = Type::Fallible(Box::new(Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = make()!
    return header.code
}

func make(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(16, 8),
                failure_mode: FallibleFailureMode::Trap,
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_fallible_aggregate_force_unwrap_member_binding_as_trapping_fallible_call() {
    let packet_type = Type::Fallible(Box::new(Type::Aggregate {
        layout: ValueLayout::new(32, 8),
    }));
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
    let header = make()!.header
    return header.code
}

func make(): Packet! {
    return Packet { prefix: 1, header: Header { tag: 7, ok: true, code: 42, len: 11 }, tail: 2 }
}
"#,
        "main",
        function_signatures(vec![("make", packet_type, vec![])]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Trap,
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 0,
                source: AggregateLocation::Slot(1),
                source_offset: 8,
                layout: ValueLayout::new(16, 8),
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_fallible_aggregate_force_unwrap_value_argument_as_trapping_fallible_call() {
    let aggregate_type = Type::DirectAggregate {
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

func make(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}

func consume(header: Header): i32 {
    return header.code
}

func main(): i32 {
    return consume(make()!)
}
"#,
        "main",
        function_signatures(vec![
            (
                "make",
                Type::Fallible(Box::new(aggregate_type.clone())),
                vec![],
            ),
            ("consume", Type::I32, vec![aggregate_type]),
        ]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(16, 8),
                failure_mode: FallibleFailureMode::Trap,
            }),
        "{function:?}"
    );
    assert!(
        function.instructions.contains(&Instruction::TailCall {
            target: CallTarget::same_file("consume"),
            arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                source: AggregateArgumentSource::Slot(0),
                layout: ValueLayout::new(16, 8),
                words: 2,
            })],
        }),
        "{function:?}"
    );
}

#[test]
fn lowers_fallible_aggregate_force_unwrap_assignment_as_trapping_fallible_call() {
    let aggregate_type = Type::Fallible(Box::new(Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    var header = Header { tag: 1, ok: false, code: 1, len: 1 }
    header = make()!
    return header.code
}

func make(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(16, 8),
                failure_mode: FallibleFailureMode::Trap,
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_fallible_aggregate_force_unwrap_struct_literal_field_as_trapping_fallible_call() {
    let aggregate_type = Type::Fallible(Box::new(Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    }));
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
    let packet = Packet { prefix: 1, header: make()!, tail: 2 }
    return packet.header.code
}

func make(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(16, 8),
                failure_mode: FallibleFailureMode::Trap,
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
}

#[test]
fn lowers_fallible_aggregate_force_unwrap_call_return_as_trapping_fallible_call() {
    let aggregate_type = Type::DirectAggregate {
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

func main(): i32 {
    return 0
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}

func make(): Header {
    return source()!
}
"#,
        "make",
        function_signatures(vec![(
            "source",
            Type::Fallible(Box::new(aggregate_type.clone())),
            vec![],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "make".to_string(),
            target: CallTarget::same_file("make"),
            return_type: aggregate_type,
            instructions: vec![
                Instruction::CallFallibleDirectAggregate {
                    destination: AggregateLocation::DirectReturn,
                    target: CallTarget::same_file("source"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 8),
                    failure_mode: FallibleFailureMode::Trap,
                },
                Instruction::Return,
            ],
        }
    );
}
