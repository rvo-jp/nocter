use super::*;

#[test]
fn lowers_ignored_fallible_str_catch_statement_with_reserved_error_locals() {
    let ir = lower_text(
        r#"func main(): i32 {
    text() catch failure {
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
                Instruction::CallOutcomeStr {
                    destination: StrLocation::Local(0),
                    target: CallTarget::same_file("text"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Catch {
                        code: StrLocation::Local(2),
                        message: StrLocation::Local(4),
                        instructions: vec![
                            Instruction::SetI32 {
                                destination: I32Location::Return,
                                value: i32_const(7),
                            },
                            Instruction::Return,
                        ],
                        recovers: false,
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
fn lowers_fallible_void_function_static_error_failure() {
    let ir = lower_text_with_std_error(
        r#"func main(): void! {
    fail()?
}

func fail(): void! {
    return error.new("app.inner", "inner failed")
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
        r#"func main(): void! {
    fail()?
}

func fail(): void! {
    return app_failed()
}

func app_failed(): error {
    return error.new("app.failed", "failed")
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
        r#"func main(): i32! {
    let value = answer() catch failure {
        return error.new("app.answer", failure.message)
    }
    return value
}

func answer(): i32! {
    return error.new("app.inner", "inner failed")
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
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Catch {
                        code: StrLocation::Local(1),
                        message: StrLocation::Local(3),
                        instructions: vec![Instruction::ReturnFallibleFailure {
                            code: StrValue::StaticBytes(b"app.answer".to_vec()),
                            message: StrValue::Location(StrLocation::Local(3)),
                        }],
                        recovers: false,
                    },
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::Location(I32Location::Local(0)),
                },
                Instruction::ReturnOutcomeSuccess,
            ],
        }
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
                "std/io_catch/index.nct",
                r#"use std/io.write_text_raw

pub func print_catch(text: &str): void! {
    write_text_raw(1, text) catch failure {
        return error.new("app.write", failure.message)
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
                failure_mode: OutcomeFailureMode::Catch {
                    code: StrLocation::Local(0),
                    message: StrLocation::Local(2),
                    instructions: vec![Instruction::ReturnFallibleFailure {
                        code: StrValue::StaticBytes(b"app.write".to_vec()),
                        message: StrValue::Location(StrLocation::Local(2)),
                    }],
                    recovers: false,
                },
            },
            Instruction::ReturnOutcomeSuccess,
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
                "std/io_bytes/index.nct",
                r#"use std/io.write_bytes_raw

pub func write_bytes_catch(bytes: &[u8]): void! {
    write_bytes_raw(1, bytes) catch failure {
        return error.new("app.write", failure.message)
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
                failure_mode: OutcomeFailureMode::Catch {
                    code: StrLocation::Local(0),
                    message: StrLocation::Local(2),
                    instructions: vec![Instruction::ReturnFallibleFailure {
                        code: StrValue::StaticBytes(b"app.write".to_vec()),
                        message: StrValue::Location(StrLocation::Local(2)),
                    }],
                    recovers: false,
                },
            },
            Instruction::ReturnOutcomeSuccess,
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
                "std/io_bytes_catch/index.nct",
                r#"use std/io.read_bytes_raw

pub func read_count_catch(buffer: &+[u8]): usize! {
    let count = read_bytes_raw(0, buffer) catch failure {
        return error.new("app.read", failure.message)
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
                failure_mode: OutcomeFailureMode::Catch {
                    code: StrLocation::Local(1),
                    message: StrLocation::Local(3),
                    instructions: vec![Instruction::ReturnFallibleFailure {
                        code: StrValue::StaticBytes(b"app.read".to_vec()),
                        message: StrValue::Location(StrLocation::Local(3)),
                    }],
                    recovers: false,
                },
            },
            Instruction::SetUsize {
                destination: UsizeLocation::Return,
                value: UsizeValue::Location(UsizeLocation::Local(0)),
            },
            Instruction::ReturnOutcomeSuccess,
        ]
    );
}

#[test]
fn lowers_fallible_entry_return_static_error_constructor() {
    let ir = lower_text_with_std_error(
        r#"func main(): i32! {
    return error.new("app.failed", "failed")
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
        r#"func main(): i32! {
    return error.new("app.failed", dynamic())
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
        r#"func main(): i32! {
    let value = error.new("app.failed", dynamic())
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
        r#"func main(): i32! {
    return forward(error.new("app.failed", "failed"))?
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
            Instruction::CallOutcomeI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("forward"),
                arguments: vec![
                    ScalarArgument::Str(StrValue::Location(StrLocation::Local(0))),
                    ScalarArgument::Str(StrValue::Location(StrLocation::Local(2))),
                ],
                failure_mode: OutcomeFailureMode::Propagate,
            },
            Instruction::ReturnOutcomeSuccess,
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
        r#"func main(): i32! {
    return error.new(dynamic_code(), dynamic_message())
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
        r#"func main(): i32! {
    return error.new("app.failed", """
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
        r#"func main(): i32! {
    return error.new("app.failed", "failed\n")
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
        r#"func main(): i32! {
    let value = answer() catch failure {
        return failure
    }
    return value
}

func answer(): i32! {
    return error.new("app.inner", "inner failed")
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
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Catch {
                        code: StrLocation::Local(1),
                        message: StrLocation::Local(3),
                        instructions: vec![Instruction::ReturnFallibleFailure {
                            code: StrValue::Location(StrLocation::Local(1)),
                            message: StrValue::Location(StrLocation::Local(3)),
                        }],
                        recovers: false,
                    },
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::Location(I32Location::Local(0)),
                },
                Instruction::ReturnOutcomeSuccess,
            ],
        }
    );
}
