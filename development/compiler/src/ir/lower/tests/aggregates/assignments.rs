use super::*;

#[test]
fn lowers_readwrite_borrowed_aggregate_parameter_field_assignment() {
    let function = lower_named_function(
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func set_code(header: &+Header): void {
    header.code = 99
    return
}
"#,
        "set_code",
    );

    assert_eq!(
        function,
        Function {
            name: "set_code".to_string(),
            target: CallTarget::same_file("set_code"),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Borrow(UsizeLocation::Parameter(0)),
                    offset: 4,
                    value: I32Value::Const(99),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_borrowed_aggregate_parameter_field_assignment_inside_nonterminal_if() {
    let function = lower_named_function(
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func set_code(header: &+Header): void {
    if true {
        header.code = 99
    }
    return
}
"#,
        "set_code",
    );

    assert_eq!(
        function,
        Function {
            name: "set_code".to_string(),
            target: CallTarget::same_file("set_code"),
            return_type: Type::Void,
            instructions: vec![
                Instruction::If {
                    condition: BoolValue::Const(true),
                    then_instructions: vec![Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Borrow(UsizeLocation::Parameter(0)),
                        offset: 4,
                        value: I32Value::Const(99),
                    }],
                    else_instructions: Vec::new(),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readwrite_borrowed_aggregate_parameter_field_assignment_inside_nonterminal_while() {
    let function = lower_named_function(
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func set_code(header: &+Header): void {
    while ready() {
        header.code = 99
    }
    return
}

func ready(): bool {
    return false
}
"#,
        "set_code",
    );

    assert_eq!(
        function,
        Function {
            name: "set_code".to_string(),
            target: CallTarget::same_file("set_code"),
            return_type: Type::Void,
            instructions: vec![
                Instruction::While {
                    condition_instructions: vec![call_bool(
                        BoolLocation::Local(0),
                        "ready",
                        vec![],
                    )],
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    body_instructions: vec![Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Borrow(UsizeLocation::Parameter(0)),
                        offset: 4,
                        value: I32Value::Const(99),
                    }],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_outer_aggregate_assignment_inside_nonterminal_if_branch_before_return_suffix() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 1 }
    if true {
        file = File { fd: 2 }
        touch()
        return file.fd
    }
    return 0
}

func touch(): void {
    return
}
"#,
    );

    let drop_file = Instruction::CallVoid {
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
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(1),
                        offset: 0,
                        value: i32_const(2),
                    },
                    drop_file.clone(),
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(0),
                        source: AggregateLocation::Slot(1),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("touch"),
                        arguments: vec![],
                    },
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Return,
                        source: AggregateLocation::Slot(0),
                        offset: 0,
                    },
                    drop_file.clone(),
                    Instruction::Return,
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            drop_file,
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_aggregate_assignment_inside_nonterminal_while_body() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 1 }
    while false {
        file = File { fd: 2 }
    }
    return 0
}
"#,
    );

    let drop_file = Instruction::CallVoid {
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
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::Const(false),
                body_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(1),
                        offset: 0,
                        value: i32_const(2),
                    },
                    drop_file.clone(),
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(0),
                        source: AggregateLocation::Slot(1),
                        layout: ValueLayout::new(4, 4),
                    },
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            drop_file,
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_aggregate_assignment_inside_nonterminal_if_branch() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 1 }
    if true {
        file = File { fd: 2 }
    }
    return file.fd
}
"#,
    );

    let replacement_drop = Instruction::CallVoid {
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
    assert!(
        main.instructions.contains(&Instruction::If {
            condition: BoolValue::Const(true),
            then_instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(1),
                    offset: 0,
                    value: i32_const(2),
                },
                replacement_drop,
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::Slot(1),
                    layout: ValueLayout::new(4, 4),
                },
            ],
            else_instructions: vec![],
        }),
        "{main:?}"
    );
    assert!(
        main.instructions.contains(&Instruction::LoadAggregateI32 {
            destination: I32Location::Return,
            source: AggregateLocation::Slot(0),
            offset: 0,
        }),
        "{main:?}"
    );
}

