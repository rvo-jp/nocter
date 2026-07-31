use super::*;

#[test]
fn lowers_method_call_aggregate_field_receiver_as_implicit_readonly_borrow() {
    let ir = lower_text(
        r#"copy struct File {
    fd: i32
}

copy struct Holder {
    tag: i32
    file: File
}

impl File {
    method &self.value(): i32 {
        return self.fd
    }
}

func main(): i32 {
    let holder = Holder { tag: 1, file: File { fd: 42 } }
    return holder.file.value()
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallI32 {
                    destination: I32Location::Return,
                    target,
                    arguments,
                } if target == &CallTarget::same_file("File.value")
                    && arguments == &vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlotField {
                            slot_index: 0,
                            offset: 4,
                        },
                    })]
            )
        }),
        "{main:?}"
    );
}

#[test]
fn lowers_method_call_aggregate_field_receiver_as_implicit_readwrite_borrow() {
    let ir = lower_text(
        r#"copy struct File {
    fd: i32
}

copy struct Holder {
    tag: i32
    file: File
}

impl File {
    method &+self.touch(): void {
        return
    }
}

func main(): i32 {
    var holder = Holder { tag: 1, file: File { fd: 42 } }
    holder.file.touch()
    return holder.file.fd
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallVoid {
                    target,
                    arguments,
                } if target == &CallTarget::same_file("File.touch")
                    && arguments == &vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlotField {
                            slot_index: 0,
                            offset: 4,
                        },
                    })]
            )
        }),
        "{main:?}"
    );
}

#[test]
fn lowers_imported_function_returning_hidden_nested_aggregate_type() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/pack.make

func main(): i32 {
    var outer = make()
    return outer.value
}
"#,
        &[(
            "std/pack.nct",
            r#"pub copy struct Inner {
    pub tag: i32
}

pub copy struct Outer {
    pub inner: Inner
    pub value: i32
}

pub func make(): Outer {
    return Outer {
        inner: Inner { tag: 7 },
        value: 42,
    }
}
"#,
        )],
    );
    let analysis = &fixture.analysis;
    let imported_source = analysis
        .files
        .iter()
        .find(|file| {
            !file.is_root
                && file.ast.items.iter().any(|item| {
                    matches!(item, crate::ast::Item::Function(function) if function.name == "make")
                })
        })
        .map(|file| file.ast.span.source)
        .unwrap();

    let ir = lower_executable(analysis, &fixture.sources).unwrap();
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(main.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::Imported { source, name },
                layout,
                ..
            } if *source == imported_source
                && name == "make"
                && *layout == ValueLayout::new(8, 4)
        )
    }));
    assert!(main.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::LoadAggregateI32 {
                destination: I32Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 4,
            }
        )
    }));
}

#[test]
fn indexes_indirect_aggregate_function_signature_return_type() {
    let analysis = analyze_text(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func make(): Text {
    return Text { start: 0, len: 0, capacity: 0 }
}
"#,
    );
    let root = analysis.root_file().unwrap();
    let index = FunctionIndex::new(&analysis, root.ast.span.source);
    let signatures = index.signatures();

    assert_eq!(
        signatures.return_type(&CallTarget::same_file("make")),
        Some(&Type::Aggregate {
            layout: ValueLayout::new(24, 8),
        })
    );
    assert_eq!(
        signatures.success_return_passing(&CallTarget::same_file("make")),
        Some(ReturnPassing::IndirectPointer)
    );
}

#[test]
fn indexes_direct_aggregate_function_signature_return_type() {
    let analysis = analyze_text(
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
"#,
    );
    let root = analysis.root_file().unwrap();
    let index = FunctionIndex::new(&analysis, root.ast.span.source);
    let signatures = index.signatures();

    assert_eq!(
        signatures.return_type(&CallTarget::same_file("page_allocator")),
        Some(&Type::DirectAggregate {
            layout: ValueLayout::new(16, 8),
            words: 2,
        })
    );
    assert_eq!(
        signatures.success_return_passing(&CallTarget::same_file("page_allocator")),
        Some(ReturnPassing::Direct { words: 2 })
    );
}

#[test]
fn indexes_aggregate_function_signature_parameter_types() {
    let analysis = analyze_text(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func consume(text: Text, header: Header): i32 {
    return 0
}
"#,
    );
    let root = analysis.root_file().unwrap();
    let index = FunctionIndex::new(&analysis, root.ast.span.source);
    let signatures = index.signatures();

    assert_eq!(
        signatures.parameter_types(&CallTarget::same_file("consume")),
        Some(
            vec![
                Type::Aggregate {
                    layout: ValueLayout::new(24, 8),
                },
                Type::DirectAggregate {
                    layout: ValueLayout::new(16, 8),
                    words: 2,
                },
            ]
            .as_slice()
        )
    );
    assert_eq!(
        signatures.parameter_abi_word_count(&CallTarget::same_file("consume")),
        Some(3)
    );
}

