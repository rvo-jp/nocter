use super::*;

#[test]
fn lowers_generic_function_call_inferred_from_catch_block_return_type() {
    let ir = lower_text(
        r#"struct Marker<T> {
    code: i32
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}

func source(): Marker<u8>! {
    return Marker<u8> { code: 1 }
}

func recover(): Marker<u8> {
    return source() catch error {
        return make()
    }
}

func main(): i32 {
    return recover().code
}
"#,
    );

    let specialized_target = CallTarget::same_file("make<u8>");
    let recover = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("recover"))
        .expect("expected lowered recover function");

    let catch_instructions = recover.instructions.iter().find_map(|instruction| {
        if let Instruction::CallFallibleDirectAggregate {
            failure_mode: FallibleFailureMode::Catch { instructions, .. },
            ..
        } = instruction
        {
            Some(instructions)
        } else {
            None
        }
    });
    let catch_instructions = catch_instructions.expect("expected aggregate catch call");
    assert!(
        catch_instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallDirectAggregate { target, .. } if target == &specialized_target
            )
        }),
        "{recover:?}"
    );
    assert!(
        ir.functions
            .iter()
            .any(|function| function.target == specialized_target),
        "{ir:?}"
    );
}

#[test]
fn indexes_fallible_function_signature_parameter_abi_word_count() {
    let analysis = analyze_text(
        r#"func main(): i32 {
    return 0
}

func load(text: &str, count: usize): i32! {
    return 1
}
"#,
    );
    let root = analysis.root_file().unwrap();
    let index = FunctionIndex::new(&analysis, root.ast.span.source);
    let signatures = index.signatures();

    assert_eq!(
        signatures.return_type(&CallTarget::same_file("load")),
        Some(&Type::Fallible(Box::new(Type::I32)))
    );
    assert_eq!(
        signatures.parameter_abi_word_count(&CallTarget::same_file("load")),
        Some(3)
    );
}

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
fn lowers_fallible_void_terminal_if_entry() {
    let ir = lower_text(
        r#"func main(): void! {
    if true {
        return
    } else {
        return
    }
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![Instruction::If {
            condition: BoolValue::Const(true),
            then_instructions: vec![Instruction::ReturnFallibleSuccess],
            else_instructions: vec![Instruction::ReturnFallibleSuccess],
        }],
    );
}

#[test]
fn lowers_fallible_void_nested_terminal_if_entry() {
    let ir = lower_text(
        r#"func main(): void! {
    if true {
        if false {
            return
        } else {
            return
        }
    } else {
        return
    }
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![Instruction::If {
            condition: BoolValue::Const(true),
            then_instructions: vec![Instruction::If {
                condition: BoolValue::Const(false),
                then_instructions: vec![Instruction::ReturnFallibleSuccess],
                else_instructions: vec![Instruction::ReturnFallibleSuccess],
            }],
            else_instructions: vec![Instruction::ReturnFallibleSuccess],
        }],
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
fn lowers_fallible_aggregate_catch_field_read_in_comparison() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    return run()?
}

func run(): i32! {
    if (source() catch error {
        return Error.new("app.source", error.message)
    }).code == 42 {
        return 42
    } else {
        return 1
    }
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let run = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(run, AggregateLocation::Slot(0), "source");
    assert!(run.instructions.contains(&Instruction::LoadAggregateI32 {
        destination: I32Location::Local(0),
        source: AggregateLocation::Slot(0),
        offset: 4,
    }));
    assert!(run.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::If {
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Equal,
                    left,
                    right,
                },
                ..
            } if left == &i32_local(0) && right == &i32_const(42)
        )
    }));
}

#[test]
fn lowers_fixed_array_optional_otherwise_binding_and_return() {
    let source = r#"func main(): i32 {
    let fallback: [i32; 2] = [1, 2]
    let values: [i32; 2] = maybe_pair() otherwise { fallback }
    return values[0] + values[1]
}

func choose(): [i32; 2] {
    return maybe_pair() otherwise { [20, 22] }
}

