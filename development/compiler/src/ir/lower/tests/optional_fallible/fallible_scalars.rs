use super::*;

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
            then_instructions: vec![Instruction::ReturnOutcomeSuccess],
            else_instructions: vec![Instruction::ReturnOutcomeSuccess],
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
                then_instructions: vec![Instruction::ReturnOutcomeSuccess],
                else_instructions: vec![Instruction::ReturnOutcomeSuccess],
            }],
            else_instructions: vec![Instruction::ReturnOutcomeSuccess],
        }],
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
                Instruction::CallOutcomeI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("value"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Propagate,
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_const(0),
                },
                Instruction::ReturnOutcomeSuccess,
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
                Instruction::CallOutcomeStr {
                    destination: StrLocation::Local(0),
                    target: CallTarget::same_file("text"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Trap,
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
                Instruction::CallOutcomeSlice {
                    destination: SliceLocation::Local(0),
                    target: CallTarget::same_file("maybe_bytes"),
                    arguments: vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                    failure_mode: OutcomeFailureMode::Propagate,
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_const(0),
                },
                Instruction::ReturnOutcomeSuccess,
            ],
        }
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
            instructions: vec![set_return_i32(7), Instruction::ReturnOutcomeSuccess],
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
            instructions: vec![set_return_i32(7), Instruction::ReturnOutcomeSuccess],
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
            instructions: vec![Instruction::ReturnOutcomeSuccess],
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
            instructions: vec![set_return_i32(42), Instruction::ReturnOutcomeSuccess],
        }
    );
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
                Instruction::CallOutcomeI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Propagate,
                },
                Instruction::ReturnOutcomeSuccess,
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
                Instruction::ReturnOutcomeSuccess,
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
                Instruction::CallOutcomeI32 {
                    destination: I32Location::Local(1),
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Propagate,
                },
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: I32Value::Location(I32Location::Local(0)),
                    right: I32Value::Location(I32Location::Local(1)),
                },
                Instruction::ReturnOutcomeSuccess,
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
        Instruction::CallOutcomeU8 {
            destination: U8Location::Local(0),
            ..
        }
    ));
    assert!(matches!(
        main.instructions[1],
        Instruction::CallOutcomeUsize {
            destination: UsizeLocation::Local(1),
            ..
        }
    ));
    assert!(matches!(
        main.instructions[2],
        Instruction::CallOutcomeBool {
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
        Instruction::CallOutcomeStr {
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
        Instruction::CallOutcomeSlice {
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
                Instruction::CallOutcomeStr {
                    destination: StrLocation::Local(0),
                    target: CallTarget::same_file("make_text"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Propagate,
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::StrLen(StrLocation::Local(0)),
                },
                Instruction::ReturnOutcomeSuccess,
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
                Instruction::CallOutcomeSlice {
                    destination: SliceLocation::Local(0),
                    target: CallTarget::same_file("maybe_bytes"),
                    arguments: vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                    failure_mode: OutcomeFailureMode::Propagate,
                },
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Local(0),
                        index: usize_const(0),
                    },
                },
                Instruction::ReturnOutcomeSuccess,
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
                Instruction::CallOutcomeI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Trap,
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
        Instruction::CallOutcomeVoid { failure_mode, .. },
        Instruction::Return,
    ] = ir.functions[0].instructions.as_slice()
    else {
        panic!(
            "unexpected main instructions: {:?}",
            ir.functions[0].instructions
        );
    };
    assert_eq!(*failure_mode, OutcomeFailureMode::Trap);
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
                failure_mode: OutcomeFailureMode::Propagate,
            },
            Instruction::ReturnOutcomeSuccess,
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
                failure_mode: OutcomeFailureMode::Propagate,
            },
            Instruction::ReturnOutcomeSuccess,
        ]
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
        Instruction::CallOutcomeVoid {
            target, arguments, ..
        },
        Instruction::ReturnOutcomeSuccess,
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
            Instruction::ReturnOutcomeSuccess,
        ]
    );
}
