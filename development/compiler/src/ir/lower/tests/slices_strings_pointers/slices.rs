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
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(0),
                    value: usize_slice_index(SliceLocation::Parameter(0), usize_const(0)),
                },
                Instruction::If {
                    condition: BoolValue::UsizeComparison {
                        operator: I32ComparisonOperator::Equal,
                        left: UsizeValue::Location(UsizeLocation::Local(0)),
                        right: usize_const(42),
                    },
                    then_instructions: vec![set_return_i32(1), Instruction::Return],
                    else_instructions: vec![set_return_i32(2), Instruction::Return],
                },
            ],
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
                Instruction::SetU8 {
                    destination: U8Location::Local(2),
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Local(0),
                        index: usize_const(0),
                    },
                },
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(
                            U8Location::Local(2),
                        ))),
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
                Instruction::SetI32 {
                    destination: I32Location::Local(2),
                    value: I32Value::SliceIndex {
                        source: SliceLocation::Local(0),
                        index: usize_const(0),
                    },
                },
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::Location(I32Location::Local(2)),
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
                    destination: BoolLocation::Local(2),
                    value: BoolValue::SliceIndex {
                        source: SliceLocation::Local(0),
                        index: usize_const(0),
                    },
                },
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::BoolComparison {
                        operator: BoolComparisonOperator::Equal,
                        left: Box::new(BoolValue::Location(BoolLocation::Local(2))),
                        right: Box::new(BoolValue::Const(true)),
                    },
                },
                Instruction::Return,
            ],
        }
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