#[test]
fn lowers_indirect_aggregate_value_parameter_field_return() {
    let function = lower_named_function(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func length(text: Text): usize {
    return text.len
}
"#,
        "length",
    );

    assert_eq!(
        function,
        Function {
            name: "length".to_string(),
            target: CallTarget::same_file("length"),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::Parameter(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::LoadAggregateUsize {
                    destination: UsizeLocation::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 8,
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_direct_aggregate_value_parameter_field_return() {
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

func code(header: Header): i32 {
    return header.code
}
"#,
        "code",
    );

    assert_eq!(
        function,
        Function {
            name: "code".to_string(),
            target: CallTarget::same_file("code"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    layout: ValueLayout::new(16, 8),
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
fn lowers_small_direct_aggregate_value_parameter_field_return() {
    let function = lower_named_function(
        r#"struct Code {
    value: i32
}

func main(): i32 {
    return 0
}

func read(code: Code): i32 {
    return code.value
}
"#,
        "read",
    );

    assert_eq!(
        function,
        Function {
            name: "read".to_string(),
            target: CallTarget::same_file("read"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 0,
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_two_byte_direct_aggregate_value_parameter_field_return() {
    let function = lower_named_function(
        r#"struct Bytes {
    first: u8
    second: u8
}

func main(): i32 {
    return 0
}

func read(bytes: Bytes): u8 {
    return bytes.second
}
"#,
        "read",
    );

    assert_eq!(
        function,
        Function {
            name: "read".to_string(),
            target: CallTarget::same_file("read"),
            return_type: Type::U8,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(2, 1),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    layout: ValueLayout::new(2, 1),
                },
                Instruction::LoadAggregateU8 {
                    destination: U8Location::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 1,
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_three_byte_direct_aggregate_value_parameter_field_return() {
    let function = lower_named_function(
        r#"struct Bytes {
    first: u8
    second: u8
    third: u8
}

func main(): i32 {
    return 0
}

func read(bytes: Bytes): u8 {
    return bytes.third
}
"#,
        "read",
    );

    assert_eq!(
        function,
        Function {
            name: "read".to_string(),
            target: CallTarget::same_file("read"),
            return_type: Type::U8,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(3, 1),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    layout: ValueLayout::new(3, 1),
                },
                Instruction::LoadAggregateU8 {
                    destination: U8Location::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 2,
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_borrowed_aggregate_parameter_field_return() {
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

func code(header: &Header): i32 {
    return header.code
}
"#,
        "code",
    );

    assert_eq!(
        function,
        Function {
            name: "code".to_string(),
            target: CallTarget::same_file("code"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::Parameter(0),
                    offset: 4,
                },
                Instruction::Return,
            ],
        }
    );
}

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
                    destination: AggregateLocation::Parameter(0),
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
                        destination: AggregateLocation::Parameter(0),
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
                        destination: AggregateLocation::Parameter(0),
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
                    destination: UsizeLocation::Local(0),
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateIndirect(AggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                    })],
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: usize_local(0),
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
fn suppresses_scope_end_drop_for_moved_aggregate_return() {
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
    var file = make_file()
    drop file
    return 0
}

func make_file(): File {
    var file = File { fd: 3 }
    return move file
}
"#,
    );

    let make_file = ir
        .functions
        .iter()
        .find(|function| function.name == "make_file")
        .unwrap();
    assert_eq!(
        make_file.instructions,
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
            Instruction::CopyAggregate {
                destination: AggregateLocation::DirectReturn,
                source: AggregateLocation::Slot(0),
                layout: ValueLayout::new(4, 4),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_copy_aggregate_binding_from_copy_local() {
    let function = lower_named_function(
        r#"copy struct Pair {
    left: i32
    right: i32
}

func main(): i32 {
    return 0
}

func use_pair(): i32 {
    let source = Pair { left: 40, right: 2 }
    let target = source
    return target.left + target.right
}
"#,
        "use_pair",
    );

    assert_eq!(
        function,
        Function {
            name: "use_pair".to_string(),
            target: CallTarget::same_file("use_pair"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: i32_const(40),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: i32_const(2),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(1),
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Local(0),
                    source: AggregateLocation::Slot(1),
                    offset: 0,
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Local(1),
                    source: AggregateLocation::Slot(1),
                    offset: 4,
                },
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: i32_local(0),
                    right: i32_local(1),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn suppresses_scope_end_drop_for_moved_aggregate_binding() {
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
    let source = File { fd: 3 }
    let target = move source
    return target.fd
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
                value: i32_const(3),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(1),
                source: AggregateLocation::Slot(0),
                layout: ValueLayout::new(4, 4),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Local(0),
                source: AggregateLocation::Slot(1),
                offset: 0,
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("File.drop"),
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
fn lowers_nonterminal_if_branch_aggregate_slots_with_distinct_layouts() {
    let ir = lower_text(
        r#"struct Small {
    value: i32
}

impl Small {
    drop &+self {
        return
    }
}

struct Wide {
    left: i32
    right: i32
}

impl Wide {
    drop &+self {
        return
    }
}

func main(): i32 {
    if true {
        var small = Small { value: 1 }
    } else {
        var wide = Wide { left: 2, right: 3 }
    }
    return 0
}
"#,
    );

    let small_drop = Instruction::CallVoid {
        target: CallTarget::same_file("Small.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let wide_drop = Instruction::CallVoid {
        target: CallTarget::same_file("Wide.drop"),
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
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(1),
                    },
                    small_drop,
                ],
                else_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(8, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(1),
                        offset: 0,
                        value: i32_const(2),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(1),
                        offset: 4,
                        value: i32_const(3),
                    },
                    wide_drop,
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_aggregate_move_binding_inside_nonterminal_if_branch_before_return() {
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
    if true {
        var moved = move file
        return moved.fd
    }
    return 0
}
"#,
    );

    let drop_original = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_moved = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
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
                value: i32_const(3),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(1),
                        source: AggregateLocation::Slot(0),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Local(0),
                        source: AggregateLocation::Slot(1),
                        offset: 0,
                    },
                    drop_moved,
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_original,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_aggregate_move_binding_inside_nonterminal_if_branch_before_return_suffix() {
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
    if true {
        var moved = move file
        touch()
        return moved.fd
    }
    return 0
}

func touch(): void {
    return
}
"#,
    );

    let drop_original = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_moved = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
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
                value: i32_const(3),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(1),
                        source: AggregateLocation::Slot(0),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("touch"),
                        arguments: vec![],
                    },
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Local(0),
                        source: AggregateLocation::Slot(1),
                        offset: 0,
                    },
                    drop_moved,
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_original,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_aggregate_assignment_inside_nonterminal_if_branch_before_return_suffix() {
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
                        destination: I32Location::Local(0),
                        source: AggregateLocation::Slot(0),
                        offset: 0,
                    },
                    drop_file.clone(),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_file,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_aggregate_move_assignment_inside_nonterminal_if_branch_before_return_suffix() {
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
    var target = File { fd: 1 }
    var source = File { fd: 2 }
    if true {
        target = move source
        touch()
        return target.fd
    }
    return 0
}

func touch(): void {
    return
}
"#,
    );

    let drop_target = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_source = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
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
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 2,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(2),
                        source: AggregateLocation::Slot(1),
                        layout: ValueLayout::new(4, 4),
                    },
                    drop_target.clone(),
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(0),
                        source: AggregateLocation::Slot(2),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("touch"),
                        arguments: vec![],
                    },
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Local(0),
                        source: AggregateLocation::Slot(0),
                        offset: 0,
                    },
                    drop_target.clone(),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_source,
            drop_target,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_aggregate_move_assignment_inside_nonterminal_if_branch_before_nested_return_if() {
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
    var target = File { fd: 1 }
    var source = File { fd: 2 }
    if true {
        target = move source
        if choose() {
            return target.fd
        } else {
            return 7
        }
    }
    return 0
}

func choose(): bool {
    return true
}
"#,
    );

    let drop_target = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_source = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
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
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 2,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(2),
                        source: AggregateLocation::Slot(1),
                        layout: ValueLayout::new(4, 4),
                    },
                    drop_target.clone(),
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(0),
                        source: AggregateLocation::Slot(2),
                        layout: ValueLayout::new(4, 4),
                    },
                    call_bool(BoolLocation::Local(0), "choose", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![
                            Instruction::LoadAggregateI32 {
                                destination: I32Location::Local(0),
                                source: AggregateLocation::Slot(0),
                                offset: 0,
                            },
                            drop_target.clone(),
                            Instruction::SetI32 {
                                destination: I32Location::Return,
                                value: i32_local(0),
                            },
                            Instruction::Return,
                        ],
                        else_instructions: vec![
                            Instruction::SetI32 {
                                destination: I32Location::Local(0),
                                value: i32_const(7),
                            },
                            drop_target.clone(),
                            Instruction::SetI32 {
                                destination: I32Location::Return,
                                value: i32_local(0),
                            },
                            Instruction::Return,
                        ],
                    },
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_source,
            drop_target,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_aggregate_move_assignment_inside_nonterminal_if_branch_before_never_suffix() {
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
    var target = File { fd: 1 }
    var source = File { fd: 2 }
    if true {
        target = move source
        abort()
    }
    return 0
}

func abort(): never {
    abort()
}
"#,
    );

    let drop_target = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_source = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
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
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 2,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(2),
                        source: AggregateLocation::Slot(1),
                        layout: ValueLayout::new(4, 4),
                    },
                    drop_target.clone(),
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(0),
                        source: AggregateLocation::Slot(2),
                        layout: ValueLayout::new(4, 4),
                    },
                    drop_target.clone(),
                    Instruction::TailCall {
                        target: CallTarget::same_file("abort"),
                        arguments: vec![],
                    },
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_source,
            drop_target,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_aggregate_terminal_if_never_branch_with_scope_cleanup() {
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
    var file = make()
    return file.fd
}