#[test]
fn lowers_outer_aggregate_assignment_before_loop_control() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 1 }
    while false {
        file = File { fd: 2 }
        break
        return 1
    }
    return 0
}
"#,
    );

    let replacement_drop = Instruction::CallVoid {
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
    assert!(
        main.instructions.contains(&Instruction::While {
            condition_instructions: vec![],
            condition: BoolValue::Const(false),
            body_instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(1),
                    offset: 0,
                    value: i32_const(2),
                },
                replacement_drop,
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::Slot(1),
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::Break,
            ],
        }),
        "{main:?}"
    );
}

#[test]
fn lowers_copy_aggregate_slot_assignment_borrow_argument() {
    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Text {
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
    var source = Text { start: 1, len: 2, capacity: 3 }
    var target = Text { start: 4, len: 5, capacity: 6 }
    target = source
    touch(&+target)
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
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(1),
                    offset: 0,
                    value: usize_const(4),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(1),
                    offset: 8,
                    value: usize_const(5),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(1),
                    offset: 16,
                    value: usize_const(6),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(1),
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("touch"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlot(1),
                    })],
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_copy_aggregate_call_result_assignment_borrow_argument() {
    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Text {
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

func use_text(): i32 {
    var source = make()
    var target = make()
    target = source
    touch(&+target)
    return 0
}
"#,
        "use_text",
        function_signatures(vec![
            ("make", aggregate_type.clone(), vec![]),
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
            return_type: Type::I32,
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
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(1),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(1),
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("touch"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlot(1),
                    })],
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_copy_aggregate_alias_slot_assignment() {
    let aggregate_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Pair {
    left: usize
    right: usize
}

type PairAlias = Pair

func main(): i32 {
    return 0
}

func touch(value: &+Pair): void {
    return
}

func use_pair(): i32 {
    var source = PairAlias { left: 1, right: 2 }
    var target = PairAlias { left: 3, right: 4 }
    target = source
    touch(&+target)
    return 0
}
"#,
        "use_pair",
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

    assert!(function.instructions.contains(&Instruction::CopyAggregate {
        destination: AggregateLocation::Slot(1),
        source: AggregateLocation::Slot(0),
        layout: ValueLayout::new(16, 8),
    }));
}

#[test]
fn lowers_direct_aggregate_call_assignment_borrow_argument() {
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

func reset_allocator(): Allocator {
    return Allocator { state: 1, kind: 2 }
}

func touch(allocator: &+Allocator): void {
    return
}

func use_allocator(): i32 {
    var allocator = page_allocator()
    allocator = reset_allocator()
    touch(&+allocator)
    return 0
}
"#,
        "use_allocator",
        function_signatures(vec![
            ("page_allocator", aggregate_type.clone(), vec![]),
            ("reset_allocator", aggregate_type.clone(), vec![]),
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
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallDirectAggregate {
                    destination: AggregateLocation::Slot(1),
                    target: CallTarget::same_file("reset_allocator"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::Slot(1),
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
fn lowers_indirect_aggregate_call_assignment_borrow_argument() {
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
    return Text { start: 4, len: 5, capacity: 6 }
}

func touch(value: &+Text): void {
    return
}

func use_text(): i32 {
    var value = Text { start: 1, len: 2, capacity: 3 }
    value = make()
    touch(&+value)
    return 0
}
"#,
        "use_text",
        function_signatures(vec![
            ("make", aggregate_type.clone(), vec![]),
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
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(1),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::Slot(1),
                    layout: ValueLayout::new(24, 8),
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
fn lowers_nested_aggregate_scalar_field_assignment_to_local_slot() {
    let function = lower_named_function(
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

func update_code(): i32 {
    var packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: true, code: 42, len: 11 },
        tail: 99,
    }
    packet.header.code = 100
    return packet.header.code
}
"#,
        "update_code",
    );

    assert!(
        function
            .instructions
            .contains(&Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 12,
                value: I32Value::Const(100),
            })
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::LoadAggregateI32 {
                destination: I32Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 12,
            })
    );
}

#[test]
fn lowers_non_copy_aggregate_field_replacement_assignment() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

struct Holder {
    tag: i32
    file: File
}

func main(): i32 {
    var holder = Holder { tag: 1, file: File { fd: 1 } }
    holder.file = File { fd: 2 }
    return holder.file.fd
}
"#,
    );
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(4, 4),
            })
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(1),
                offset: 0,
                value: I32Value::Const(2),
            })
    );
    assert!(function.instructions.contains(&Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlotField {
                slot_index: 0,
                offset: 4,
            },
        })],
    }));
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 4,
                source: AggregateLocation::Slot(1),
                source_offset: 0,
                layout: ValueLayout::new(4, 4),
            })
    );
}

