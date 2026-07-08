use super::*;
use crate::analysis::{CompileUnit, analyze_compile_unit_with_entry};
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::ir::{
    BoolLocation, BoolValue, Function, I32ComparisonOperator, I32Location, I32Value, Instruction,
    IrModule, Type,
};
use crate::source::SourceMap;
use crate::target::DEFAULT_TARGET;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn lowers_entry_returning_i32_literal() {
    let ir = lower_text(
        r#"func main(): i32 {
    return 42
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::I32,
            instructions: vec![set_return_i32(42), Instruction::Return],
        }])
    );
}

#[test]
fn lowers_entry_i32_let_binding_then_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 42
    return value
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(42),
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_local(0),
                },
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_entry_i32_annotated_let_binding_then_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value: i32 = 42
    return value
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(42),
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_entry_i32_local_addition_binding_then_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    let base = 40
    let result = base + 2
    return result
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(40),
                },
                Instruction::AddI32 {
                    destination: I32Location::Local(1),
                    left: i32_local(0),
                    right: i32_const(2),
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_local(1),
                },
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_entry_terminal_if_with_bool_literal_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    if false {
        return 1
    } else {
        return 2
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::I32,
            instructions: vec![Instruction::If {
                condition: BoolValue::Const(false),
                then_instructions: vec![set_return_i32(1), Instruction::Return],
                else_instructions: vec![set_return_i32(2), Instruction::Return],
            }],
        }])
    );
}

#[test]
fn lowers_entry_terminal_if_with_bool_local_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let enabled = true
    if enabled {
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
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(true),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions: vec![set_return_i32(0), Instruction::Return],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
            ],
        }])
    );
}

#[test]
fn lowers_entry_terminal_if_with_mixed_i32_and_bool_locals() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 42
    let enabled = true
    if enabled {
        return value
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
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(42),
                },
                Instruction::SetBool {
                    destination: BoolLocation::Local(1),
                    value: BoolValue::Const(true),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(1)),
                    then_instructions: vec![
                        Instruction::SetI32 {
                            destination: I32Location::Return,
                            value: i32_local(0),
                        },
                        Instruction::Return,
                    ],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
            ],
        }])
    );
}

#[test]
fn lowers_entry_terminal_if_returning_outer_local() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 42
    if true {
        return value
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(42),
                },
                Instruction::If {
                    condition: BoolValue::Const(true),
                    then_instructions: vec![
                        Instruction::SetI32 {
                            destination: I32Location::Return,
                            value: i32_local(0),
                        },
                        Instruction::Return,
                    ],
                    else_instructions: vec![set_return_i32(0), Instruction::Return],
                },
            ],
        }])
    );
}

#[test]
fn lowers_entry_terminal_if_with_i32_equality_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 42
    if value == 42 {
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
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(42),
                },
                Instruction::If {
                    condition: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: i32_local(0),
                        right: i32_const(42),
                    },
                    then_instructions: vec![set_return_i32(0), Instruction::Return],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
            ],
        }])
    );
}

#[test]
fn lowers_entry_terminal_if_with_i32_less_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 41
    if value < 42 {
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
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(41),
                },
                Instruction::If {
                    condition: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Less,
                        left: i32_local(0),
                        right: i32_const(42),
                    },
                    then_instructions: vec![set_return_i32(0), Instruction::Return],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
            ],
        }])
    );
}

#[test]
fn lowers_configured_entry_name() {
    let ir = lower_text_with_entry(
        r#"func start(): i32 {
    return 9
}

func main(): i32 {
    return 0
}
"#,
        "start",
    );

    assert_eq!(ir.functions[0].name, "start");
    assert_eq!(
        ir.functions[0].instructions,
        vec![set_return_i32(9), Instruction::Return]
    );
}

#[test]
fn lowers_entry_returning_negative_i32_literal() {
    let ir = lower_text(
        r#"func main(): i32 {
    return -42
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![set_return_i32(-42), Instruction::Return]
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
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![set_return_i32(7), Instruction::Return],
        }])
    );
}

#[test]
fn lowers_fallible_entry_fail_make_error() {
    let ir = lower_text(
        r#"primitive make_error(code: str, message: str): error

func main(): i32! {
    fail make_error("app.failed", "failed")
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::WriteStaticStderr(b"failed\n".to_vec()),
                set_return_i32(1),
                Instruction::Return,
            ],
        }])
    );
}

#[test]
fn lowers_fallible_entry_fail_message_without_duplicate_newline() {
    let ir = lower_text(
        r#"primitive make_error(code: str, message: str): error

func main(): i32! {
    fail make_error("app.failed", "failed\n")
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::WriteStaticStderr(b"failed\n".to_vec()),
            set_return_i32(1),
            Instruction::Return,
        ]
    );
}

#[test]
fn reports_unsupported_fail_payload() {
    let diagnostics = lower_text_diagnostics(
        r#"primitive make_error(code: str, message: str): error

func main(): i32! {
    fail make_error("app.failed", dynamic())
}

func dynamic(): str {
    return "failed"
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8004");
}

#[test]
fn lowers_void_entry_with_empty_body() {
    let ir = lower_text(
        r#"func main(): void {
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        }])
    );
}