func make(): File {
    if true {
        var temp = File { fd: 2 }
        abort()
    } else {
        return File { fd: 1 }
    }
}

func abort(): never {
    abort()
}
"#,
    );

    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "make")
        .unwrap();
    assert_eq!(
        function,
        &Function {
            name: "make".to_string(),
            target: CallTarget::same_file("make"),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(4, 4),
                words: 1,
            },
            instructions: vec![Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(2),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    Instruction::TailCall {
                        target: CallTarget::same_file("abort"),
                        arguments: vec![],
                    },
                ],
                else_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(1),
                        offset: 0,
                        value: i32_const(1),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::DirectReturn,
                        source: AggregateLocation::Slot(1),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::Return,
                ],
            }],
        }
    );
}

#[test]
fn lowers_branch_local_aggregate_move_assignment_from_outer_before_return_suffix() {
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
    var source = File { fd: 2 }
    if true {
        var target = File { fd: 1 }
        target = move source
        touch()
        return target.fd
    }
    return source.fd
}

func touch(): void {
    return
}
"#,
    );

    let drop_source = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_target = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
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
                value: i32_const(2),
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
                        value: i32_const(1),
                    },
                    Instruction::ReserveAggregateSlot {
                        slot_index: 2,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(2),
                        source: AggregateLocation::Slot(0),
                        layout: ValueLayout::new(4, 4),
                    },
                    drop_target.clone(),
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(1),
                        source: AggregateLocation::Slot(2),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("touch"),
                        arguments: vec![],
                    },
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Local(0),
                        source: AggregateLocation::Slot(1),
                        offset: 0,
                    },
                    drop_target,
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![],
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Local(0),
                source: AggregateLocation::Slot(0),
                offset: 0,
            },
            drop_source,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_local_aggregate_move_inside_nonterminal_if_branch() {
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
    if true {
        var file = File { fd: 1 }
        var moved = move file
    }
    return 0
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
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
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
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(1),
                        source: AggregateLocation::Slot(0),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(1),
                        })],
                    },
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_assignment_to_nonterminal_while_body_local_aggregate_with_replacement_drop() {
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
    while false {
        var file = File { fd: 1 }
        file = File { fd: 2 }
    }
    return 0
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
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::Const(false),
                body_instructions: vec![
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
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(0),
                        source: AggregateLocation::Slot(1),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
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

impl File {
    drop &+self {
        return
    }
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
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_file,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
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

impl File {
    drop &+self {
        return
    }
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
            destination: I32Location::Local(0),
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

impl File {
    drop &+self {
        return
    }
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
fn lowers_explicit_aggregate_move_in_terminal_if_condition() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func consume(file: File): bool {
    return true
}

func main(): i32 {
    var file = File { fd: 1 }
    if consume(move file) {
        return 0
    } else {
        return 1
    }
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
                value: i32_const(1),
            },
            call_bool(
                BoolLocation::Local(0),
                "consume",
                vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(4, 4),
                    words: 1,
                })],
            ),
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_const(0),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_const(1),
                    },
                    Instruction::Return,
                ],
            },
        ],
    );
}

#[test]
fn lowers_explicit_aggregate_move_in_terminal_bool_if_condition() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func consume(file: File): bool {
    return true
}

func pick(): bool {
    var file = File { fd: 1 }
    if consume(move file) {
        return true
    } else {
        return false
    }
}