#[test]
fn lowers_non_copy_borrowed_aggregate_field_replacement_assignment() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

struct Holder {
    tag: i32
    file: File
}

func main(): i32 {
    var holder = Holder { tag: 1, file: File { fd: 1 } }
    replace(&+holder)
    return holder.file.fd
}

func replace(holder: &+Holder): void {
    holder.file = File { fd: 2 }
    return
}
"#,
    );
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "replace")
        .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            })
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: I32Value::Const(2),
            })
    );
    assert!(function.instructions.contains(&Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::BorrowLocalField {
                pointer: UsizeLocation::Parameter(0),
                offset: 4,
            },
        })],
    }));
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Borrow(UsizeLocation::Parameter(0)),
                destination_offset: 4,
                source: AggregateLocation::Slot(0),
                source_offset: 0,
                layout: ValueLayout::new(4, 4),
            })
    );
}

#[test]
fn lowers_non_copy_aggregate_field_replacement_assignment_from_move() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

struct Holder {
    tag: i32
    file: File
}

func main(): i32 {
    var replacement = File { fd: 2 }
    var holder = Holder { tag: 1, file: File { fd: 1 } }
    holder.file = move replacement
    return holder.file.fd
}
"#,
    );
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(2),
                destination_offset: 0,
                source: AggregateLocation::Slot(0),
                source_offset: 0,
                layout: ValueLayout::new(4, 4),
            })
    );
    assert!(function.instructions.contains(&Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlotField {
                slot_index: 1,
                offset: 4,
            },
        })],
    }));
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 4,
                source: AggregateLocation::Slot(2),
                source_offset: 0,
                layout: ValueLayout::new(4, 4),
            })
    );
    assert!(!function.instructions.contains(&Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    }));
}

#[test]
fn lowers_non_copy_aggregate_field_replacement_assignment_from_call() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

struct Holder {
    tag: i32
    file: File
}

func main(): i32 {
    var holder = Holder { tag: 1, file: File { fd: 1 } }
    holder.file = make_file()
    return holder.file.fd
}

func make_file(): File {
    return File { fd: 2 }
}
"#,
    );
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(1),
                target: CallTarget::same_file("make_file"),
                arguments: vec![],
                layout: ValueLayout::new(4, 4),
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
                layout: ValueLayout::new(4, 4),
            })
    );
    assert!(function.instructions.contains(&Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlotField {
                slot_index: 0,
                offset: 4,
            },
        })],
    }));
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 4,
                source: AggregateLocation::Slot(1),
                source_offset: 0,
                layout: ValueLayout::new(4, 4),
            })
    );
}

#[test]
fn lowers_nested_aggregate_field_copy_assignment() {
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
    let header = Header { tag: 8, ok: true, code: 42, len: 12 }
    packet.header = header
    return packet.header.code
}
"#,
        "update",
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
fn lowers_nested_aggregate_field_call_assignment() {
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

func update(): i32 {
    var packet = Packet { prefix: 1, header: Header { tag: 7, ok: false, code: 1, len: 11 }, tail: 2 }
    packet.header = make_header()
    return packet.header.code
}
"#,
        "update",
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
}