func maybe_pair(): [i32; 2]? {
    return none
}
"#;
    let layout = ValueLayout::new(8, 4);
    let pair_type = Type::DirectAggregate { layout, words: 1 };

    let main = lower_named_function_with_signatures(
        source,
        "main",
        function_signatures(vec![(
            "maybe_pair",
            Type::Fallible(Box::new(pair_type.clone())),
            vec![],
        )]),
    )
    .unwrap();

    let Some(Instruction::CallFallibleDirectAggregate {
        destination,
        target,
        arguments,
        layout: call_layout,
        failure_mode: FallibleFailureMode::Recover { instructions },
    }) = main
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallFallibleDirectAggregate { .. }))
    else {
        panic!("{main:?}");
    };
    assert_eq!(*destination, AggregateLocation::Slot(1));
    assert_eq!(*target, CallTarget::same_file("maybe_pair"));
    assert!(arguments.is_empty());
    assert_eq!(*call_layout, layout);
    assert!(instructions.contains(&Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(1),
        destination_offset: 0,
        source: AggregateLocation::Slot(0),
        source_offset: 0,
        layout,
    }));

    let choose = lower_named_function_with_signatures(
        source,
        "choose",
        function_signatures(vec![(
            "maybe_pair",
            Type::Fallible(Box::new(pair_type)),
            vec![],
        )]),
    )
    .unwrap();

    assert!(
        choose.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::DirectReturn,
                target,
                arguments,
                layout: call_layout,
                failure_mode: FallibleFailureMode::Handle { instructions },
            } if *target == CallTarget::same_file("maybe_pair")
                && arguments.is_empty()
                && *call_layout == layout
                && instructions.contains(&Instruction::Return)
        )),
        "{choose:?}"
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
fn lowers_ignored_fallible_i32_call_expression_statement() {
    let ir = lower_text(
        r#"func main(): i32! {
    value()?
    return 0
}

func value(): i32! {
    return 1
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("value"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_const(0),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_ignored_fallible_str_force_expression_statement() {
    let ir = lower_text(
        r#"func main(): i32 {
    text()!
    return 0
}

func text(): &str! {
    return "ignored"
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallFallibleStr {
                    destination: StrLocation::Local(0),
                    target: CallTarget::same_file("text"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Trap,
                },
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
fn lowers_ignored_fallible_slice_call_expression_statement() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func wrapper(bytes: &[u8]): i32! {
    maybe_bytes(bytes)?
    return 0
}

func maybe_bytes(bytes: &[u8]): &[u8]! {
    return bytes
}
"#,
        "wrapper",
        function_signatures(vec![(
            "maybe_bytes",
            Type::Fallible(Box::new(readonly_u8_slice_type())),
            vec![readonly_u8_slice_type()],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::CallFallibleSlice {
                    destination: SliceLocation::Local(0),
                    target: CallTarget::same_file("maybe_bytes"),
                    arguments: vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_const(0),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_ignored_fallible_str_catch_statement_with_reserved_error_locals() {
    let ir = lower_text(
        r#"func main(): i32 {
    text() catch error {
        return 7
    }
    return 0
}

func text(): &str! {
    return "ignored"
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallFallibleStr {
                    destination: StrLocation::Local(0),
                    target: CallTarget::same_file("text"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Catch {
                        code: StrLocation::Local(2),
                        message: StrLocation::Local(4),
                        instructions: vec![
                            Instruction::SetI32 {
                                destination: I32Location::Return,
                                value: i32_const(7),
                            },
                            Instruction::Return,
                        ],
                    },
                },
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
fn lowers_fallible_entry_returning_i32_literal() {
    let ir = lower_text(
        r#"func main(): i32! {
    return 7
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![set_return_i32(7), Instruction::ReturnFallibleSuccess],
        }])
    );
}

#[test]
fn lowers_fallible_entry_alias_return_type() {
    let ir = lower_text(
        r#"type ExitResult = i32!

func main(): ExitResult {
    return 7
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![set_return_i32(7), Instruction::ReturnFallibleSuccess],
        }])
    );
}

#[test]
fn lowers_fallible_void_function_success_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func run(): void! {
    return
}
"#,
        "run",
    );

    assert_eq!(
        function,
        Function {
            name: "run".to_string(),
            target: crate::ir::CallTarget::same_file("run".to_string()),
            return_type: Type::Fallible(Box::new(Type::Void)),
            instructions: vec![Instruction::ReturnFallibleSuccess],
        }
    );
}

#[test]
fn lowers_fallible_i32_function_success_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func answer(): i32! {
    return 42
}
"#,
        "answer",
    );

    assert_eq!(
        function,
        Function {
            name: "answer".to_string(),
            target: crate::ir::CallTarget::same_file("answer".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![set_return_i32(42), Instruction::ReturnFallibleSuccess],
        }
    );
}

#[test]
fn lowers_optional_i32_function_none_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func maybe_answer(): i32? {
    return none
}
"#,
        "maybe_answer",
    );

    assert_eq!(
        function,
        Function {
            name: "maybe_answer".to_string(),
            target: crate::ir::CallTarget::same_file("maybe_answer".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![Instruction::ReturnOptionalNone],
        }
    );
}

#[test]
fn lowers_optional_alias_i32_function_none_return() {
    let function = lower_named_function(
        r#"type MaybeI32 = i32?

func main(): i32 {
    return 0
}

func maybe_answer(): MaybeI32 {
    return none
}
"#,
        "maybe_answer",
    );

    assert_eq!(
        function,
        Function {
            name: "maybe_answer".to_string(),
            target: crate::ir::CallTarget::same_file("maybe_answer".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![Instruction::ReturnOptionalNone],
        }
    );
}

#[test]
fn lowers_optional_i32_function_success_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func maybe_answer(): i32? {
    return 42
}
"#,
        "maybe_answer",
    );

    assert_eq!(
        function,
        Function {
            name: "maybe_answer".to_string(),
            target: crate::ir::CallTarget::same_file("maybe_answer".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![set_return_i32(42), Instruction::ReturnFallibleSuccess],
        }
    );
}

#[test]
fn lowers_optional_i32_terminal_if_none_branch() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func maybe_answer(flag: bool): i32? {
    if flag {
        return 42
    } else {
        return none
    }
}
"#,
        "maybe_answer",
    );

    assert_eq!(
        function,
        Function {
            name: "maybe_answer".to_string(),
            target: crate::ir::CallTarget::same_file("maybe_answer".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![Instruction::If {
                condition: BoolValue::Location(BoolLocation::Parameter(0)),
                then_instructions: vec![set_return_i32(42), Instruction::ReturnFallibleSuccess],
                else_instructions: vec![Instruction::ReturnOptionalNone],
            }],
        }
    );
}

#[test]
fn lowers_optional_i32_return_propagation() {
    let ir = lower_text(
        r#"func main(): i32 {
    return value()!
}

func value(): i32? {
    return maybe_answer()?
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "value")
        .unwrap();

    assert_eq!(
        function,
        &Function {
            name: "value".to_string(),
            target: crate::ir::CallTarget::same_file("value".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::CallFallibleI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_optional_i32_otherwise_return_call_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { return 1 }

    return value
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Handle {
                        instructions: vec![set_return_i32(1), Instruction::Return],
                    },
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
fn lowers_optional_i32_otherwise_break_call_binding_inside_loop() {
    let ir = lower_text(
        r#"func main(): i32 {
    var total = 0
    loop {
        let value = maybe_answer(total) otherwise { break }
        total += value
    }
    return total
}

func maybe_answer(total: i32): i32? {
    return none
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
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::Const(true),
                body_instructions: vec![
                    Instruction::CallFallibleI32 {
                        destination: I32Location::Local(1),
                        target: CallTarget::same_file("maybe_answer"),
                        arguments: vec![ScalarArgument::I32(i32_local(0))],
                        failure_mode: FallibleFailureMode::Handle {
                            instructions: vec![Instruction::Break],
                        },
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(0),
                        right: i32_local(1),
                    },
                ],
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
fn lowers_optional_i32_otherwise_continue_call_binding_inside_range_for() {
    let ir = lower_text(
        r#"func main(): i32 {
    var total = 0
    for index in 0..<4 {
        let value = only_even(index) otherwise { continue }
        total += value
    }
    return total
}

func only_even(index: i32): i32? {
    return none
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
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            Instruction::SetI32 {
                destination: I32Location::Local(1),
                value: i32_const(0),
            },
            Instruction::SetI32 {
                destination: I32Location::Local(2),
                value: i32_const(4),
            },
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Less,
                    left: i32_local(1),
                    right: i32_local(2),
                },
                body_instructions: vec![
                    Instruction::CallFallibleI32 {
                        destination: I32Location::Local(3),
                        target: CallTarget::same_file("only_even"),
                        arguments: vec![ScalarArgument::I32(i32_local(1))],
                        failure_mode: FallibleFailureMode::Handle {
                            instructions: vec![
                                Instruction::AddI32 {
                                    destination: I32Location::Local(1),
                                    left: i32_local(1),
                                    right: i32_const(1),
                                },
                                Instruction::Continue,
                            ],
                        },
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(0),
                        right: i32_local(3),
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Local(1),
                        left: i32_local(1),
                        right: i32_const(1),
                    },
                ],
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
fn lowers_optional_i32_otherwise_never_call_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { abort() }

    return value
}

func maybe_answer(): i32? {
    return 42
}

func abort(): never {
    abort()
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Handle {
                        instructions: vec![Instruction::TailCall {
                            target: CallTarget::same_file("abort"),
                            arguments: vec![],
                        }],
                    },
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
fn lowers_optional_i32_otherwise_never_call_binding_with_scope_cleanup() {
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
    var file = File { fd: 3 }
    let value = maybe_answer() otherwise { abort() }

    return value
}

func maybe_answer(): i32? {
    return 42
}

func abort(): never {
    abort()
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
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
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Handle {
                        instructions: vec![
                            drop_call.clone(),
                            Instruction::TailCall {
                                target: CallTarget::same_file("abort"),
                                arguments: vec![],
                            },
                        ],
                    },
                },
                Instruction::SetI32 {
                    destination: I32Location::Local(1),
                    value: i32_local(0),
                },
                drop_call,
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_local(1),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_optional_i32_otherwise_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    return maybe_answer() otherwise { 7 }
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallFallibleI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Handle {
                        instructions: vec![set_return_i32(7), Instruction::Return],
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_optional_i32_otherwise_return_with_scope_cleanup() {
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
    return choose()
}

func choose(): i32 {
    var file = File { fd: 3 }
    return maybe_answer() otherwise { 7 }
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let choose = ir
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
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
            Instruction::CallFallibleI32 {
                destination: I32Location::Local(0),
                target: CallTarget::same_file("maybe_answer"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetI32 {
                            destination: I32Location::Local(0),
                            value: i32_const(7),
                        },
                        drop_call.clone(),
                        Instruction::SetI32 {
                            destination: I32Location::Return,
                            value: i32_local(0),
                        },
                        Instruction::Return,
                    ],
                },
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
fn lowers_optional_scalar_otherwise_returns() {
    let source = r#"func main(): i32 {
    return 0
}

func use_byte(): u8 {
    return maybe_byte() otherwise { 7 }
}

func maybe_byte(): u8? {
    return 42
}

func use_size(): usize {
    return maybe_size() otherwise { 7 }
}

func maybe_size(): usize? {
    return 42
}

func use_flag(): bool {
    return maybe_flag() otherwise { true }
}

func maybe_flag(): bool? {
    return false
}

func use_text(): &str {
    return maybe_text() otherwise { "fallback" }
}

func maybe_text(): &str? {
    return "text"
}

func use_bytes(bytes: &[u8]): &[u8] {
    return maybe_bytes(bytes) otherwise { bytes }
}

func maybe_bytes(bytes: &[u8]): &[u8]? {
    return bytes
}
"#;
    let signatures = function_signatures(vec![
        ("maybe_byte", Type::Fallible(Box::new(Type::U8)), vec![]),
        ("maybe_size", Type::Fallible(Box::new(Type::Usize)), vec![]),
        ("maybe_flag", Type::Fallible(Box::new(Type::Bool)), vec![]),
        ("maybe_text", Type::Fallible(Box::new(Type::Str)), vec![]),
        (
            "maybe_bytes",
            Type::Fallible(Box::new(Type::Slice {
                is_readwrite: false,
            })),
            vec![Type::Slice {
                is_readwrite: false,
            }],
        ),
    ]);

    let use_byte =
        lower_named_function_with_signatures(source, "use_byte", signatures.clone()).unwrap();
    assert_eq!(
        use_byte.instructions,
        vec![
            Instruction::CallFallibleU8 {
                destination: U8Location::Return,
                target: CallTarget::same_file("maybe_byte"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetU8 {
                            destination: U8Location::Return,
                            value: U8Value::Const(7),
                        },
                        Instruction::Return,
                    ],
                },
            },
            Instruction::Return,
        ]
    );

    let use_size =
        lower_named_function_with_signatures(source, "use_size", signatures.clone()).unwrap();
    assert_eq!(
        use_size.instructions,
        vec![
            Instruction::CallFallibleUsize {
                destination: UsizeLocation::Return,
                target: CallTarget::same_file("maybe_size"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetUsize {
                            destination: UsizeLocation::Return,
                            value: UsizeValue::Const(7),
                        },
                        Instruction::Return,
                    ],
                },
            },
            Instruction::Return,
        ]
    );

    let use_flag =
        lower_named_function_with_signatures(source, "use_flag", signatures.clone()).unwrap();
    assert_eq!(
        use_flag.instructions,
        vec![
            Instruction::CallFallibleBool {
                destination: BoolLocation::Return,
                target: CallTarget::same_file("maybe_flag"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetBool {
                            destination: BoolLocation::Return,
                            value: BoolValue::Const(true),
                        },
                        Instruction::Return,
                    ],
                },
            },
            Instruction::Return,
        ]
    );

    let use_text =
        lower_named_function_with_signatures(source, "use_text", signatures.clone()).unwrap();
    assert_eq!(
        use_text.instructions,
        vec![
            Instruction::CallFallibleStr {
                destination: StrLocation::Return,
                target: CallTarget::same_file("maybe_text"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetStr {
                            destination: StrLocation::Return,
                            value: str_static_value(b"fallback"),
                        },
                        Instruction::Return,
                    ],
                },
            },
            Instruction::Return,
        ]
    );

    let use_bytes = lower_named_function_with_signatures(source, "use_bytes", signatures).unwrap();
    assert_eq!(
        use_bytes.instructions,
        vec![
            Instruction::CallFallibleSlice {
                destination: SliceLocation::Return,
                target: CallTarget::same_file("maybe_bytes"),
                arguments: vec![ScalarArgument::Slice(SliceValue::Location(
                    SliceLocation::Parameter(0),
                ))],
                failure_mode: FallibleFailureMode::Handle {
                    instructions: vec![
                        Instruction::SetSlice {
                            destination: SliceLocation::Return,
                            value: SliceValue::Location(SliceLocation::Parameter(0)),
                        },
                        Instruction::Return,
                    ],
                },
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_optional_direct_aggregate_otherwise_return() {
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
    return 0
}

func choose(): Header {
    return make() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
}

func make(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "choose",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    let [
        Instruction::CallFallibleDirectAggregate {
            destination,
            target,
            arguments,
            layout,
            failure_mode: FallibleFailureMode::Handle { instructions },
        },
        Instruction::Return,
    ] = function.instructions.as_slice()
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::DirectReturn);
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert_eq!(*layout, ValueLayout::new(16, 8));
    assert!(instructions.contains(&Instruction::StoreAggregateI32 {
        destination: AggregateLocation::Slot(0),
        offset: 4,
        value: I32Value::Const(7),
    }));
    assert!(instructions.contains(&Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(0),
        layout: ValueLayout::new(16, 8),
    }));
    assert_eq!(instructions.last(), Some(&Instruction::Return));
}

#[test]
fn lowers_optional_indirect_aggregate_otherwise_return() {
    let aggregate_type = Type::Fallible(Box::new(Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    return 0
}

func choose(): Triple {
    return make() otherwise { Triple { first: 1, second: 7, third: 3 } }
}

func make(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
}
"#,
        "choose",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    let [
        Instruction::CallFallibleAggregate {
            destination,
            target,
            arguments,
            failure_mode: FallibleFailureMode::Handle { instructions },
        },
        Instruction::Return,
    ] = function.instructions.as_slice()
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::Return);
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert!(instructions.contains(&Instruction::StoreAggregateUsize {
        destination: AggregateLocation::Return,
        offset: 8,
        value: UsizeValue::Const(7),
    }));
    assert_eq!(instructions.last(), Some(&Instruction::Return));
}

#[test]
fn lowers_optional_direct_aggregate_otherwise_return_with_scope_cleanup() {
    let aggregate_type = Type::Fallible(Box::new(Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    }));
    let file_type = Type::DirectAggregate {
        layout: ValueLayout::new(4, 4),
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func choose(): Header {
    var file = File { fd: 3 }
    return make() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
}

func make(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "choose",
        function_signatures(vec![
            (
                "File.drop",
                Type::Void,
                vec![Type::Borrow {
                    is_readwrite: true,
                    inner: Box::new(file_type),
                }],
            ),
            ("make", aggregate_type, vec![]),
        ]),
    )
    .unwrap();

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let [
        Instruction::ReserveAggregateSlot {
            slot_index: file_slot,
            layout: file_layout,
        },
        Instruction::StoreAggregateI32 {
            destination: file_destination,
            offset: file_offset,
            value: file_value,
        },
        Instruction::ReserveAggregateSlot {
            slot_index: staged_slot,
            layout: staged_layout,
        },
        Instruction::CallFallibleDirectAggregate {
            destination,
            target,
            arguments,
            layout,
            failure_mode: FallibleFailureMode::Handle { instructions },
        },
        top_drop,
        Instruction::CopyAggregate {
            destination: top_copy_destination,
            source: top_copy_source,
            layout: top_copy_layout,
        },
        Instruction::Return,
    ] = function.instructions.as_slice()
    else {
        panic!("{function:?}");
    };

    assert_eq!(*file_slot, 0);
    assert_eq!(*file_layout, ValueLayout::new(4, 4));
    assert_eq!(*file_destination, AggregateLocation::Slot(0));
    assert_eq!(*file_offset, 0);
    assert_eq!(*file_value, i32_const(3));
    assert_eq!(*staged_slot, 1);
    assert_eq!(*staged_layout, ValueLayout::new(16, 8));
    assert_eq!(*destination, AggregateLocation::Slot(1));
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert_eq!(*layout, ValueLayout::new(16, 8));
    assert_eq!(top_drop, &drop_call);
    assert_eq!(*top_copy_destination, AggregateLocation::DirectReturn);
    assert_eq!(*top_copy_source, AggregateLocation::Slot(1));
    assert_eq!(*top_copy_layout, ValueLayout::new(16, 8));
    assert!(instructions.contains(&Instruction::StoreAggregateI32 {
        destination: AggregateLocation::Slot(1),
        offset: 4,
        value: i32_const(7),
    }));
    assert!(instructions.as_slice().ends_with(&[
        drop_call,
        Instruction::CopyAggregate {
            destination: AggregateLocation::DirectReturn,
            source: AggregateLocation::Slot(1),
            layout: ValueLayout::new(16, 8),
        },
        Instruction::Return,
    ]));
}

#[test]
fn lowers_optional_direct_aggregate_otherwise_fallible_return_with_scope_cleanup() {
    let header_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
    let file_type = Type::DirectAggregate {
        layout: ValueLayout::new(4, 4),
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func choose(): Header! {
    var file = File { fd: 3 }
    return make() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
}

func make(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "choose",
        function_signatures(vec![
            (
                "File.drop",
                Type::Void,
                vec![Type::Borrow {
                    is_readwrite: true,
                    inner: Box::new(file_type),
                }],
            ),
            (
                "make",
                Type::Fallible(Box::new(header_type.clone())),
                vec![],
            ),
        ]),
    )
    .unwrap();

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let copy_to_return = Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(1),
        layout: ValueLayout::new(16, 8),
    };
    let Some(Instruction::CallFallibleDirectAggregate {
        destination,
        failure_mode: FallibleFailureMode::Handle { instructions },
        ..
    }) = function
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallFallibleDirectAggregate { .. }))
    else {
        panic!("{function:?}");
    };

    assert_eq!(*destination, AggregateLocation::Slot(1));
    assert!(instructions.as_slice().ends_with(&[
        drop_call.clone(),
        copy_to_return.clone(),
        Instruction::ReturnFallibleSuccess,
    ]));
    assert!(function.instructions.ends_with(&[
        drop_call,
        copy_to_return,
        Instruction::ReturnFallibleSuccess,
    ]));
}

#[test]
fn lowers_optional_indirect_aggregate_otherwise_return_with_scope_cleanup() {
    let aggregate_type = Type::Fallible(Box::new(Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    }));
    let file_type = Type::DirectAggregate {
        layout: ValueLayout::new(4, 4),
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    return 0
}

func choose(): Triple {
    var file = File { fd: 3 }
    return make() otherwise { Triple { first: 1, second: 7, third: 3 } }
}

func make(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
}
"#,
        "choose",
        function_signatures(vec![
            (
                "File.drop",
                Type::Void,
                vec![Type::Borrow {
                    is_readwrite: true,
                    inner: Box::new(file_type),
                }],
            ),
            ("make", aggregate_type, vec![]),
        ]),
    )
    .unwrap();

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let [
        Instruction::ReserveAggregateSlot {
            slot_index: file_slot,
            layout: file_layout,
        },
        Instruction::StoreAggregateI32 {
            destination: file_destination,
            offset: file_offset,
            value: file_value,
        },
        Instruction::ReserveAggregateSlot {
            slot_index: staged_slot,
            layout: staged_layout,
        },
        Instruction::CallFallibleAggregate {
            destination,
            target,
            arguments,
            failure_mode: FallibleFailureMode::Handle { instructions },
        },
        top_drop,
        Instruction::CopyAggregate {
            destination: top_copy_destination,
            source: top_copy_source,
            layout: top_copy_layout,
        },
        Instruction::Return,
    ] = function.instructions.as_slice()
    else {
        panic!("{function:?}");
    };

    assert_eq!(*file_slot, 0);
    assert_eq!(*file_layout, ValueLayout::new(4, 4));
    assert_eq!(*file_destination, AggregateLocation::Slot(0));
    assert_eq!(*file_offset, 0);
    assert_eq!(*file_value, i32_const(3));
    assert_eq!(*staged_slot, 1);
    assert_eq!(*staged_layout, ValueLayout::new(24, 8));
    assert_eq!(*destination, AggregateLocation::Slot(1));
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert_eq!(top_drop, &drop_call);
    assert_eq!(*top_copy_destination, AggregateLocation::Return);
    assert_eq!(*top_copy_source, AggregateLocation::Slot(1));
    assert_eq!(*top_copy_layout, ValueLayout::new(24, 8));
    assert!(instructions.contains(&Instruction::StoreAggregateUsize {
        destination: AggregateLocation::Slot(1),
        offset: 8,
        value: usize_const(7),
    }));
    assert!(instructions.as_slice().ends_with(&[
        drop_call,
        Instruction::CopyAggregate {
            destination: AggregateLocation::Return,
            source: AggregateLocation::Slot(1),
            layout: ValueLayout::new(24, 8),
        },
        Instruction::Return,
    ]));
}

#[test]
fn lowers_optional_i32_otherwise_call_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = maybe_answer() otherwise { 7 }
    return value
}

func maybe_answer(): i32? {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("maybe_answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Recover {
                        instructions: vec![Instruction::SetI32 {
                            destination: I32Location::Local(0),
                            value: I32Value::Const(7),
                        }],
                    },
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
fn lowers_optional_scalar_otherwise_call_bindings() {
    let source = r#"func main(): i32 {
    return 0
}

func use_byte(): i32 {
    let value: u8 = maybe_byte() otherwise { 7 }
    return value as i32
}

func maybe_byte(): u8? {
    return 42
}

func use_size(): usize {
    let value = maybe_size() otherwise { 7 }
    return value
}

func maybe_size(): usize? {
    return 42
}

func use_flag(): i32 {
    let value = maybe_flag() otherwise { true }
    if value {
        return 42
    } else {
        return 1
    }
}

func maybe_flag(): bool? {
    return false
}

func use_text(): usize {
    let value = maybe_text() otherwise { "fallback" }
    return value.len()
}

func maybe_text(): &str? {
    return "text"
}

func use_bytes(bytes: &[u8]): usize {
    let value: &[u8] = maybe_bytes(bytes) otherwise { bytes }
    return value.len()
}

func maybe_bytes(bytes: &[u8]): &[u8]? {
    return bytes
}
"#;
    let signatures = function_signatures(vec![
        ("maybe_byte", Type::Fallible(Box::new(Type::U8)), vec![]),
        ("maybe_size", Type::Fallible(Box::new(Type::Usize)), vec![]),
        ("maybe_flag", Type::Fallible(Box::new(Type::Bool)), vec![]),
        ("maybe_text", Type::Fallible(Box::new(Type::Str)), vec![]),
        (
            "maybe_bytes",
            Type::Fallible(Box::new(Type::Slice {
                is_readwrite: false,
            })),
            vec![Type::Slice {
                is_readwrite: false,
            }],
        ),
    ]);

    let use_byte =
        lower_named_function_with_signatures(source, "use_byte", signatures.clone()).unwrap();
    assert_eq!(
        use_byte.instructions[0],
        Instruction::CallFallibleU8 {
            destination: U8Location::Local(0),
            target: CallTarget::same_file("maybe_byte"),
            arguments: vec![],
            failure_mode: FallibleFailureMode::Recover {
                instructions: vec![Instruction::SetU8 {
                    destination: U8Location::Local(0),
                    value: U8Value::Const(7),
                }],
            },
        }
    );

    let use_size =
        lower_named_function_with_signatures(source, "use_size", signatures.clone()).unwrap();
    assert_eq!(
        use_size.instructions[0],
        Instruction::CallFallibleUsize {
            destination: UsizeLocation::Local(0),
            target: CallTarget::same_file("maybe_size"),
            arguments: vec![],
            failure_mode: FallibleFailureMode::Recover {
                instructions: vec![Instruction::SetUsize {
                    destination: UsizeLocation::Local(0),
                    value: UsizeValue::Const(7),
                }],
            },
        }
    );

    let use_flag =
        lower_named_function_with_signatures(source, "use_flag", signatures.clone()).unwrap();
    assert_eq!(
        use_flag.instructions[0],
        Instruction::CallFallibleBool {
            destination: BoolLocation::Local(0),
            target: CallTarget::same_file("maybe_flag"),
            arguments: vec![],
            failure_mode: FallibleFailureMode::Recover {
                instructions: vec![Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(true),
                }],
            },
        }
    );

    let use_text =
        lower_named_function_with_signatures(source, "use_text", signatures.clone()).unwrap();
    assert_eq!(
        use_text.instructions[0],
        Instruction::CallFallibleStr {
            destination: StrLocation::Local(0),
            target: CallTarget::same_file("maybe_text"),
            arguments: vec![],
            failure_mode: FallibleFailureMode::Recover {
                instructions: vec![Instruction::SetStr {
                    destination: StrLocation::Local(0),
                    value: str_static_value(b"fallback"),
                }],
            },
        }
    );

    let use_bytes = lower_named_function_with_signatures(source, "use_bytes", signatures).unwrap();
    assert_eq!(
        use_bytes.instructions[0],
        Instruction::CallFallibleSlice {
            destination: SliceLocation::Local(0),
            target: CallTarget::same_file("maybe_bytes"),
            arguments: vec![ScalarArgument::Slice(SliceValue::Location(
                SliceLocation::Parameter(0),
            ))],
            failure_mode: FallibleFailureMode::Recover {
                instructions: vec![Instruction::SetSlice {
                    destination: SliceLocation::Local(0),
                    value: SliceValue::Location(SliceLocation::Parameter(0)),
                }],
            },
        }
    );
}

#[test]
fn diagnoses_nested_optional_success_none_return_without_panic() {
    let diagnostics = lower_named_function_diagnostics_with_signatures(
        r#"func main(): i32 {
    return 0
}

func value(): (i32?)! {
    return none
}
"#,
        "value",
        context::FunctionSignatures::new(HashMap::new()),
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E8007");
    assert!(diagnostics[0].message.contains("nested fallible"));
}

#[test]
fn lowers_fallible_i32_return_propagation() {
    let ir = lower_text(
        r#"func main(): i32! {
    return answer()?
}

func answer(): i32! {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::CallFallibleI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_fallible_i32_success_call_return_as_normal_call() {
    let ir = lower_text(
        r#"func main(): i32! {
    return answer()
}

func answer(): i32 {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::CallI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_fallible_i32_let_propagation() {
    let ir = lower_text(
        r#"func main(): i32! {
    let base = 2
    let value = answer()?
    return base + value
}

func answer(): i32! {
    return 40
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: I32Value::Const(2),
                },
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(1),
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: I32Value::Location(I32Location::Local(0)),
                    right: I32Value::Location(I32Location::Local(1)),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_fallible_scalar_let_propagation() {
    let ir = lower_text(
        r#"func main(): i32! {
    let byte_value: u8 = make_byte()?
    let size_value: usize = make_size()?
    let flag_value: bool = make_flag()?
    if flag_value && size_value == 40 {
        return byte_value as i32
    } else {
        return 1
    }
}

func make_byte(): u8! {
    return 42
}

func make_size(): usize! {
    return 40
}

func make_flag(): bool! {
    return true
}
"#,
    );

    let main = &ir.functions[0];
    assert_eq!(main.return_type, Type::Fallible(Box::new(Type::I32)));
    assert!(matches!(
        main.instructions[0],
        Instruction::CallFallibleU8 {
            destination: U8Location::Local(0),
            ..
        }
    ));
    assert!(matches!(
        main.instructions[1],
        Instruction::CallFallibleUsize {
            destination: UsizeLocation::Local(1),
            ..
        }
    ));
    assert!(matches!(
        main.instructions[2],
        Instruction::CallFallibleBool {
            destination: BoolLocation::Local(2),
            ..
        }
    ));
}

#[test]
fn lowers_fallible_str_and_slice_let_propagation() {
    let source = r#"func main(): i32 {
    return 0
}

func use_text(): usize! {
    let text: &str = make_text()?
    return text.len()
}

func make_text(): &str! {
    return "abc"
}

func use_bytes(bytes: &[u8]): usize! {
    let view: &[u8] = maybe_bytes(bytes)?
    return view.len()
}

func maybe_bytes(bytes: &[u8]): &[u8]! {
    return bytes
}
"#;

    let use_text = lower_named_function_with_signatures(
        source,
        "use_text",
        function_signatures(vec![(
            "make_text",
            Type::Fallible(Box::new(Type::Str)),
            vec![],
        )]),
    )
    .unwrap();
    assert_eq!(use_text.return_type, Type::Fallible(Box::new(Type::Usize)));
    assert!(matches!(
        use_text.instructions[0],
        Instruction::CallFallibleStr {
            destination: StrLocation::Local(0),
            ..
        }
    ));

    let use_bytes = lower_named_function_with_signatures(
        source,
        "use_bytes",
        function_signatures(vec![(
            "maybe_bytes",
            Type::Fallible(Box::new(readonly_u8_slice_type())),
            vec![readonly_u8_slice_type()],
        )]),
    )
    .unwrap();
    assert_eq!(use_bytes.return_type, Type::Fallible(Box::new(Type::Usize)));
    assert!(matches!(
        use_bytes.instructions[0],
        Instruction::CallFallibleSlice {
            destination: SliceLocation::Local(0),
            ..
        }
    ));
}

#[test]
fn lowers_fallible_str_call_result_len_return() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func size(): usize! {
    return make_text()?.len()
}

func make_text(): &str! {
    return "abc"
}
"#,
        "size",
        function_signatures(vec![(
            "make_text",
            Type::Fallible(Box::new(Type::Str)),
            vec![],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "size".to_string(),
            target: crate::ir::CallTarget::same_file("size".to_string()),
            return_type: Type::Fallible(Box::new(Type::Usize)),
            instructions: vec![
                Instruction::CallFallibleStr {
                    destination: StrLocation::Local(0),
                    target: CallTarget::same_file("make_text"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::StrLen(StrLocation::Local(0)),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_fallible_slice_call_result_index_return() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func first(bytes: &[u8]): u8! {
    return maybe_bytes(bytes)?[0]
}

func maybe_bytes(bytes: &[u8]): &[u8]! {
    return bytes
}
"#,
        "first",
        function_signatures(vec![(
            "maybe_bytes",
            Type::Fallible(Box::new(readonly_u8_slice_type())),
            vec![readonly_u8_slice_type()],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::Fallible(Box::new(Type::U8)),
            instructions: vec![
                Instruction::CallFallibleSlice {
                    destination: SliceLocation::Local(0),
                    target: CallTarget::same_file("maybe_bytes"),
                    arguments: vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Local(0),
                        index: usize_const(0),
                    },
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_fallible_force_unwrap_call_as_trapping_fallible_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = answer()!
    return value
}

func answer(): i32! {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Trap,
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::Location(I32Location::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_fallible_void_force_unwrap_statement_as_trapping_fallible_call() {
    let ir = lower_text(
        r#"func main(): void {
    effect()!
    return
}

func effect(): void! {
    return
}
"#,
    );

    assert_eq!(ir.functions[0].return_type, Type::Void);
    let [
        Instruction::CallFallibleVoid { failure_mode, .. },
        Instruction::Return,
    ] = ir.functions[0].instructions.as_slice()
    else {
        panic!(
            "unexpected main instructions: {:?}",
            ir.functions[0].instructions
        );
    };
    assert_eq!(*failure_mode, FallibleFailureMode::Trap);
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
fn lowers_optional_direct_aggregate_otherwise_return_call_binding() {
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
    let header = make() otherwise { return 7 }

    return header.code
}

func make(): Header? {
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
                failure_mode: FallibleFailureMode::Handle {
                    instructions: vec![set_return_i32(7), Instruction::Return],
                },
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_optional_indirect_aggregate_otherwise_return_call_binding() {
    let aggregate_type = Type::Fallible(Box::new(Type::Aggregate {
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
    let packet = make() otherwise { return 7 }

    return packet.header.code
}

func make(): Packet? {
    return Packet { prefix: 1, header: Header { tag: 7, ok: true, code: 42, len: 11 }, tail: 2 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Handle {
                    instructions: vec![set_return_i32(7), Instruction::Return],
                },
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_optional_direct_aggregate_otherwise_call_binding() {
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
    let header = make() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
    return header.code
}

func make(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    let Some(Instruction::CallFallibleDirectAggregate {
        destination,
        target,
        arguments,
        layout,
        failure_mode: FallibleFailureMode::Recover { instructions },
    }) = function
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallFallibleDirectAggregate { .. }))
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::Slot(0));
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert_eq!(*layout, ValueLayout::new(16, 8));
    assert!(instructions.contains(&Instruction::StoreAggregateI32 {
        destination: AggregateLocation::Slot(0),
        offset: 4,
        value: I32Value::Const(7),
    }));
}

#[test]
fn lowers_optional_indirect_aggregate_otherwise_call_binding() {
    let aggregate_type = Type::Fallible(Box::new(Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = make() otherwise { Triple { first: 1, second: 7, third: 3 } }
    if value.second == 42 {
        return 42
    } else {
        return 7
    }
}

func make(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    let Some(Instruction::CallFallibleAggregate {
        destination,
        target,
        arguments,
        failure_mode: FallibleFailureMode::Recover { instructions },
    }) = function
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallFallibleAggregate { .. }))
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::Slot(0));
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert!(instructions.contains(&Instruction::StoreAggregateUsize {
        destination: AggregateLocation::Slot(0),
        offset: 8,
        value: UsizeValue::Const(7),
    }));
}

#[test]
fn lowers_optional_direct_aggregate_otherwise_call_binding_from_copy_local_fallback() {
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
    let fallback = Header { tag: 1, ok: false, code: 7, len: 2 }
    let header = make() otherwise { fallback }
    return header.code
}

func make(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    let Some(Instruction::CallFallibleDirectAggregate {
        destination,
        failure_mode: FallibleFailureMode::Recover { instructions },
        ..
    }) = function
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallFallibleDirectAggregate { .. }))
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::Slot(1));
    assert!(instructions.contains(&Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(1),
        destination_offset: 0,
        source: AggregateLocation::Slot(0),
        source_offset: 0,
        layout: ValueLayout::new(16, 8),
    }));
}

#[test]
fn lowers_optional_indirect_aggregate_otherwise_call_binding_from_call_fallback() {
    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = make() otherwise { fallback() }
    if value.second == 42 {
        return 42
    } else {
        return 7
    }
}

func make(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
}

func fallback(): Triple {
    return Triple { first: 1, second: 7, third: 3 }
}
"#,
        "main",
        function_signatures(vec![
            (
                "make",
                Type::Fallible(Box::new(aggregate_type.clone())),
                vec![],
            ),
            ("fallback", aggregate_type, vec![]),
        ]),
    )
    .unwrap();

    let Some(Instruction::CallFallibleAggregate {
        destination,
        failure_mode: FallibleFailureMode::Recover { instructions },
        ..
    }) = function
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallFallibleAggregate { .. }))
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::Slot(0));
    assert!(instructions.contains(&Instruction::CallAggregate {
        destination: AggregateLocation::Slot(1),
        target: CallTarget::same_file("fallback"),
        arguments: vec![],
    }));
    assert!(instructions.contains(&Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(0),
        destination_offset: 0,
        source: AggregateLocation::Slot(1),
        source_offset: 0,
        layout: ValueLayout::new(24, 8),
    }));
}

#[test]
fn lowers_optional_aggregate_otherwise_value_arguments() {
    let header_layout = ValueLayout::new(16, 8);
    let triple_layout = ValueLayout::new(24, 8);
    let pair_layout = ValueLayout::new(8, 4);
    let header_type = Type::DirectAggregate {
        layout: header_layout,
        words: 2,
    };
    let triple_type = Type::Aggregate {
        layout: triple_layout,
    };
    let pair_type = Type::DirectAggregate {
        layout: pair_layout,
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let header_score = consume_header(maybe_header(false) otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } })
    let fallback = Triple { first: 2, second: 8, third: 4 }
    let triple_score = consume_triple(maybe_triple(false) otherwise { fallback })
    let pair_score = sum_pair(maybe_pair(false) otherwise { [3, 4] })
    return header_score + triple_score + pair_score
}

func consume_header(header: Header): i32 {
    return header.code
}

func consume_triple(triple: Triple): i32 {
    if triple.second == 8 {
        return 8
    }
    return 1
}

func sum_pair(pair: [i32; 2]): i32 {
    return pair[0] + pair[1]
}

func maybe_header(flag: bool): Header? {
    return none
}

func maybe_triple(flag: bool): Triple? {
    return none
}

func maybe_pair(flag: bool): [i32; 2]? {
    return none
}
"#,
        "main",
        function_signatures(vec![
            (
                "consume_header",
                Type::I32,
                vec![header_type.clone()],
            ),
            ("consume_triple", Type::I32, vec![triple_type.clone()]),
            ("sum_pair", Type::I32, vec![pair_type.clone()]),
            (
                "maybe_header",
                Type::Fallible(Box::new(header_type.clone())),
                vec![Type::Bool],
            ),
            (
                "maybe_triple",
                Type::Fallible(Box::new(triple_type.clone())),
                vec![Type::Bool],
            ),
            (
                "maybe_pair",
                Type::Fallible(Box::new(pair_type.clone())),
                vec![Type::Bool],
            ),
        ]),
    )
    .unwrap();

    let header_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallFallibleDirectAggregate {
            destination,
            target,
            layout,
            failure_mode: FallibleFailureMode::Recover { instructions },
            ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_header") && *layout == header_layout)
            .then_some((*destination, instructions))
    });
    let Some((header_destination, header_fallback)) = header_call else {
        panic!("{function:?}");
    };
    let AggregateLocation::Slot(header_slot) = header_destination else {
        panic!("{function:?}");
    };
    assert!(header_fallback.contains(&Instruction::StoreAggregateI32 {
        destination: header_destination,
        offset: 4,
        value: i32_const(7),
    }));
    let consume_header_arguments = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallI32 {
            target, arguments, ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("consume_header")).then_some(arguments)
    });
    let Some(consume_header_arguments) = consume_header_arguments else {
        panic!("{function:?}");
    };
    assert_eq!(consume_header_arguments.len(), 1, "{function:?}");
    assert!(matches!(
        &consume_header_arguments[0],
        ScalarArgument::AggregateDirect(DirectAggregateArgument {
            source: AggregateArgumentSource::Slot(slot),
            layout,
            words,
        }) if *slot == header_slot && *layout == header_layout && *words == 2
    ));

    let triple_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallFallibleAggregate {
            destination,
            target,
            failure_mode: FallibleFailureMode::Recover { instructions },
            ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_triple")).then_some((*destination, instructions))
    });
    let Some((triple_destination, triple_fallback)) = triple_call else {
        panic!("{function:?}");
    };
    assert!(triple_fallback.iter().any(|instruction| matches!(
        instruction,
        Instruction::CopyAggregateRange {
            destination,
            destination_offset: 0,
            source,
            source_offset: 0,
            layout,
        } if *destination == triple_destination
            && *source == AggregateLocation::Slot(1)
            && *layout == triple_layout
    )));

    let pair_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallFallibleDirectAggregate {
            destination,
            target,
            layout,
            failure_mode: FallibleFailureMode::Recover { instructions },
            ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_pair") && *layout == pair_layout)
            .then_some((*destination, instructions))
    });
    let Some((pair_destination, pair_fallback)) = pair_call else {
        panic!("{function:?}");
    };
    assert!(pair_fallback.contains(&Instruction::StoreAggregateI32 {
        destination: pair_destination,
        offset: 4,
        value: i32_const(4),
    }));
}

#[test]
fn lowers_optional_aggregate_otherwise_struct_literal_fields() {
    let header_layout = ValueLayout::new(16, 8);
    let triple_layout = ValueLayout::new(24, 8);
    let pair_layout = ValueLayout::new(8, 4);
    let packet_layout = ValueLayout::new(56, 8);
    let header_type = Type::DirectAggregate {
        layout: header_layout,
        words: 2,
    };
    let triple_type = Type::Aggregate {
        layout: triple_layout,
    };
    let pair_type = Type::DirectAggregate {
        layout: pair_layout,
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: usize
    second: usize
    third: usize
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
    pair: [i32; 2]
}

func main(): i32 {
    let fallback = Triple { first: 2, second: 8, third: 4 }
    let packet = Packet {
        prefix: 1,
        header: maybe_header(false) otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } },
        triple: maybe_triple(false) otherwise { fallback },
        pair: maybe_pair(false) otherwise { [3, 4] },
    }
    return packet.header.code + packet.pair[1]
}