func main(): i32 {
    if pick() {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let pick = ir
        .functions
        .iter()
        .find(|function| function.name == "pick")
        .unwrap();
    assert_eq!(
        pick.instructions,
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
            call_bool(
                BoolLocation::Local(0),
                "consume",
                vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(4, 4),
                    words: 1,
                })],
            ),
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![
                    Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: BoolValue::Const(true),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![
                    Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: BoolValue::Const(false),
                    },
                    Instruction::Return,
                ],
            },
        ],
    );
}

#[test]
fn transfers_scope_end_drop_to_by_value_aggregate_parameter() {
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
    consume(move file)
    return 0
}

func consume(file: File): void {
    return
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
                value: i32_const(3),
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("consume"),
                arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(4, 4),
                    words: 1,
                })],
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    );

    let consume = ir
        .functions
        .iter()
        .find(|function| function.name == "consume")
        .unwrap();
    assert_eq!(
        consume.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::DirectParameter { start_index: 0 },
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("File.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
                })],
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn suppresses_scope_end_drop_for_moved_aggregate_tail_return_argument() {
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
    return consume(move file)
}

func consume(file: File): i32 {
    return file.fd
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
                value: i32_const(3),
            },
            Instruction::TailCall {
                target: CallTarget::same_file("consume"),
                arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(4, 4),
                    words: 1,
                })],
            },
        ],
    );

    let consume = ir
        .functions
        .iter()
        .find(|function| function.name == "consume")
        .unwrap();
    assert_eq!(
        consume.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::DirectParameter { start_index: 0 },
                layout: ValueLayout::new(4, 4),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Local(0),
                source: AggregateLocation::Slot(0),
                offset: 0,
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("File.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
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
fn lowers_aggregate_reinitialization_after_explicit_drop_without_replacement_drop() {
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
    drop file
    file = File { fd: 42 }
    return file.fd
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
            drop_call.clone(),
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(42),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Local(0),
                source: AggregateLocation::Slot(0),
                offset: 0,
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
fn lowers_replacement_drop_for_moved_aggregate_assignment() {
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
    var source = File { fd: 1 }
    var target = File { fd: 2 }
    target = move source
    return 0
}
"#,
    );

    let drop_target = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
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
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(2),
                source: AggregateLocation::Slot(0),
                layout: ValueLayout::new(4, 4),
            },
            drop_target.clone(),
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(1),
                source: AggregateLocation::Slot(2),
                layout: ValueLayout::new(4, 4),
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_target,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_scope_end_drop_after_staged_aggregate_field_return() {
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
    var file = File { fd: 1 }
    file = File { fd: 42 }
    return file.fd
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
                value: i32_const(42),
            },
            drop_call.clone(),
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::Slot(1),
                layout: ValueLayout::new(4, 4),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Local(0),
                source: AggregateLocation::Slot(0),
                offset: 0,
            },
            drop_call,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::ReturnFallibleSuccess,
        ],
    );
}

#[test]
fn lowers_direct_aggregate_borrow_parameter_signature() {
    let function = lower_named_function(
        r#"struct Allocator {
    state: usize
    kind: usize
}

func main(): i32 {
    return 0
}

func touch(allocator: &+Allocator): void {
    return
}
"#,
        "touch",
    );

    assert_eq!(
        function,
        Function {
            name: "touch".to_string(),
            target: CallTarget::same_file("touch"),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        }
    );
}

#[test]
fn lowers_direct_aggregate_terminal_if_call_return_after_scope_drop() {
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

func make_pair(first: usize, second: usize): Pair {
    return Pair { first: first, second: second }
}

func choose(flag: bool): Pair {
    var file = File { fd: 3 }
    if flag {
        return make_pair(1, 2)
    } else {
        return make_pair(3, 4)
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
                        Instruction::CallDirectAggregate {
                            destination: AggregateLocation::Slot(1),
                            target: CallTarget::same_file("make_pair"),
                            arguments: vec![
                                ScalarArgument::Usize(usize_const(1)),
                                ScalarArgument::Usize(usize_const(2)),
                            ],
                            layout: pair_layout,
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
                        Instruction::CallDirectAggregate {
                            destination: AggregateLocation::Slot(2),
                            target: CallTarget::same_file("make_pair"),
                            arguments: vec![
                                ScalarArgument::Usize(usize_const(3)),
                                ScalarArgument::Usize(usize_const(4)),
                            ],
                            layout: pair_layout,
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
fn lowers_direct_aggregate_terminal_if_moved_local_return_after_scope_drop() {
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
    let left = Pair { first: 1, second: 2 }
    let right = Pair { first: 3, second: 4 }
    if flag {
        return move left
    } else {
        return move right
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
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Parameter(0)),
                    then_instructions: vec![
                        Instruction::ReserveAggregateSlot {
                            slot_index: 3,
                            layout: pair_layout,
                        },
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::Slot(3),
                            source: AggregateLocation::Slot(1),
                            layout: pair_layout,
                        },
                        drop_call.clone(),
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::DirectReturn,
                            source: AggregateLocation::Slot(3),
                            layout: pair_layout,
                        },
                        Instruction::Return,
                    ],
                    else_instructions: vec![
                        Instruction::ReserveAggregateSlot {
                            slot_index: 4,
                            layout: pair_layout,
                        },
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::Slot(4),
                            source: AggregateLocation::Slot(2),
                            layout: pair_layout,
                        },
                        drop_call,
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::DirectReturn,
                            source: AggregateLocation::Slot(4),
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
fn lowers_direct_aggregate_terminal_if_leading_drop_and_void_call_before_return() {
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
        drop file
        return Pair { first: 1, second: 2 }
    } else {
        touch(&+file)
        return Pair { first: 3, second: 4 }
    }
}

func touch(file: &+File): void {
    return
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let touch_call = Instruction::CallVoid {
        target: CallTarget::same_file("touch"),
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
                        drop_call.clone(),
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
                        touch_call,
                        Instruction::ReserveAggregateSlot {
                            slot_index: 1,
                            layout: pair_layout,
                        },
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::Slot(1),
                            offset: 0,
                            value: usize_const(3),
                        },
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::Slot(1),
                            offset: 8,
                            value: usize_const(4),
                        },
                        drop_call,
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::DirectReturn,
                            source: AggregateLocation::Slot(1),
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
fn lowers_direct_aggregate_terminal_if_branch_local_binding_drop_before_return() {
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
    if flag {
        var file = File { fd: 1 }
        return Pair { first: 1, second: 2 }
    } else {
        var file = File { fd: 2 }
        return Pair { first: 3, second: 4 }
    }
}
"#,
    );

    let then_drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let else_drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(2),
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
            instructions: vec![Instruction::If {
                condition: BoolValue::Location(BoolLocation::Parameter(0)),
                then_instructions: vec![
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
                    then_drop_call,
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
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(2),
                        offset: 0,
                        value: i32_const(2),
                    },
                    Instruction::ReserveAggregateSlot {
                        slot_index: 3,
                        layout: pair_layout,
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Slot(3),
                        offset: 0,
                        value: usize_const(3),
                    },
                    Instruction::StoreAggregateUsize {
                        destination: AggregateLocation::Slot(3),
                        offset: 8,
                        value: usize_const(4),
                    },
                    else_drop_call,
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::DirectReturn,
                        source: AggregateLocation::Slot(3),
                        layout: pair_layout,
                    },
                    Instruction::Return,
                ],
            }],
        }
    );
}