#[test]
fn lowers_nested_aggregate_field_member_assignment_from_call_result() {
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

func update(): i32 {
    var packet = Packet { prefix: 1, header: Header { tag: 7, ok: false, code: 1, len: 11 }, tail: 2 }
    packet.header = make().header
    return packet.header.code
}
"#,
        "update",
        function_signatures(vec![("make", packet_type, vec![])]),
    )
    .unwrap();

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
fn lowers_aggregate_scalar_field_assignments_to_local_slot() {
    let function = lower_named_function(
        r#"struct Header {
    tag: u8
    ok: bool
    small: u16
    code: i32
    wide: u32
    len: usize
}

func main(): i32 {
    return 0
}

func update(): i32 {
    var value = Header { tag: 7, ok: true, small: 8, code: 42, wide: 10, len: 11 }
    value.tag = 9
    value.ok = false
    value.small = 12
    value.code = 99
    value.wide = 100
    value.len = 13
    return value.code
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
                    layout: ValueLayout::new(24, 8),
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
                Instruction::StoreAggregateInteger {
                    kind: crate::integer::IntegerType::U16,
                    destination: AggregateLocation::Slot(0),
                    offset: 2,
                    value: usize_const(8),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: i32_const(42),
                },
                Instruction::StoreAggregateInteger {
                    kind: crate::integer::IntegerType::U32,
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: usize_const(10),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 16,
                    value: usize_const(11),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: u8_const(9),
                },
                Instruction::StoreAggregateBool {
                    destination: AggregateLocation::Slot(0),
                    offset: 1,
                    value: BoolValue::Const(false),
                },
                Instruction::StoreAggregateInteger {
                    kind: crate::integer::IntegerType::U16,
                    destination: AggregateLocation::Slot(0),
                    offset: 2,
                    value: usize_const(12),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: i32_const(99),
                },
                Instruction::StoreAggregateInteger {
                    kind: crate::integer::IntegerType::U32,
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: usize_const(100),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 16,
                    value: usize_const(13),
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 4,
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_i32_aggregate_field_compound_assignment_with_call_rhs() {
    let ir = lower_text(
        r#"struct Counter {
    pad: i32
    value: i32
}

func main(): i32 {
    var counter = Counter { pad: 0, value: 40 }
    counter.value += answer()
    return counter.value
}

func answer(): i32 {
    return 2
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(8, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(0),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 4,
                value: i32_const(40),
            },
            call_i32(I32Location::Local(0), "answer", vec![]),
            Instruction::LoadAggregateI32 {
                destination: I32Location::Local(1),
                source: AggregateLocation::Slot(0),
                offset: 4,
            },
            Instruction::AddI32 {
                destination: I32Location::Local(2),
                left: i32_local(1),
                right: i32_local(0),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 4,
                value: i32_local(2),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 4,
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_usize_aggregate_field_compound_remainder_assignment() {
    let ir = lower_text(
        r#"struct Counter {
    pad: i32
    size: usize
}

func main(): usize {
    var counter = Counter { pad: 0, size: 47 }
    counter.size %= 5
    return counter.size
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(16, 8),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(0),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(0),
                offset: 8,
                value: usize_const(47),
            },
            Instruction::LoadAggregateUsize {
                destination: UsizeLocation::Local(0),
                source: AggregateLocation::Slot(0),
                offset: 8,
            },
            Instruction::RemainderUsize {
                destination: UsizeLocation::Local(1),
                left: usize_local(0),
                right: usize_const(5),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(0),
                offset: 8,
                value: usize_local(1),
            },
            Instruction::LoadAggregateUsize {
                destination: UsizeLocation::Return,
                source: AggregateLocation::Slot(0),
                offset: 8,
            },
            Instruction::Return,
        ]
    );
}