func maybe_header(flag: bool): Header? {
    return none
}

func maybe_triple(flag: bool): Triple? {
    return none
}

func maybe_pair(flag: bool): [i32; 2]? {
    return none
}
"#,
        "main",
        function_signatures(vec![
            (
                "maybe_header",
                Type::Fallible(Box::new(header_type.clone())),
                vec![Type::Bool],
            ),
            (
                "maybe_triple",
                Type::Fallible(Box::new(triple_type.clone())),
                vec![Type::Bool],
            ),
            (
                "maybe_pair",
                Type::Fallible(Box::new(pair_type.clone())),
                vec![Type::Bool],
            ),
        ]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: packet_layout,
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(2),
                target: CallTarget::same_file("maybe_header"),
                arguments: vec![ScalarArgument::Bool(BoolValue::Const(false))],
                layout: header_layout,
                failure_mode: FallibleFailureMode::Recover {
                    instructions: vec![
                        Instruction::StoreAggregateU8 {
                            destination: AggregateLocation::Slot(2),
                            offset: 0,
                            value: u8_const(1),
                        },
                        Instruction::StoreAggregateBool {
                            destination: AggregateLocation::Slot(2),
                            offset: 1,
                            value: BoolValue::Const(false),
                        },
                        Instruction::StoreAggregateI32 {
                            destination: AggregateLocation::Slot(2),
                            offset: 4,
                            value: i32_const(7),
                        },
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::Slot(2),
                            offset: 8,
                            value: usize_const(2),
                        },
                    ],
                },
            })
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 8,
                source: AggregateLocation::Slot(2),
                source_offset: 0,
                layout: header_layout,
            })
    );

    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CallFallibleAggregate {
            destination: AggregateLocation::Slot(3),
            target,
            failure_mode: FallibleFailureMode::Recover { instructions },
            ..
        } if target == &CallTarget::same_file("maybe_triple")
            && instructions.contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(3),
                destination_offset: 0,
                source: AggregateLocation::Slot(0),
                source_offset: 0,
                layout: triple_layout,
            })
    )));
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 24,
                source: AggregateLocation::Slot(3),
                source_offset: 0,
                layout: triple_layout,
            })
    );

    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CallFallibleDirectAggregate {
            destination: AggregateLocation::Slot(4),
            target,
            layout,
            failure_mode: FallibleFailureMode::Recover { instructions },
            ..
        } if target == &CallTarget::same_file("maybe_pair")
            && *layout == pair_layout
            && instructions.contains(&Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(4),
                offset: 4,
                value: i32_const(4),
            })
    )));
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 48,
                source: AggregateLocation::Slot(4),
                source_offset: 0,
                layout: pair_layout,
            })
    );
}