#[test]
fn lowers_direct_aggregate_terminal_if_branch_assignment_before_moved_return() {
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
    var file = choose(true)
    drop file
    return 0
}

func choose(flag: bool): File {
    var file = File { fd: 1 }
    if flag {
        file = File { fd: 2 }
        return move file
    } else {
        file = File { fd: 3 }
        return move file
    }
}
"#,
    );

    let layout = ValueLayout::new(4, 4);
    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
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
            return_type: Type::DirectAggregate { layout, words: 1 },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: i32_const(1),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Parameter(0)),
                    then_instructions: vec![
                        Instruction::ReserveAggregateSlot {
                            slot_index: 1,
                            layout,
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
                            layout,
                        },
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::DirectReturn,
                            source: AggregateLocation::Slot(0),
                            layout,
                        },
                        Instruction::Return,
                    ],
                    else_instructions: vec![
                        Instruction::ReserveAggregateSlot {
                            slot_index: 2,
                            layout,
                        },
                        Instruction::StoreAggregateI32 {
                            destination: AggregateLocation::Slot(2),
                            offset: 0,
                            value: i32_const(3),
                        },
                        drop_call,
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::Slot(0),
                            source: AggregateLocation::Slot(2),
                            layout,
                        },
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::DirectReturn,
                            source: AggregateLocation::Slot(0),
                            layout,
                        },
                        Instruction::Return,
                    ],
                },
            ],
        }
    );
}

