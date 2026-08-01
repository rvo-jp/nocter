use super::*;

#[test]
fn lowers_scalar_alias_parameter_and_return() {
    let function = lower_named_function(
        r#"type Exit = i32

func main(): i32 {
    return answer(42)
}

func answer(value: Exit): Exit {
    return value
}
"#,
        "answer",
    );

    assert_eq!(
        function,
        Function {
            name: "answer".to_string(),
            target: crate::ir::CallTarget::same_file("answer".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_param(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn indexes_alias_parameter_and_return_signatures_for_calls() {
    let ir = lower_text(
        r#"type Exit = i32
type Text = str

func main(): i32 {
    return wrapper("Nocter")
}

func wrapper(name: &Text): Exit {
    return consume(name, 42)
}

func consume(name: &Text, code: Exit): Exit {
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
fn lowers_imported_i32_normal_call() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/math.answer

func main(): i32 {
    let value = answer()
    return value
}
"#,
        &[(
            "std/math.nct",
            r#"pub func answer(): i32 {
    return 42
}
"#,
        )],
    );
    let analysis = &fixture.analysis;
    let root = analysis.root_file().unwrap();
    let imported_source = analysis
        .files
        .iter()
        .find(|file| {
            !file.is_root
                && file.ast.items.iter().any(|item| {
                    matches!(item, crate::ast::Item::Function(function) if function.name == "answer")
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
                instructions: vec![
                    Instruction::CallI32 {
                        destination: I32Location::Local(0),
                        target: CallTarget::imported(imported_source, "answer"),
                        arguments: vec![],
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                target: CallTarget::imported(imported_source, "answer"),
                return_type: Type::I32,
                instructions: vec![set_return_i32(42), Instruction::Return],
            },
        ])
    );
    assert_ne!(root.ast.span.source, imported_source);
}

#[test]
fn lowers_imported_i32_associated_function_normal_call() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/point.Point

func main(): i32 {
    let value = Point.origin()
    return value
}
"#,
        &[(
            "std/point.nct",
            r#"pub struct Point {
    x: i32
}

pub func Point.origin(): i32 {
    return 42
}
"#,
        )],
    );
    let analysis = &fixture.analysis;
    let root = analysis.root_file().unwrap();
    let imported_source = analysis
        .files
        .iter()
        .find(|file| {
            !file.is_root
                && file.ast.items.iter().any(|item| {
                    matches!(item, crate::ast::Item::Function(function) if function.name == "Point.origin")
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
                instructions: vec![
                    Instruction::CallI32 {
                        destination: I32Location::Local(0),
                        target: CallTarget::imported(imported_source, "Point.origin"),
                        arguments: vec![],
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "Point.origin".to_string(),
                target: CallTarget::imported(imported_source, "Point.origin"),
                return_type: Type::I32,
                instructions: vec![set_return_i32(42), Instruction::Return],
            },
        ])
    );
    assert_ne!(root.ast.span.source, imported_source);
}

#[test]
fn lowers_imported_bool_normal_call_in_terminal_if_condition() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/flags.ready

func main(): i32 {
    if ready() {
        return 42
    } else {
        return 1
    }
}
"#,
        &[(
            "std/flags.nct",
            r#"pub func ready(): bool {
    return true
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
                    matches!(item, crate::ast::Item::Function(function) if function.name == "ready")
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
                instructions: vec![
                    Instruction::CallBool {
                        destination: BoolLocation::Local(0),
                        target: CallTarget::imported(imported_source, "ready"),
                        arguments: vec![],
                    },
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(42), Instruction::Return],
                        else_instructions: vec![set_return_i32(1), Instruction::Return],
                    },
                ],
            },
            Function {
                name: "ready".to_string(),
                target: CallTarget::imported(imported_source, "ready"),
                return_type: Type::Bool,
                instructions: vec![
                    Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: BoolValue::Const(true),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn imported_alias_call_uses_imported_declaration_name_as_target() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/math.answer as imported_answer

func main(): i32 {
    return imported_answer()
}
"#,
        &[(
            "std/math.nct",
            r#"pub func answer(): i32 {
    return 42
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
                    matches!(item, crate::ast::Item::Function(function) if function.name == "answer")
                })
        })
        .map(|file| file.ast.span.source)
        .unwrap();

    let ir = lower_executable(analysis, &fixture.sources).unwrap();

    assert_eq!(
        ir.functions
            .iter()
            .map(|function| function.target.clone())
            .collect::<Vec<_>>(),
        vec![
            CallTarget::same_file("main"),
            CallTarget::imported(imported_source, "answer"),
        ]
    );
    assert!(matches!(
        &ir.functions[0].instructions[0],
        Instruction::TailCall {
            target: CallTarget::Imported { source, name },
            ..
        } if *source == imported_source && name == "answer"
    ));
}

#[test]
fn collects_loaded_imported_call_targets() {
    let analysis = analyze_text_with_nocter_home_files(
        r#"use std/math.answer

func main(): i32 {
    let value = answer()
    return value
}
"#,
        &[(
            "std/math.nct",
            r#"pub func answer(): i32 {
    return 42
}
"#,
        )],
    );
    let root = analysis.root_file().unwrap();
    let entry = root
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            crate::ast::Item::Function(function)
                if function.name == crate::entry::DEFAULT_ENTRY_NAME =>
            {
                Some(function)
            }
            _ => None,
        })
        .unwrap();

    let targets =
        super::imported_calls::imported_call_targets(entry, root.ast.span.source, &root.resolved);

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].call_name, "answer");
    assert!(targets[0].span.start < targets[0].span.end);
    assert!(matches!(
        targets[0].source,
        super::imported_calls::ImportedCallSource::Loaded(source)
            if source != root.ast.span.source
    ));
}