#[test]
fn lowers_optional_aggregate_otherwise_assignments() {
    let header_layout = ValueLayout::new(16, 8);
    let triple_layout = ValueLayout::new(20, 4);
    let header_type = Type::DirectAggregate {
        layout: header_layout,
        words: 2,
    };
    let triple_type = Type::Aggregate {
        layout: triple_layout,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: i32
    second: i32
    third: i32
    fourth: i32
    fifth: i32
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
}

func main(): i32 {
    var header = Header { tag: 0, ok: false, code: 0, len: 0 }
    let fallback = Triple { first: 2, second: 8, third: 1, fourth: 1, fifth: 4 }
    var packet = Packet {
        prefix: 5,
        header: Header { tag: 3, ok: false, code: 3, len: 3 },
        triple: Triple { first: 1, second: 1, third: 1, fourth: 1, fifth: 1 },
    }
    header = maybe_header(false) otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
    packet.header = maybe_header(true) otherwise { Header { tag: 9, ok: false, code: 90, len: 9 } }
    packet.triple = maybe_triple(false) otherwise { fallback }
    return header.code + packet.header.code + packet.triple.second
}

func maybe_header(flag: bool): Header? {
    return none
}

func maybe_triple(flag: bool): Triple? {
    return none
}
"#,
        "main",
        function_signatures(vec![
            (
                "maybe_header",
                Type::Fallible(Box::new(header_type.clone())),
                vec![Type::Bool],
            ),
            (
                "maybe_triple",
                Type::Fallible(Box::new(triple_type.clone())),
                vec![Type::Bool],
            ),
        ]),
    )
    .unwrap();

    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target,
                arguments,
                layout,
                failure_mode: FallibleFailureMode::Recover { instructions },
            } if target == &CallTarget::same_file("maybe_header")
                && arguments.as_slice() == [ScalarArgument::Bool(BoolValue::Const(false))]
                && *layout == header_layout
                && instructions.contains(&Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: i32_const(7),
                })
        )),
        "{function:?}"
    );

    let field_header_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallFallibleDirectAggregate {
            destination,
            target,
            arguments,
            layout,
            failure_mode: FallibleFailureMode::Recover { instructions },
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_header")
            && arguments.as_slice() == [ScalarArgument::Bool(BoolValue::Const(true))]
            && *layout == header_layout)
            .then_some((*destination, instructions))
    });
    let Some((header_destination, header_fallback)) = field_header_call else {
        panic!("{function:?}");
    };
    assert!(header_fallback.contains(&Instruction::StoreAggregateI32 {
        destination: header_destination,
        offset: 4,
        value: i32_const(90),
    }));
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CopyAggregateRange {
            destination_offset: 8,
            source,
            source_offset: 0,
            layout,
            ..
        } if *source == header_destination && *layout == header_layout
    )));

    let triple_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallFallibleAggregate {
            destination,
            target,
            failure_mode: FallibleFailureMode::Recover { instructions },
            ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_triple")).then_some((*destination, instructions))
    });
    let Some((triple_destination, triple_fallback)) = triple_call else {
        panic!("{function:?}");
    };
    assert!(triple_fallback.contains(&Instruction::CopyAggregateRange {
        destination: triple_destination,
        destination_offset: 0,
        source: AggregateLocation::Slot(1),
        source_offset: 0,
        layout: triple_layout,
    }));
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CopyAggregateRange {
            destination_offset: 24,
            source,
            source_offset: 0,
            layout,
            ..
        } if *source == triple_destination && *layout == triple_layout
    )));
}

