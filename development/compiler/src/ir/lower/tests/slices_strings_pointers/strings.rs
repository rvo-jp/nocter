use super::*;

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
            instructions: vec![Instruction::TailCall {
                target: builtin_str_method_target("len"),
                arguments: vec![ScalarArgument::Str(StrValue::Location(
                    StrLocation::Parameter(0),
                ))],
            }],
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
            instructions: vec![Instruction::TailCall {
                target: builtin_str_method_target("len"),
                arguments: vec![ScalarArgument::Str(StrValue::StaticBytes(
                    b"Nocter".to_vec(),
                ))],
            }],
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
                Instruction::TailCall {
                    target: builtin_str_method_target("len"),
                    arguments: vec![ScalarArgument::Str(StrValue::Location(StrLocation::Local(
                        0
                    ),))],
                },
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
                Instruction::TailCall {
                    target: builtin_str_method_target("is_empty"),
                    arguments: vec![ScalarArgument::Str(StrValue::Location(StrLocation::Local(
                        0
                    ),))],
                },
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
                Instruction::CallBool {
                    destination: BoolLocation::Local(0),
                    target: builtin_str_method_target("is_empty"),
                    arguments: vec![ScalarArgument::Str(StrValue::Location(
                        StrLocation::Parameter(0),
                    ))],
                },
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::BoolComparison {
                        operator: BoolComparisonOperator::Equal,
                        left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                        right: Box::new(BoolValue::Const(false)),
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
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Local(0),
                    value: U8Value::StaticStrIndex {
                        bytes: b"Nocter".to_vec(),
                        index: usize_const(0),
                    },
                },
                Instruction::If {
                    condition: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(
                            U8Location::Local(0),
                        ))),
                        right: I32Value::U8ZeroExtend(Box::new(u8_const(78))),
                    },
                    then_instructions: vec![set_return_i32(0), Instruction::Return],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
            ],
        }])
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
        vec![Instruction::TailCall {
            target: builtin_slice_method_target("u8", "len"),
            arguments: vec![ScalarArgument::Slice(SliceValue::StrBytes(
                StrValue::Location(StrLocation::Parameter(0)),
            ))],
        }]
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
fn lowers_trusted_str_subview_projection_without_raw_pointer_ir() {
    let middle = lower_imported_named_function_with_nocter_home_files(
        r#"use std/string_views.middle

func main(): void {
    return
}
"#,
        "middle",
        &[(
            "std/string_views.nct",
            r#"pub(nocter) primitive str_subview_unchecked(
    text: &str,
    start: usize,
    len: usize,
): &str from text

pub func middle(text: &str, start: usize, end: usize): &str {
    return str_subview_unchecked(text, start, end - start)
}
"#,
        )],
    );

    assert_eq!(
        middle.instructions,
        vec![
            Instruction::SubtractUsize {
                destination: UsizeLocation::Local(0),
                left: UsizeValue::Location(UsizeLocation::Parameter(3)),
                right: UsizeValue::Location(UsizeLocation::Parameter(2)),
            },
            Instruction::SetStrSubview {
                destination: StrLocation::Return,
                source: StrValue::Location(StrLocation::Parameter(0)),
                start: UsizeValue::Location(UsizeLocation::Parameter(2)),
                len: UsizeValue::Location(UsizeLocation::Local(0)),
            },
            Instruction::Return,
        ]
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