#[test]
fn indexes_imported_function_signatures_by_call_target() {
    let analysis = analyze_text_with_nocter_home_files(
        r#"use std/math.answer

func main(): i32 {
    let value = answer()
    return value
}
"#,
        &[(
            "std/math.nct",
            r#"pub func answer(): i32 {
    return 42
}
"#,
        )],
    );
    let root = analysis.root_file().unwrap();
    let imported_source = analysis
        .files
        .iter()
        .find(|file| {
            !file.is_root
                && file.ast.items.iter().any(|item| {
                    matches!(item, crate::ast::Item::Function(function) if function.name == "answer")
                })
        })
        .map(|file| file.ast.span.source)
        .unwrap();

    let index = FunctionIndex::new(&analysis, root.ast.span.source);
    let signatures = index.signatures();

    assert_eq!(
        signatures.return_type(&CallTarget::imported(imported_source, "answer")),
        Some(&Type::I32)
    );
}

#[test]
fn lowers_imported_i32_call_target_when_boundary_is_bypassed() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/math.answer

func main(): i32 {
    let value = answer()
    return value
}
"#,
        &[(
            "std/math.nct",
            r#"pub func answer(): i32 {
    return 42
}
"#,
        )],
    );
    let analysis = &fixture.analysis;
    let root = analysis.root_file().unwrap();
    let imported_source = analysis
        .files
        .iter()
        .find(|file| {
            !file.is_root
                && file.ast.items.iter().any(|item| {
                    matches!(item, crate::ast::Item::Function(function) if function.name == "answer")
                })
        })
        .map(|file| file.ast.span.source)
        .unwrap();
    let entry = root
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            crate::ast::Item::Function(function)
                if function.name == crate::entry::DEFAULT_ENTRY_NAME =>
            {
                Some(function)
            }
            _ => None,
        })
        .unwrap();
    let index = FunctionIndex::new(analysis, root.ast.span.source);

    let function = entry::lower_entry_function(
        entry,
        &fixture.sources,
        index.signatures(),
        index.names(),
        root.ast.span.source,
        &root.resolved,
        &root.typecheck_facts,
        index.resolved_sources(),
        index.error_payloads(root.ast.span.source),
    )
    .unwrap();

    assert!(matches!(
        &function.instructions[0],
        Instruction::CallI32 {
            target: CallTarget::Imported { source, name },
            ..
        } if *source == imported_source && name == "answer"
    ));
}

#[test]
fn lowers_entry_scalar_alias_annotated_let_binding_then_return() {
    let ir = lower_text(
        r#"type Exit = i32

func main(): i32 {
    let value: Exit = 42
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
fn lowers_entry_scalar_alias_return_type() {
    let ir = lower_text(
        r#"type Exit = i32

func main(): Exit {
    return 42
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(42), Instruction::Return],
        }])
    );
}