#[test]
fn lowers_optional_aggregate_otherwise_member_roots() {
    let packet_layout = ValueLayout::new(48, 8);
    let triple_layout = ValueLayout::new(20, 4);
    let packet_type = Type::Aggregate {
        layout: packet_layout,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: i32
    second: i32
    third: i32
    fourth: i32
    fifth: i32
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
}

func main(): i32 {
    let fallback = Packet {
        prefix: 5,
        header: Header { tag: 1, ok: false, code: 7, len: 2 },
        triple: Triple { first: 2, second: 8, third: 1, fourth: 1, fifth: 4 },
    }
    let code = (maybe_packet(false) otherwise { fallback }).header.code
    let triple = (maybe_packet(true) otherwise { fallback }).triple
    return code + triple.second
}

func maybe_packet(flag: bool): Packet? {
    return none
}
"#,
        "main",
        function_signatures(vec![(
            "maybe_packet",
            Type::Fallible(Box::new(packet_type.clone())),
            vec![Type::Bool],
        )]),
    )
    .unwrap();

    let scalar_member_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallFallibleAggregate {
            destination,
            target,
            arguments,
            failure_mode: FallibleFailureMode::Recover { instructions },
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_packet")
            && arguments.as_slice() == [ScalarArgument::Bool(BoolValue::Const(false))])
        .then_some((*destination, instructions))
    });
    let Some((scalar_source, scalar_fallback)) = scalar_member_call else {
        panic!("{function:?}");
    };
    assert!(scalar_fallback.contains(&Instruction::CopyAggregateRange {
        destination: scalar_source,
        destination_offset: 0,
        source: AggregateLocation::Slot(0),
        source_offset: 0,
        layout: packet_layout,
    }));
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::LoadAggregateI32 {
            source,
            offset: 12,
            ..
        } if *source == scalar_source
    )));

    let aggregate_member_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallFallibleAggregate {
            destination,
            target,
            arguments,
            ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_packet")
            && arguments.as_slice() == [ScalarArgument::Bool(BoolValue::Const(true))])
        .then_some(*destination)
    });
    let Some(aggregate_source) = aggregate_member_call else {
        panic!("{function:?}");
    };
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CopyAggregateRange {
            destination_offset: 0,
            source,
            source_offset: 24,
            layout,
            ..
        } if *source == aggregate_source && *layout == triple_layout
    )));
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

