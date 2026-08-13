use super::*;

#[test]
fn lowers_zero_parameter_scalar_helpers_through_the_common_mir_route() {
    let ir = lower_text(
        r#"func helper(): i32 {
    let base = 40
    return base + 2
}

func main(): i32 {
    return helper()
}
"#,
    );
    let helper = ir
        .functions
        .iter()
        .find(|function| function.name == "helper")
        .unwrap();

    assert_eq!(
        helper.instructions,
        vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: I32Value::Const(40),
            },
            Instruction::AddI32 {
                destination: I32Location::Return,
                left: I32Value::Location(I32Location::Local(0)),
                right: I32Value::Const(2),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_scalar_parameters_as_identity_backed_mir_places() {
    let ir = lower_text(
        r#"func helper(value: i32): i32 {
    return value + 2
}

func main(): i32 {
    return helper(40)
}
"#,
    );
    let helper = ir
        .functions
        .iter()
        .find(|function| function.name == "helper")
        .unwrap();

    assert_eq!(
        helper.instructions,
        vec![
            Instruction::AddI32 {
                destination: I32Location::Return,
                left: I32Value::Location(I32Location::Parameter(0)),
                right: I32Value::Const(2),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_scalar_assignment_as_a_mir_place_update() {
    let ir = lower_text(
        r#"func helper(): i32 {
    var value = 40
    value = value + 2
    return value
}

func main(): i32 {
    return helper()
}
"#,
    );
    let helper = ir
        .functions
        .iter()
        .find(|function| function.name == "helper")
        .unwrap();

    assert_eq!(
        helper.instructions,
        vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: I32Value::Const(40),
            },
            Instruction::AddI32 {
                destination: I32Location::Local(0),
                left: I32Value::Location(I32Location::Local(0)),
                right: I32Value::Const(2),
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: I32Value::Location(I32Location::Local(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_bool_parameters_and_results_through_scalar_mir() {
    let ir = lower_text(
        r#"func helper(value: bool): bool {
    var result = value
    result = false
    return result
}

func main(): i32 {
    let value = helper(true)
    return 0
}
"#,
    );
    let helper = ir
        .functions
        .iter()
        .find(|function| function.name == "helper")
        .unwrap();

    assert_eq!(
        helper.instructions,
        vec![
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::Location(BoolLocation::Parameter(0)),
            },
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::Const(false),
            },
            Instruction::SetBool {
                destination: BoolLocation::Return,
                value: BoolValue::Location(BoolLocation::Local(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_process_exit_to_target_exit_primitive() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/process.exit

func main(): i32 {
    return exit(7)
}
"#,
        &[std_process_file(), std_os_file()],
    );
    let analysis = &fixture.analysis;
    let process_source = analysis
        .files
        .iter()
        .find(|file| {
            !file.is_root
                && file.ast.items.iter().any(|item| {
                    matches!(item, crate::ast::Item::Function(function) if function.name == "exit")
                })
        })
        .map(|file| file.ast.span.source)
        .unwrap();

    let ir = lower_executable(analysis, &fixture.sources).unwrap();

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: CallTarget::same_file("main"),
                return_type: Type::I32,
                instructions: vec![Instruction::TailCall {
                    target: CallTarget::imported(process_source, "exit"),
                    arguments: vec![ScalarArgument::I32(I32Value::Const(7))],
                }],
            },
            Function {
                name: "exit".to_string(),
                target: CallTarget::imported(process_source, "exit"),
                return_type: Type::Never,
                instructions: vec![Instruction::ProcessExit {
                    code: I32Value::Location(I32Location::Parameter(0)),
                }],
            },
        ])
    );
}

#[test]
fn lowers_process_arg_count_primitive_to_ir_value() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/process.arg_count_probe

func main(): i32 {
    let count: usize = arg_count_probe()
    return 0
}
"#,
        &[(
            "std/process/index.nct",
            r#"#target: "arm64-darwin"
pub(/) primitive arg_count_raw(): usize

pub func arg_count_probe(): usize {
    return arg_count_raw()
}
"#,
        )],
    );
    let ir = lower_executable(&fixture.analysis, &fixture.sources).unwrap();
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "arg_count_probe")
        .unwrap();

    assert!(matches!(
        &function.instructions[0],
        Instruction::SetUsize {
            destination: UsizeLocation::Return,
            value: UsizeValue::ProcessArgCount,
        }
    ));
}

#[test]
fn lowers_process_arg_primitive_to_ir_value() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/process.arg_probe

func main(): i32 {
    let first = arg_probe(0)
    return 0
}

"#,
        &[(
            "std/process/index.nct",
            r#"#target: "arm64-darwin"
pub(/) primitive arg_raw(index: usize): &str from static

pub func arg_probe(index: usize): &str from static {
    return arg_raw(index)
}
"#,
        )],
    );
    let ir = lower_executable(&fixture.analysis, &fixture.sources).unwrap();
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "arg_probe")
        .unwrap();

    assert!(matches!(
        &function.instructions[0],
        Instruction::SetStr {
            destination: StrLocation::Return,
            value: StrValue::ProcessArg {
                index: UsizeValue::Location(UsizeLocation::Parameter(0)),
            },
        }
    ));
}

#[test]
fn lowers_process_environment_primitives_to_structural_ir_values() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/process.{env_count_probe, env_name_probe, env_value_probe}

func main(): i32 {
    let count = env_count_probe()
    let name = env_name_probe(0)
    let value = env_value_probe(0)
    return 0
}
"#,
        &[(
            "std/process/index.nct",
            r#"#target: "arm64-darwin"
pub(/) primitive env_count_raw(): usize
#target: "arm64-darwin"
pub(/) primitive env_name_raw(index: usize): &str from static
#target: "arm64-darwin"
pub(/) primitive env_value_raw(index: usize): &str from static

pub func env_count_probe(): usize { return env_count_raw() }
pub func env_name_probe(index: usize): &str from static { return env_name_raw(index) }
pub func env_value_probe(index: usize): &str from static { return env_value_raw(index) }
"#,
        )],
    );
    let ir = lower_executable(&fixture.analysis, &fixture.sources).unwrap();

    let count = ir
        .functions
        .iter()
        .find(|function| function.name == "env_count_probe")
        .unwrap();
    assert!(matches!(
        &count.instructions[0],
        Instruction::SetUsize {
            destination: UsizeLocation::Return,
            value: UsizeValue::ProcessEnvironmentCount,
        }
    ));

    for (function_name, expected_name) in [("env_name_probe", true), ("env_value_probe", false)] {
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == function_name)
            .unwrap();
        let Instruction::SetStr {
            destination: StrLocation::Return,
            value,
        } = &function.instructions[0]
        else {
            panic!("{function:?}");
        };
        assert_eq!(
            matches!(value, StrValue::ProcessEnvironmentName { .. }),
            expected_name
        );
        assert_eq!(
            matches!(value, StrValue::ProcessEnvironmentValue { .. }),
            !expected_name
        );
    }
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        }])
    );
}

#[test]
fn lowers_void_entry_with_binding_before_implicit_return() {
    let ir = lower_text(
        r#"func main(): void {
    let value = 1
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(1),
                },
                Instruction::Return,
            ],
        }])
    );
}
