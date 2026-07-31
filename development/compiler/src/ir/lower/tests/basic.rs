use super::*;

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
            "std/process.nct",
            r#"#target("arm64-darwin")
pub(nocter) primitive arg_count_raw(): usize

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
            "std/process.nct",
            r#"#target("arm64-darwin")
pub(nocter) primitive arg_raw(index: usize): &str

pub func arg_probe(index: usize): &str {
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