#[test]
fn lowers_fallible_aggregate_catch_call_binding() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    return run()?
}

func run(): i32! {
    let value = source() catch error {
        return Error.new("app.source", error.message)
    }
    return value.code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(0),
        "source",
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnFallibleSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_call_return() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    let value = forward()?
    return value.code
}

func forward(): Header! {
    return source() catch error {
        return Error.new("app.source", error.message)
    }
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "forward")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::DirectReturn,
        "source",
    );
    assert_eq!(
        main.instructions.last(),
        Some(&Instruction::ReturnFallibleSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_value_argument() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    return run()?
}

func run(): i32! {
    return consume(source() catch error {
        return Error.new("app.source", error.message)
    })
}

func consume(header: Header): i32 {
    return header.code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(0),
        "source",
    );
    assert!(main.instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::CallI32 { target, .. } if target == &CallTarget::same_file("consume"))
    }));
}

#[test]
fn lowers_fallible_aggregate_catch_member_field_read() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    return run()?
}

func run(): i32! {
    return (source() catch error {
        return Error.new("app.source", error.message)
    }).code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(0),
        "source",
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnFallibleSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_member_binding() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
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

func main(): i32! {
    return run()?
}

func run(): i32! {
    let header = (source() catch error {
        return Error.new("app.source", error.message)
    }).header
    return header.code
}

func source(): Packet! {
    return Packet {
        prefix: 1,
        header: Header { tag: 7, ok: true, code: 42, len: 11 },
        tail: 2,
    }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert!(main.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Slot(1),
                target,
                arguments,
                failure_mode: FallibleFailureMode::Catch { .. },
            } if target == &CallTarget::same_file("source") && arguments.is_empty()
        )
    }));
    assert!(
        main.instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 0,
                source: AggregateLocation::Slot(1),
                source_offset: 8,
                layout: ValueLayout::new(16, 8),
            })
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnFallibleSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_assignment() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    return run()?
}