#[test]
fn lowers_concrete_generic_aggregate_value_parameter_field_return() {
    let function = lower_named_function(
        r#"struct Box<T> {
    value: T
}

func main(): i32 {
    return 0
}

func read(box: Box<i32>): i32 {
    return box.value
}
"#,
        "read",
    );

    assert_eq!(
        function,
        Function {
            name: "read".to_string(),
            target: CallTarget::same_file("read"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 0,
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
                Instruction::CallDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("reset_allocator"),
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
                set_return_i32(0),
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_aggregate_i32_field_return_from_local_slot() {
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

func read_code(): i32 {
    let value = Header { tag: 7, ok: true, code: 42, len: 11 }
    return value.code
}
"#,
        "read_code",
    );

    assert_eq!(
        function,
        Function {
            name: "read_code".to_string(),
            target: CallTarget::same_file("read_code"),
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
fn lowers_nested_aggregate_i32_field_return_from_local_slot() {
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

func read_code(): i32 {
    let packet = Packet {
        prefix: 1,
        header: Header { tag: 7, ok: true, code: 42, len: 11 },
        tail: 99,
    }
    return packet.header.code
}
"#,
        "read_code",
    );

    assert_eq!(
        function,
        Function {
            name: "read_code".to_string(),
            target: CallTarget::same_file("read_code"),
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
                    value: usize_const(11),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 24,
                    value: usize_const(99),
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

impl File {
    drop &+self {
        return
    }
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

impl File {
    drop &+self {
        return
    }
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
            source: BorrowSource::AggregateParameterField {
                parameter_index: 0,
                offset: 4,
            },
        })],
    }));
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Parameter(0),
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

impl File {
    drop &+self {
        return
    }
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

impl File {
    drop &+self {
        return
    }
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
                destination: AggregateLocation::Slot(2),
                target: CallTarget::same_file("make_file"),
                arguments: vec![],
                layout: ValueLayout::new(4, 4),
            })
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 0,
                source: AggregateLocation::Slot(2),
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
fn lowers_nested_borrowed_aggregate_parameter_field_return() {
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

func read_code(packet: &Packet): i32 {
    return packet.header.code
}
"#,
        "read_code",
    );

    assert_eq!(
        function,
        Function {
            name: "read_code".to_string(),
            target: CallTarget::same_file("read_code"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::Parameter(0),
                    offset: 12,
                },
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

#[test]
fn lowers_aggregate_scalar_field_reads_as_expression_operands() {
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

func read_next_code(): i32 {
    let value = Header { tag: 7, ok: true, code: 42, len: 11 }
    return value.code + 1
}
"#,
        "read_next_code",
    );

    assert_eq!(
        function,
        Function {
            name: "read_next_code".to_string(),
            target: CallTarget::same_file("read_next_code"),
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
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Local(0),
                    source: AggregateLocation::Slot(0),
                    offset: 4,
                },
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: i32_local(0),
                    right: i32_const(1),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_aggregate_field_return_from_call_binding_slot() {
    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(16, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func make(): Header {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}

func read_code(): i32 {
    let value = make()
    return value.code
}
"#,
        "read_code",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "read_code".to_string(),
            target: CallTarget::same_file("read_code"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
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
fn lowers_aggregate_field_return_from_direct_call_result_slot() {
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

func main(): i32 {
    return 0
}

func make(): Header {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}

func read_code(): i32 {
    return make().code
}
"#,
        "read_code",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "read_code".to_string(),
            target: CallTarget::same_file("read_code"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 8),
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
fn lowers_nested_aggregate_field_binding() {
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

func read_code(): i32 {
    let packet = Packet { prefix: 1, header: Header { tag: 7, ok: true, code: 42, len: 11 }, tail: 2 }
    let header = packet.header
    return header.code
}
"#,
        "read_code",
    );

    assert_eq!(
        function,
        Function {
            name: "read_code".to_string(),
            target: CallTarget::same_file("read_code"),
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
                    value: usize_const(11),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 24,
                    value: usize_const(2),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Slot(1),
                    destination_offset: 0,
                    source: AggregateLocation::Slot(0),
                    source_offset: 8,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::Slot(1),
                    offset: 4,
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_copy_aggregate_field_binding_from_non_copy_owner() {
    let function = lower_named_function(
        r#"copy struct Header {
    code: i32
    len: i32
}

struct Packet {
    prefix: i32
    header: Header
    tail: i32
}

func main(): i32 {
    return 0
}

func read_code(): i32 {
    let packet = Packet { prefix: 1, header: Header { code: 40, len: 2 }, tail: 3 }
    let header = packet.header
    return header.code + header.len
}
"#,
        "read_code",
    );

    assert_eq!(
        function,
        Function {
            name: "read_code".to_string(),
            target: CallTarget::same_file("read_code"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: i32_const(1),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: i32_const(40),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: i32_const(2),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 12,
                    value: i32_const(3),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Slot(1),
                    destination_offset: 0,
                    source: AggregateLocation::Slot(0),
                    source_offset: 4,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Local(0),
                    source: AggregateLocation::Slot(1),
                    offset: 0,
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Local(1),
                    source: AggregateLocation::Slot(1),
                    offset: 4,
                },
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: i32_local(0),
                    right: i32_local(1),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_copy_aggregate_field_binding_from_non_copy_call_result() {
    let packet_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 4),
        words: 2,
    };
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

func make_packet(): Packet {
    return Packet { prefix: 1, header: Header { code: 40, len: 2 }, tail: 3 }
}

func main(): i32 {
    return 0
}

func read_code(): i32 {
    let header = make_packet().header
    let again = header
    return again.code + again.len
}
"#,
        "read_code",
        function_signatures(vec![("make_packet", packet_type, vec![])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "read_code".to_string(),
            target: CallTarget::same_file("read_code"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(16, 4),
                },
                Instruction::CallDirectAggregate {
                    destination: AggregateLocation::Slot(1),
                    target: CallTarget::same_file("make_packet"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 4),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Slot(0),
                    destination_offset: 0,
                    source: AggregateLocation::Slot(1),
                    source_offset: 4,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 2,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(2),
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Local(0),
                    source: AggregateLocation::Slot(2),
                    offset: 0,
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Local(1),
                    source: AggregateLocation::Slot(2),
                    offset: 4,
                },
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: i32_local(0),
                    right: i32_local(1),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_nested_aggregate_field_value_argument() {
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

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func consume(header: Header): i32 {
    return header.code
}

func main(): i32 {
    let packet = Packet { prefix: 1, header: Header { tag: 7, ok: true, code: 42, len: 11 }, tail: 2 }
    let result = consume(packet.header)
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
                    value: usize_const(11),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 24,
                    value: usize_const(2),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Slot(1),
                    destination_offset: 0,
                    source: AggregateLocation::Slot(0),
                    source_offset: 8,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                        source: AggregateArgumentSource::Slot(1),
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
fn lowers_nested_aggregate_field_return() {
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

func pick(packet: Packet): Header {
    return packet.header
}
"#,
        "pick",
    );

    assert_eq!(
        function,
        Function {
            name: "pick".to_string(),
            target: CallTarget::same_file("pick"),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(16, 8),
                words: 2,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(32, 8),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::Parameter(0),
                    layout: ValueLayout::new(32, 8),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::DirectReturn,
                    destination_offset: 0,
                    source: AggregateLocation::Slot(0),
                    source_offset: 8,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::Return,
            ],
        }
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
fn lowers_nested_aggregate_field_binding_from_call_result() {
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
    return Packet { prefix: 1, header: Header { tag: 7, ok: true, code: 42, len: 11 }, tail: 2 }
}

func read_code(): i32 {
    let header = make().header
    return header.code
}
"#,
        "read_code",
        function_signatures(vec![("make", packet_type, vec![])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "read_code".to_string(),
            target: CallTarget::same_file("read_code"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(32, 8),
                },
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(1),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Slot(0),
                    destination_offset: 0,
                    source: AggregateLocation::Slot(1),
                    source_offset: 8,
                    layout: ValueLayout::new(16, 8),
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
fn lowers_nested_aggregate_field_value_argument_from_call_result() {
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

func make(): Packet {
    return Packet { prefix: 1, header: Header { tag: 7, ok: true, code: 42, len: 11 }, tail: 2 }
}

func consume(header: Header): i32 {
    return header.code
}

func main(): i32 {
    let result = consume(make().header)
    return result
}
"#,
        "main",
        function_signatures(vec![
            ("make", packet_type, vec![]),
            ("consume", Type::I32, vec![header_type]),
        ]),
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
                    layout: ValueLayout::new(32, 8),
                },
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Slot(1),
                    destination_offset: 0,
                    source: AggregateLocation::Slot(0),
                    source_offset: 8,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                        source: AggregateArgumentSource::Slot(1),
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
fn lowers_nested_aggregate_field_return_from_call_result() {
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
    return Packet { prefix: 1, header: Header { tag: 7, ok: true, code: 42, len: 11 }, tail: 2 }
}

func pick(): Header {
    return make().header
}
"#,
        "pick",
        function_signatures(vec![("make", packet_type, vec![])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "pick".to_string(),
            target: CallTarget::same_file("pick"),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(16, 8),
                words: 2,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(32, 8),
                },
                Instruction::CallAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::DirectReturn,
                    destination_offset: 0,
                    source: AggregateLocation::Slot(0),
                    source_offset: 8,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::Return,
            ],
        }
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
fn lowers_aggregate_field_reads_in_comparisons() {
    let text = r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func code_is_answer(): bool {
    let value = Header { tag: 7, ok: true, code: 42, len: 11 }
    return value.code == 42
}

func ok_is_true(): bool {
    let value = Header { tag: 7, ok: true, code: 42, len: 11 }
    return value.ok == true
}
"#;

    let code = lower_named_function(text, "code_is_answer");
    let ok = lower_named_function(text, "ok_is_true");

    assert!(
        code.instructions.contains(&Instruction::LoadAggregateI32 {
            destination: I32Location::Local(0),
            source: AggregateLocation::Slot(0),
            offset: 4,
        }),
        "{code:?}"
    );
    assert!(
        code.instructions.contains(&Instruction::SetBool {
            destination: BoolLocation::Return,
            value: BoolValue::I32Comparison {
                operator: I32ComparisonOperator::Equal,
                left: i32_local(0),
                right: i32_const(42),
            },
        }),
        "{code:?}"
    );
    assert!(
        ok.instructions.contains(&Instruction::LoadAggregateBool {
            destination: BoolLocation::Local(0),
            source: AggregateLocation::Slot(0),
            offset: 1,
        }),
        "{ok:?}"
    );
    assert!(
        ok.instructions.contains(&Instruction::SetBool {
            destination: BoolLocation::Return,
            value: BoolValue::BoolComparison {
                operator: BoolComparisonOperator::Equal,
                left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                right: Box::new(BoolValue::Const(true)),
            },
        }),
        "{ok:?}"
    );
}

#[test]
fn lowers_aggregate_field_reads_in_short_circuit_comparison_condition() {
    let ir = lower_text(
        r#"struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let value = Header { tag: 7, ok: true, code: 42, len: 11 }
    if value.code == 42 && value.len == 11 {
        return 42
    } else {
        return 1
    }
}
"#,
    );

    let main = &ir.functions[0];
    assert!(
        main.instructions.contains(&Instruction::LoadAggregateI32 {
            destination: I32Location::Local(0),
            source: AggregateLocation::Slot(0),
            offset: 4,
        }),
        "{main:?}"
    );
    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::If {
                    condition: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left,
                        right,
                    },
                    then_instructions,
                    ..
                } if left == &i32_local(0)
                    && right == &i32_const(42)
                    && then_instructions.contains(&Instruction::LoadAggregateUsize {
                        destination: UsizeLocation::Local(0),
                        source: AggregateLocation::Slot(0),
                        offset: 8,
                    })
                    && then_instructions.iter().any(|then_instruction| matches!(
                        then_instruction,
                        Instruction::If {
                            condition: BoolValue::UsizeComparison {
                                operator: I32ComparisonOperator::Equal,
                                left,
                                right,
                            },
                            ..
                        } if left == &UsizeValue::Location(UsizeLocation::Local(0))
                            && right == &UsizeValue::Const(11)
                    ))
            )
        }),
        "{main:?}"
    );
}

#[test]
fn lowers_aggregate_call_field_read_in_comparison() {
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

func main(): i32 {
    return 0
}

func make(): Header {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}

func code_is_answer(): bool {
    return make().code == 42
}
"#,
        "code_is_answer",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "code_is_answer".to_string(),
            target: CallTarget::same_file("code_is_answer"),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Local(0),
                    source: AggregateLocation::Slot(0),
                    offset: 4,
                },
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: i32_local(0),
                        right: i32_const(42),
                    },
                },
                Instruction::Return,
            ],
        }
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
                Instruction::StoreAggregateU16 {
                    destination: AggregateLocation::Slot(0),
                    offset: 2,
                    value: 8,
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: i32_const(42),
                },
                Instruction::StoreAggregateU32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: 10,
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
                Instruction::StoreAggregateU16 {
                    destination: AggregateLocation::Slot(0),
                    offset: 2,
                    value: 12,
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: i32_const(99),
                },
                Instruction::StoreAggregateU32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: 100,
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
fn lowers_function_with_stack_passed_parameter_word() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func consume(a: &str, b: &str, c: &str, d: &str, e: usize): usize {
    return e
}
"#,
        "consume",
    );

    assert_eq!(
        function,
        Function {
            name: "consume".to_string(),
            target: CallTarget::same_file("consume"),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: usize_param(8),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_call_with_stack_passed_argument_word_as_normal_return_call() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func run(): usize {
    return consume(1, 2, 3, 4, 5, 6, 7, 8, 9)
}

func consume(
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: usize,
    g: usize,
    h: usize,
    i: usize,
): usize {
    return i
}
"#,
        "run",
        function_signatures(vec![(
            "consume",
            Type::Usize,
            vec![
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
            ],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "run".to_string(),
            target: CallTarget::same_file("run"),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::CallUsize {
                    destination: UsizeLocation::Return,
                    target: CallTarget::same_file("consume"),
                    arguments: vec![
                        ScalarArgument::Usize(usize_const(1)),
                        ScalarArgument::Usize(usize_const(2)),
                        ScalarArgument::Usize(usize_const(3)),
                        ScalarArgument::Usize(usize_const(4)),
                        ScalarArgument::Usize(usize_const(5)),
                        ScalarArgument::Usize(usize_const(6)),
                        ScalarArgument::Usize(usize_const(7)),
                        ScalarArgument::Usize(usize_const(8)),
                        ScalarArgument::Usize(usize_const(9)),
                    ],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_stack_passed_never_call_as_normal_call_then_trap() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return abort(1, 2, 3, 4, 5, 6, 7, 8, 9)
}

func abort(
    a: i32,
    b: i32,
    c: i32,
    d: i32,
    e: i32,
    f: i32,
    g: i32,
    h: i32,
    i: i32,
): never {
    abort(a, b, c, d, e, f, g, h, i)
}
"#,
        "main",
        function_signatures(vec![(
            "abort",
            Type::Never,
            vec![
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
            ],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("abort"),
                    arguments: vec![
                        ScalarArgument::I32(i32_const(1)),
                        ScalarArgument::I32(i32_const(2)),
                        ScalarArgument::I32(i32_const(3)),
                        ScalarArgument::I32(i32_const(4)),
                        ScalarArgument::I32(i32_const(5)),
                        ScalarArgument::I32(i32_const(6)),
                        ScalarArgument::I32(i32_const(7)),
                        ScalarArgument::I32(i32_const(8)),
                        ScalarArgument::I32(i32_const(9)),
                    ],
                },
                Instruction::Trap,
            ],
        }
    );
}

#[test]
fn lowers_split_register_stack_direct_aggregate_call_argument() {
    let pair_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 4),
        words: 2,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    return consume(1, 2, 3, 4, 5, 6, 7, Pair { a: 1, b: 2, c: 3, d: 4 })
}

func consume(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, pair: Pair): i32 {
    return pair.c
}
"#,
        "main",
        function_signatures(vec![(
            "consume",
            Type::I32,
            vec![
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                pair_type,
            ],
        )]),
    )
    .unwrap();

    assert!(
        function.instructions.contains(&Instruction::CallI32 {
            destination: I32Location::Return,
            target: CallTarget::same_file("consume"),
            arguments: vec![
                ScalarArgument::I32(i32_const(1)),
                ScalarArgument::I32(i32_const(2)),
                ScalarArgument::I32(i32_const(3)),
                ScalarArgument::I32(i32_const(4)),
                ScalarArgument::I32(i32_const(5)),
                ScalarArgument::I32(i32_const(6)),
                ScalarArgument::I32(i32_const(7)),
                ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(16, 4),
                    words: 2,
                }),
            ],
        }),
        "{function:?}"
    );
}

#[test]
fn lowers_aggregate_scalar_field_borrow_call_argument() {
    let ir = lower_text(
        r#"copy struct Pair {
    value: i32
}

func main(): i32 {
    let pair = Pair { value: 1 }
    return choose(&pair.value, 42)
}

func choose(value: &i32, code: i32): i32 {
    return code
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
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: I32Value::Const(1),
            },
            Instruction::CallI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("choose"),
                arguments: vec![
                    ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlotField {
                            slot_index: 0,
                            offset: 0,
                        },
                    }),
                    ScalarArgument::I32(I32Value::Const(42)),
                ],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_aggregate_call_scalar_field_borrow_call_argument() {
    let ir = lower_text(
        r#"copy struct Pair {
    value: i32
}

func main(): i32 {
    return choose(&make().value, 42)
}

func make(): Pair {
    return Pair { value: 1 }
}

func choose(value: &i32, code: i32): i32 {
    return code
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
            Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CallI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("choose"),
                arguments: vec![
                    ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlotField {
                            slot_index: 0,
                            offset: 0,
                        },
                    }),
                    ScalarArgument::I32(I32Value::Const(42)),
                ],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_borrowed_aggregate_scalar_field_borrow_call_argument() {
    let ir = lower_text(
        r#"copy struct Pair {
    value: i32
}

func main(): i32 {
    let pair = Pair { value: 1 }
    return caller(&pair)
}

func caller(pair: &Pair): i32 {
    return choose(&pair.value, 42)
}

func choose(value: &i32, code: i32): i32 {
    return code
}
"#,
    );
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "caller")
        .unwrap();

    assert_eq!(
        function.instructions,
        vec![
            Instruction::CallI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("choose"),
                arguments: vec![
                    ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateParameterField {
                            parameter_index: 0,
                            offset: 0,
                        },
                    }),
                    ScalarArgument::I32(I32Value::Const(42)),
                ],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_readwrite_aggregate_scalar_field_borrow_call_argument() {
    let ir = lower_text(
        r#"copy struct Pair {
    value: i32
}

func main(): i32 {
    var pair = Pair { value: 1 }
    return choose(&+pair.value, 42)
}

func choose(value: &+i32, code: i32): i32 {
    return code
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
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: I32Value::Const(1),
            },
            Instruction::CallI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("choose"),
                arguments: vec![
                    ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlotField {
                            slot_index: 0,
                            offset: 0,
                        },
                    }),
                    ScalarArgument::I32(I32Value::Const(42)),
                ],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_readwrite_borrowed_aggregate_scalar_field_borrow_call_argument() {
    let ir = lower_text(
        r#"copy struct Pair {
    value: i32
}

func main(): i32 {
    var pair = Pair { value: 1 }
    return caller(&+pair)
}

func caller(pair: &+Pair): i32 {
    return choose(&+pair.value, 42)
}

func choose(value: &+i32, code: i32): i32 {
    return code
}
"#,
    );
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "caller")
        .unwrap();

    assert_eq!(
        function.instructions,
        vec![
            Instruction::CallI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("choose"),
                arguments: vec![
                    ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateParameterField {
                            parameter_index: 0,
                            offset: 0,
                        },
                    }),
                    ScalarArgument::I32(I32Value::Const(42)),
                ],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_ignored_direct_aggregate_call_expression_statement_with_drop() {
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
    make()
    return 0
}

func make(): File {
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
            Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(4, 4),
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
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_ignored_aggregate_literal_expression_statement() {
    let ir = lower_text(
        r#"struct Value {
    code: i32
}

func main(): i32 {
    Value { code: 1 }
    return 0
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
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_ignored_aggregate_literal_expression_statement_with_drop() {
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
    File { fd: 1 }
    return 0
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
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
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
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_ignored_alias_aggregate_call_expression_statement_with_drop() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

type Handle = File

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    make()
    return 0
}

func make(): Handle {
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
            Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(4, 4),
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
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_ignored_indirect_aggregate_call_expression_statement() {
    let ir = lower_text(
        r#"copy struct Big {
    a: usize
    b: usize
    c: usize
}

func main(): i32 {
    make()
    return 0
}

func make(): Big {
    return Big { a: 1, b: 2, c: 3 }
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(24, 8),
            },
            Instruction::CallAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ]
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
                destination: I32Location::Local(1),
                left: i32_local(1),
                right: i32_local(0),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 4,
                value: i32_local(1),
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
                destination: UsizeLocation::Local(0),
                left: usize_local(0),
                right: usize_const(5),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(0),
                offset: 8,
                value: usize_local(0),
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