#[test]
fn lowers_entry_returning_same_file_function_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    return 7
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_returning_i32_function_call_with_arguments() {
    let ir = lower_text(
        r#"func main(): i32 {
    return add(20, 22)
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("add", vec![i32_const(20), i32_const(22)])],
            },
            Function {
                name: "add".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_param(0),
                        right: i32_param(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_same_file_function_with_i32_let_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    let value = 7
    return value
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(7),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_same_file_function_with_i32_local_addition() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    let base = 40
    let result = base + 2
    return result
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(40),
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Local(1),
                        left: i32_local(0),
                        right: i32_const(2),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_same_file_function_with_terminal_if() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    if true {
        return 7
    } else {
        return 9
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                return_type: Type::I32,
                instructions: vec![Instruction::If {
                    condition: BoolValue::Const(true),
                    then_instructions: vec![set_return_i32(7), Instruction::Return],
                    else_instructions: vec![set_return_i32(9), Instruction::Return],
                }],
            },
        ])
    );
}

#[test]
fn lowers_same_file_function_with_i32_inequality_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    return differs(40, 2)
}

func differs(left: i32, right: i32): i32 {
    if left != right {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("differs", vec![i32_const(40), i32_const(2)])],
            },
            Function {
                name: "differs".to_string(),
                return_type: Type::I32,
                instructions: vec![Instruction::If {
                    condition: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::NotEqual,
                        left: i32_param(0),
                        right: i32_param(1),
                    },
                    then_instructions: vec![set_return_i32(1), Instruction::Return],
                    else_instructions: vec![set_return_i32(0), Instruction::Return],
                }],
            },
        ])
    );
}

#[test]
fn lowers_same_file_function_with_i32_greater_equal_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    return at_least(42, 40)
}

func at_least(left: i32, right: i32): i32 {
    if left >= right {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                return_type: Type::I32,
                instructions: vec![tail_call("at_least", vec![i32_const(42), i32_const(40)])],
            },
            Function {
                name: "at_least".to_string(),
                return_type: Type::I32,
                instructions: vec![Instruction::If {
                    condition: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::GreaterEqual,
                        left: i32_param(0),
                        right: i32_param(1),
                    },
                    then_instructions: vec![set_return_i32(1), Instruction::Return],
                    else_instructions: vec![set_return_i32(0), Instruction::Return],
                }],
            },
        ])
    );
}

#[test]
fn reports_unsupported_entry_body() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    use_value(1)
    return 1
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8002");
}

#[test]
fn rejects_nested_negative_integer_literal() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    return -(-42)
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8003");
}

fn lower_text(text: &str) -> IrModule {
    lower_text_with_entry(text, crate::entry::DEFAULT_ENTRY_NAME)
}

fn lower_text_with_entry(text: &str, entry_name: &str) -> IrModule {
    let diagnostics = lower_text_diagnostics_with_entry(text, entry_name);
    match diagnostics.as_slice() {
        [] => {
            let analysis = analyze_text_with_entry(text, entry_name);
            lower_executable_with_entry(&analysis, entry_name).unwrap()
        }
        diagnostics => panic!("unexpected diagnostics: {diagnostics:?}"),
    }
}

fn set_return_i32(value: i32) -> Instruction {
    Instruction::SetI32 {
        destination: I32Location::Return,
        value: i32_const(value),
    }
}

fn tail_call(function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::TailCall {
        function: function.to_string(),
        arguments,
    }
}

fn i32_const(value: i32) -> I32Value {
    I32Value::Const(value)
}

fn i32_param(index: usize) -> I32Value {
    I32Value::Location(I32Location::Parameter(index))
}

fn i32_local(index: usize) -> I32Value {
    I32Value::Location(I32Location::Local(index))
}

fn lower_text_diagnostics(text: &str) -> Vec<Diagnostic> {
    lower_text_diagnostics_with_entry(text, crate::entry::DEFAULT_ENTRY_NAME)
}

fn lower_text_diagnostics_with_entry(text: &str, entry_name: &str) -> Vec<Diagnostic> {
    let analysis = analyze_text_with_entry(text, entry_name);
    match lower_executable_with_entry(&analysis, entry_name) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

fn analyze_text_with_entry(text: &str, entry_name: &str) -> crate::analysis::CompileUnitAnalysis {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let temp_root = make_temp_project();
    let nocter_home = make_nocter_home(&temp_root);
    let unit: CompileUnit = load_compile_unit(
        &mut sources,
        source,
        &FrontendOptions {
            nocter_home: Some(nocter_home),
            target: DEFAULT_TARGET.to_string(),
        },
    )
    .unwrap();
    let analysis = analyze_compile_unit_with_entry(&sources, &unit, entry_name);
    let diagnostics = analysis.diagnostics();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    analysis
}

fn make_temp_project() -> PathBuf {
    let unique = format!(
        "nocter-ir-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).unwrap();
    root
}

fn make_nocter_home(root: &Path) -> PathBuf {
    let home = root.join(".nocter");
    fs::create_dir_all(home.join("std")).unwrap();
    fs::create_dir_all(home.join("targets/arm64-darwin/std")).unwrap();
    fs::write(home.join("std/prelude.nct"), "").unwrap();
    home
}