func run(): i32! {
    var value = Header { tag: 1, ok: false, code: 2, len: 3 }
    value = source() catch error {
        return Error.new("app.source", error.message)
    }
    return value.code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(0),
        "source",
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnFallibleSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_member_assignment() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
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

func main(): i32! {
    return run()?
}

func run(): i32! {
    var packet = Packet {
        prefix: 1,
        header: Header { tag: 1, ok: false, code: 2, len: 3 },
        tail: 4,
    }
    packet.header = source() catch error {
        return Error.new("app.source", error.message)
    }
    return packet.header.code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(1),
        "source",
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnFallibleSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_struct_literal_field() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
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

func main(): i32! {
    return run()?
}

func run(): i32! {
    let packet = Packet {
        prefix: 1,
        header: source() catch error {
            return Error.new("app.source", error.message)
        },
        tail: 2,
    }
    return packet.header.code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(1),
        "source",
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnFallibleSuccess)
    );
}

#[test]
fn lowers_fallible_void_function_static_error_failure() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

func main(): void! {
    fail()?
}

func fail(): void! {
    return Error.new("app.inner", "inner failed")
}
"#,
    );

    let fail = ir
        .functions
        .iter()
        .find(|function| function.name == "fail")
        .unwrap();

    assert_eq!(
        fail.instructions,
        vec![Instruction::ReturnFallibleFailure {
            code: StrValue::StaticBytes(b"app.inner".to_vec()),
            message: StrValue::StaticBytes(b"inner failed".to_vec()),
        }]
    );
}

#[test]
fn lowers_fallible_void_function_static_error_helper_failure() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

func main(): void! {
    fail()?
}

func fail(): void! {
    return app_failed()
}

func app_failed(): error {
    return Error.new("app.failed", "failed")
}
"#,
    );

    let fail = ir
        .functions
        .iter()
        .find(|function| function.name == "fail")
        .unwrap();

    assert_eq!(
        fail.instructions,
        vec![Instruction::ReturnFallibleFailure {
            code: StrValue::StaticBytes(b"app.failed".to_vec()),
            message: StrValue::StaticBytes(b"failed".to_vec()),
        }]
    );
}

#[test]
fn lowers_fallible_i32_catch_failure_return() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

func main(): i32! {
    let value = answer() catch error {
        return Error.new("app.answer", error.message)
    }
    return value
}

func answer(): i32! {
    return Error.new("app.inner", "inner failed")
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Catch {
                        code: StrLocation::Local(1),
                        message: StrLocation::Local(3),
                        instructions: vec![Instruction::ReturnFallibleFailure {
                            code: StrValue::StaticBytes(b"app.answer".to_vec()),
                            message: StrValue::Location(StrLocation::Local(3)),
                        }],
                    },
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::Location(I32Location::Local(0)),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_pending_aggregate_drop_for_catch_failure_return_cleanup() {
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

func main(): i32! {
    var file = File { fd: 3 }
    let value = answer() catch error {
        return Error.new("app.answer", error.message)
    }
    return value
}

func answer(): i32! {
    return Error.new("app.inner", "inner failed")
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
    let Some(Instruction::CallFallibleI32 {
        failure_mode:
            FallibleFailureMode::Catch {
                code,
                message,
                instructions,
            },
        ..
    }) = main
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallFallibleI32 { .. }))
    else {
        panic!("missing fallible i32 catch call: {main:?}");
    };
    assert_eq!(*code, StrLocation::Local(1));
    assert_eq!(*message, StrLocation::Local(3));
    assert_eq!(
        instructions,
        &vec![
            drop_call,
            Instruction::ReturnFallibleFailure {
                code: StrValue::StaticBytes(b"app.answer".to_vec()),
                message: StrValue::Location(StrLocation::Local(3)),
            },
        ],
    );
}

#[test]
fn lowers_fallible_write_text_raw_catch_failure_return() {
    let ir = lower_text_with_nocter_home_files(
        r#"use std/io_catch.print_catch

func main(): void! {
    print_catch("hello\n")?
}
"#,
        &[
            std_error_file(),
            std_io_file(),
            (
                "std/io_catch.nct",
                r#"use std/error.Error
use std/io.write_text_raw

pub func print_catch(text: &str): void! {
    write_text_raw(1, text) catch error {
        return Error.new("app.write", error.message)
    }
    return
}
"#,
            ),
        ],
    );

    let print = ir
        .functions
        .iter()
        .find(|function| function.name == "print_catch")
        .unwrap();

    assert_eq!(
        print.instructions,
        vec![
            Instruction::WriteStr {
                fd: I32Value::Const(1),
                text: StrValue::Location(StrLocation::Parameter(0)),
            },
            Instruction::CheckFailure {
                failure_mode: FallibleFailureMode::Catch {
                    code: StrLocation::Local(0),
                    message: StrLocation::Local(2),
                    instructions: vec![Instruction::ReturnFallibleFailure {
                        code: StrValue::StaticBytes(b"app.write".to_vec()),
                        message: StrValue::Location(StrLocation::Local(2)),
                    }],
                },
            },
            Instruction::ReturnFallibleSuccess,
        ]
    );
}

