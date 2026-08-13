use super::*;

#[test]
fn mir_projects_nested_struct_literal_leaf_paths_to_abi_offsets() {
    let function = lower_named_function(
        r#"copy struct Inner {
    value: i32
}

copy struct Outer {
    tag: u8
    inner: Inner
}

func main(): i32 {
    let outer = Outer { tag: 1, inner: Inner { value: 42 } }
    return outer.inner.value
}
"#,
        "main",
    );

    assert!(
        function
            .instructions
            .contains(&Instruction::StoreAggregateU8 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: u8_const(1),
            })
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 4,
                value: i32_const(42),
            })
    );
}

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

instance File {
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

instance File {
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
                    destination: I32Location::Local(1),
                    source: AggregateLocation::Slot(0),
                    offset: 4,
                },
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: i32_local(1),
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
            destination: I32Location::Local(1),
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
                left: i32_local(1),
                right: i32_const(42),
            },
        }),
        "{code:?}"
    );
    assert!(
        ok.instructions.contains(&Instruction::LoadAggregateBool {
            destination: BoolLocation::Local(1),
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
                left: Box::new(BoolValue::Location(BoolLocation::Local(1))),
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