#[test]
fn lowers_fallible_write_bytes_raw_catch_failure_return() {
    let write_bytes = lower_imported_named_function_with_nocter_home_files(
        r#"use std/io_bytes.write_bytes_catch

func main(): void {
    return
}
"#,
        "write_bytes_catch",
        &[
            std_error_file(),
            std_io_file(),
            (
                "std/io_bytes.nct",
                r#"use std/error.Error
use std/io.write_bytes_raw

pub func write_bytes_catch(bytes: &[u8]): void! {
    write_bytes_raw(1, bytes) catch error {
        return Error.new("app.write", error.message)
    }
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        write_bytes.instructions,
        vec![
            Instruction::WriteSlice {
                fd: I32Value::Const(1),
                bytes: SliceValue::Location(SliceLocation::Parameter(0)),
            },
            Instruction::CheckFailure {
                failure_mode: FallibleFailureMode::Catch {
                    code: StrLocation::Local(0),
                    message: StrLocation::Local(2),
                    instructions: vec![Instruction::ReturnFallibleFailure {
                        code: StrValue::StaticBytes(b"app.write".to_vec()),
                        message: StrValue::Location(StrLocation::Local(2)),
                    }],
                },
            },
            Instruction::ReturnFallibleSuccess,
        ]
    );
}

#[test]
fn lowers_fallible_read_bytes_raw_propagation() {
    let read_bytes = lower_imported_named_function_with_nocter_home_files(
        r#"use std/io_bytes.read_count

func main(): void {
    return
}
"#,
        "read_count",
        &[
            std_io_file(),
            (
                "std/io_bytes.nct",
                r#"use std/io.read_bytes_raw

pub func read_count(buffer: &+[u8]): usize! {
    return read_bytes_raw(0, buffer)?
}
"#,
            ),
        ],
    );

    assert_eq!(
        read_bytes.instructions,
        vec![
            Instruction::ReadSlice {
                destination: UsizeLocation::Return,
                fd: I32Value::Const(0),
                buffer: SliceValue::Location(SliceLocation::Parameter(0)),
                failure_mode: FallibleFailureMode::Propagate,
            },
            Instruction::ReturnFallibleSuccess,
        ]
    );
}

#[test]
fn lowers_fallible_read_bytes_raw_catch_binding() {
    let read_bytes = lower_imported_named_function_with_nocter_home_files(
        r#"use std/io_bytes_catch.read_count_catch

func main(): void {
    return
}
"#,
        "read_count_catch",
        &[
            std_error_file(),
            std_io_file(),
            (
                "std/io_bytes_catch.nct",
                r#"use std/error.Error
use std/io.read_bytes_raw

pub func read_count_catch(buffer: &+[u8]): usize! {
    let count = read_bytes_raw(0, buffer) catch error {
        return Error.new("app.read", error.message)
    }
    return count
}
"#,
            ),
        ],
    );

    assert_eq!(
        read_bytes.instructions,
        vec![
            Instruction::ReadSlice {
                destination: UsizeLocation::Local(0),
                fd: I32Value::Const(0),
                buffer: SliceValue::Location(SliceLocation::Parameter(0)),
                failure_mode: FallibleFailureMode::Catch {
                    code: StrLocation::Local(1),
                    message: StrLocation::Local(3),
                    instructions: vec![Instruction::ReturnFallibleFailure {
                        code: StrValue::StaticBytes(b"app.read".to_vec()),
                        message: StrValue::Location(StrLocation::Local(3)),
                    }],
                },
            },
            Instruction::SetUsize {
                destination: UsizeLocation::Return,
                value: UsizeValue::Location(UsizeLocation::Local(0)),
            },
            Instruction::ReturnFallibleSuccess,
        ]
    );
}

#[test]
fn lowers_fallible_open_read_raw_propagation() {
    let open = lower_imported_named_function_with_nocter_home_files(
        r#"use std/io_open.open_raw

func main(): void {
    return
}
"#,
        "open_raw",
        &[
            (
                "std/io.nct",
                r#"#target("arm64-darwin")
pub(nocter) primitive open_read_raw(path: *u8): i32!
"#,
            ),
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
"#,
            ),
            (
                "std/io_open.nct",
                r#"use std/io.open_read_raw
use std/ptr.from_addr

pub func open_raw(address: usize): i32! {
    return open_read_raw(from_addr(address))?
}
"#,
            ),
        ],
    );

    assert_eq!(
        open.instructions,
        vec![
            Instruction::OpenRead {
                destination: I32Location::Return,
                path: UsizeValue::Location(UsizeLocation::Parameter(0)),
                failure_mode: FallibleFailureMode::Propagate,
            },
            Instruction::ReturnFallibleSuccess,
        ]
    );
}

#[test]
fn lowers_fallible_entry_return_static_error_constructor() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

func main(): i32! {
    return Error.new("app.failed", "failed")
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![Instruction::ReturnFallibleFailure {
                code: StrValue::StaticBytes(b"app.failed".to_vec()),
                message: StrValue::StaticBytes(b"failed".to_vec()),
            }],
        }])
    );
}

#[test]
fn lowers_fallible_entry_return_dynamic_error_message() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

func main(): i32! {
    return Error.new("app.failed", dynamic())
}

func dynamic(): &str {
    return "failed"
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::CallStr {
                    destination: StrLocation::Local(0),
                    target: CallTarget::same_file("dynamic"),
                    arguments: vec![],
                },
                Instruction::ReturnFallibleFailure {
                    code: StrValue::StaticBytes(b"app.failed".to_vec()),
                    message: StrValue::Location(StrLocation::Local(0)),
                },
            ],
        }
    );
}

#[test]
fn lowers_fallible_entry_return_error_local_dynamic_message() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

func main(): i32! {
    let value = Error.new("app.failed", dynamic())
    return value
}

func dynamic(): &str {
    return "failed"
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::CallStr {
                    destination: StrLocation::Local(4),
                    target: CallTarget::same_file("dynamic"),
                    arguments: vec![],
                },
                Instruction::SetStr {
                    destination: StrLocation::Local(0),
                    value: StrValue::StaticBytes(b"app.failed".to_vec()),
                },
                Instruction::SetStr {
                    destination: StrLocation::Local(2),
                    value: StrValue::Location(StrLocation::Local(4)),
                },
                Instruction::ReturnFallibleFailure {
                    code: StrValue::Location(StrLocation::Local(0)),
                    message: StrValue::Location(StrLocation::Local(2)),
                },
            ],
        }
    );
}

#[test]
fn lowers_fallible_entry_forwarded_error_parameter_failure() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

func main(): i32! {
    return forward(Error.new("app.failed", "failed"))?
}

func forward(error: error): i32! {
    return error
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
            Instruction::SetStr {
                destination: StrLocation::Local(0),
                value: StrValue::StaticBytes(b"app.failed".to_vec()),
            },
            Instruction::SetStr {
                destination: StrLocation::Local(2),
                value: StrValue::StaticBytes(b"failed".to_vec()),
            },
            Instruction::CallFallibleI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("forward"),
                arguments: vec![
                    ScalarArgument::Str(StrValue::Location(StrLocation::Local(0))),
                    ScalarArgument::Str(StrValue::Location(StrLocation::Local(2))),
                ],
                failure_mode: FallibleFailureMode::Propagate,
            },
            Instruction::ReturnFallibleSuccess,
        ]
    );

    let forward = ir
        .functions
        .iter()
        .find(|function| function.name == "forward")
        .unwrap();
    assert_eq!(
        forward.instructions,
        vec![Instruction::ReturnFallibleFailure {
            code: StrValue::Location(StrLocation::Parameter(0)),
            message: StrValue::Location(StrLocation::Parameter(2)),
        }]
    );
}

#[test]
fn lowers_fallible_entry_return_dynamic_error_code_and_message() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

func main(): i32! {
    return Error.new(dynamic_code(), dynamic_message())
}

func dynamic_code(): &str {
    return "app.failed"
}

func dynamic_message(): &str {
    return "failed"
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::CallStr {
                    destination: StrLocation::Local(0),
                    target: CallTarget::same_file("dynamic_code"),
                    arguments: vec![],
                },
                Instruction::CallStr {
                    destination: StrLocation::Local(2),
                    target: CallTarget::same_file("dynamic_message"),
                    arguments: vec![],
                },
                Instruction::ReturnFallibleFailure {
                    code: StrValue::Location(StrLocation::Local(0)),
                    message: StrValue::Location(StrLocation::Local(2)),
                },
            ],
        }
    );
}

#[test]
fn lowers_fallible_entry_return_static_error_constructor_with_multi_line_message() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

func main(): i32! {
    return Error.new("app.failed", """
        failed
        later
        """)
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![Instruction::ReturnFallibleFailure {
            code: StrValue::StaticBytes(b"app.failed".to_vec()),
            message: StrValue::StaticBytes(b"failed\nlater".to_vec()),
        }]
    );
}

#[test]
fn lowers_fallible_entry_return_error_message_without_duplicate_newline() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

func main(): i32! {
    return Error.new("app.failed", "failed\n")
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![Instruction::ReturnFallibleFailure {
            code: StrValue::StaticBytes(b"app.failed".to_vec()),
            message: StrValue::StaticBytes(b"failed\n".to_vec()),
        }]
    );
}

#[test]
fn lowers_fallible_catch_direct_error_return() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

func main(): i32! {
    let value = answer() catch error {
        return error
    }
    return value
}

func answer(): i32! {
    return Error.new("app.inner", "inner failed")
}
"#,
    );

    assert_eq!(
        ir.functions[0],
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Catch {
                        code: StrLocation::Local(1),
                        message: StrLocation::Local(3),
                        instructions: vec![Instruction::ReturnFallibleFailure {
                            code: StrValue::Location(StrLocation::Local(1)),
                            message: StrValue::Location(StrLocation::Local(3)),
                        }],
                    },
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::Location(I32Location::Local(0)),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_fallible_void_entry_propagating_std_print() {
    let ir = lower_text_with_nocter_home_files(
        r#"use std/io.print

func main(): void! {
    print("hello\n")?
}
"#,
        &[std_io_file()],
    );

    let [main, print] = ir.functions.as_slice() else {
        panic!("unexpected lowered functions: {:?}", ir.functions);
    };

    assert_eq!(main.return_type, Type::Fallible(Box::new(Type::Void)));
    let [
        Instruction::CallFallibleVoid {
            target, arguments, ..
        },
        Instruction::ReturnFallibleSuccess,
    ] = main.instructions.as_slice()
    else {
        panic!("unexpected main instructions: {:?}", main.instructions);
    };
    assert!(matches!(target, CallTarget::Imported { name, .. } if name == "print"));
    assert_eq!(arguments, &vec![str_static(b"hello\n")]);

    assert_eq!(print.return_type, Type::Fallible(Box::new(Type::Void)));
    assert!(matches!(
        print.target,
        CallTarget::Imported { ref name, .. } if name == "print"
    ));
    assert_eq!(
        print.instructions,
        vec![
            Instruction::WriteStr {
                fd: I32Value::Const(1),
                text: StrValue::Location(StrLocation::Parameter(0)),
            },
            Instruction::PropagateFailure,
            Instruction::ReturnFallibleSuccess,
        ]
    );
}
