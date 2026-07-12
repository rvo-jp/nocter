use super::*;
use crate::abi::ValueLayout;
use crate::analysis::{CompileUnit, analyze_compile_unit_with_entry};
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::ir::{
    AggregateLocation, BoolComparisonOperator, BoolLocation, BoolLogicalOperator, BoolValue,
    BorrowArgument, BorrowSource, CallTarget, FallibleFailureMode, Function, I32ComparisonOperator,
    I32Location, I32Value, Instruction, IrModule, ScalarArgument, SliceLocation, SliceValue,
    StrLocation, StrValue, Type, U8Location, U8Value, UsizeLocation, UsizeValue,
};
use crate::source::SourceMap;
use crate::target::DEFAULT_TARGET;
use std::collections::HashMap;
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
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
fn lowers_entry_i32_let_initializer_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = answer()
    return value
}

func answer(): i32 {
    return 42
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
                    call_i32(I32Location::Local(0), "answer", vec![]),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(42), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_usize_let_binding_then_usize_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value: usize = 42
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(0),
                    value: usize_const(42),
                },
                Instruction::If {
                    condition: BoolValue::UsizeComparison {
                        operator: I32ComparisonOperator::Equal,
                        left: usize_local(0),
                        right: usize_const(42),
                    },
                    then_instructions: vec![set_return_i32(0), Instruction::Return],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
            ],
        }])
    );
}

#[test]
fn lowers_usize_returning_normal_call_in_let_initializer() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value: usize = size()
    if value >= 42 {
        return 0
    } else {
        return 1
    }
}

func size(): usize {
    return 42
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
                    call_usize(UsizeLocation::Local(0), "size", vec![]),
                    Instruction::If {
                        condition: BoolValue::UsizeComparison {
                            operator: I32ComparisonOperator::GreaterEqual,
                            left: usize_local(0),
                            right: usize_const(42),
                        },
                        then_instructions: vec![set_return_i32(0), Instruction::Return],
                        else_instructions: vec![set_return_i32(1), Instruction::Return],
                    },
                ],
            },
            Function {
                name: "size".to_string(),
                target: crate::ir::CallTarget::same_file("size".to_string()),
                return_type: Type::Usize,
                instructions: vec![set_return_usize(42), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_usize_parameter_normal_call_in_let_initializer() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value: usize = choose(7, 42)
    if value == 42 {
        return 0
    } else {
        return 1
    }
}

func choose(code: i32, value: usize): usize {
    return value
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
                    call_usize(
                        UsizeLocation::Local(0),
                        "choose",
                        vec![
                            ScalarArgument::I32(i32_const(7)),
                            ScalarArgument::Usize(usize_const(42)),
                        ],
                    ),
                    Instruction::If {
                        condition: BoolValue::UsizeComparison {
                            operator: I32ComparisonOperator::Equal,
                            left: usize_local(0),
                            right: usize_const(42),
                        },
                        then_instructions: vec![set_return_i32(0), Instruction::Return],
                        else_instructions: vec![set_return_i32(1), Instruction::Return],
                    },
                ],
            },
            Function {
                name: "choose".to_string(),
                target: crate::ir::CallTarget::same_file("choose".to_string()),
                return_type: Type::Usize,
                instructions: vec![
                    Instruction::SetUsize {
                        destination: UsizeLocation::Return,
                        value: usize_param(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_usize_parameter_tail_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value: usize = forward(42)
    if value == 42 {
        return 0
    } else {
        return 1
    }
}

func forward(value: usize): usize {
    return identity(value)
}

func identity(value: usize): usize {
    return value
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
                    call_usize(
                        UsizeLocation::Local(0),
                        "forward",
                        vec![ScalarArgument::Usize(usize_const(42))],
                    ),
                    Instruction::If {
                        condition: BoolValue::UsizeComparison {
                            operator: I32ComparisonOperator::Equal,
                            left: usize_local(0),
                            right: usize_const(42),
                        },
                        then_instructions: vec![set_return_i32(0), Instruction::Return],
                        else_instructions: vec![set_return_i32(1), Instruction::Return],
                    },
                ],
            },
            Function {
                name: "forward".to_string(),
                target: crate::ir::CallTarget::same_file("forward".to_string()),
                return_type: Type::Usize,
                instructions: vec![Instruction::TailCall {
                    target: CallTarget::same_file("identity"),
                    arguments: vec![ScalarArgument::Usize(usize_param(0))],
                }],
            },
            Function {
                name: "identity".to_string(),
                target: crate::ir::CallTarget::same_file("identity".to_string()),
                return_type: Type::Usize,
                instructions: vec![
                    Instruction::SetUsize {
                        destination: UsizeLocation::Return,
                        value: usize_param(0),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_usize_arithmetic_and_shift_returns() {
    let text = r#"func main(): i32 {
    return 0
}

func add(left: usize, right: usize): usize {
    return left + right
}

func subtract(left: usize, right: usize): usize {
    return left - right
}

func multiply(left: usize, right: usize): usize {
    return left * right
}

func divide(left: usize, right: usize): usize {
    return left / right
}

func remainder(left: usize, right: usize): usize {
    return left % right
}

func shift_left(left: usize, right: usize): usize {
    return left << right
}

func shift_right(left: usize, right: usize): usize {
    return left >> right
}
"#;

    for (name, instruction) in [
        (
            "add",
            Instruction::AddUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "subtract",
            Instruction::SubtractUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "multiply",
            Instruction::MultiplyUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "divide",
            Instruction::DivideUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "remainder",
            Instruction::RemainderUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "shift_left",
            Instruction::ShiftLeftUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
        (
            "shift_right",
            Instruction::ShiftRightUsize {
                destination: UsizeLocation::Return,
                left: usize_param(0),
                right: usize_param(1),
            },
        ),
    ] {
        assert_eq!(
            lower_named_function(text, name),
            Function {
                name: name.to_string(),
                target: crate::ir::CallTarget::same_file(name),
                return_type: Type::Usize,
                instructions: vec![instruction, Instruction::Return],
            }
        );
    }
}

#[test]
fn lowers_usize_call_in_nested_arithmetic_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func score(base: usize): usize {
    return base + size() * 2
}

func size(): usize {
    return 20
}
"#,
        "score",
    );

    assert_eq!(
        function,
        Function {
            name: "score".to_string(),
            target: crate::ir::CallTarget::same_file("score".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                call_usize(UsizeLocation::Local(1), "size", vec![]),
                Instruction::MultiplyUsize {
                    destination: UsizeLocation::Local(0),
                    left: usize_local(1),
                    right: usize_const(2),
                },
                Instruction::AddUsize {
                    destination: UsizeLocation::Return,
                    left: usize_param(0),
                    right: usize_local(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_entry_usize_arithmetic_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let left: usize = 20
    let right: usize = 6
    if left + right * 2 == 32 {
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
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(0),
                    value: usize_const(20),
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(1),
                    value: usize_const(6),
                },
                Instruction::MultiplyUsize {
                    destination: UsizeLocation::Local(3),
                    left: usize_local(1),
                    right: usize_const(2),
                },
                Instruction::AddUsize {
                    destination: UsizeLocation::Local(2),
                    left: usize_local(0),
                    right: usize_local(3),
                },
                Instruction::If {
                    condition: BoolValue::UsizeComparison {
                        operator: I32ComparisonOperator::Equal,
                        left: usize_local(2),
                        right: usize_const(32),
                    },
                    then_instructions: vec![set_return_i32(0), Instruction::Return],
                    else_instructions: vec![set_return_i32(1), Instruction::Return],
                },
            ],
        }])
    );
}

#[test]
fn lowers_bool_parameter_normal_call_in_terminal_if() {
    let ir = lower_text(
        r#"func main(): i32 {
    if choose(7, true, 42) {
        return 0
    } else {
        return 1
    }
}

func choose(code: i32, flag: bool, size: usize): bool {
    return flag
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
                    call_bool(
                        BoolLocation::Local(0),
                        "choose",
                        vec![
                            ScalarArgument::I32(i32_const(7)),
                            ScalarArgument::Bool(BoolValue::Const(true)),
                            ScalarArgument::Usize(usize_const(42)),
                        ],
                    ),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(0), Instruction::Return],
                        else_instructions: vec![set_return_i32(1), Instruction::Return],
                    },
                ],
            },
            Function {
                name: "choose".to_string(),
                target: crate::ir::CallTarget::same_file("choose".to_string()),
                return_type: Type::Bool,
                instructions: vec![
                    Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: bool_param(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_imported_i32_normal_call() {
    let analysis = analyze_text_with_entry_and_nocter_home_files(
        r#"from std/math import answer

func main(): i32 {
    let value = answer()
    return value
}
"#,
        crate::entry::DEFAULT_ENTRY_NAME,
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

    let ir = lower_executable_with_entry(&analysis, crate::entry::DEFAULT_ENTRY_NAME).unwrap();

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
fn lowers_imported_bool_normal_call_in_terminal_if_condition() {
    let analysis = analyze_text_with_entry_and_nocter_home_files(
        r#"from std/flags import ready

func main(): i32 {
    if ready() {
        return 42
    } else {
        return 1
    }
}
"#,
        crate::entry::DEFAULT_ENTRY_NAME,
        &[(
            "std/flags.nct",
            r#"pub func ready(): bool {
    return true
}
"#,
        )],
    );
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

    let ir = lower_executable_with_entry(&analysis, crate::entry::DEFAULT_ENTRY_NAME).unwrap();

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
    let analysis = analyze_text_with_entry_and_nocter_home_files(
        r#"from std/math import answer as imported_answer

func main(): i32 {
    return imported_answer()
}
"#,
        crate::entry::DEFAULT_ENTRY_NAME,
        &[(
            "std/math.nct",
            r#"pub func answer(): i32 {
    return 42
}
"#,
        )],
    );
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

    let ir = lower_executable_with_entry(&analysis, crate::entry::DEFAULT_ENTRY_NAME).unwrap();

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
fn lowers_never_function_returning_target_trap_primitive() {
    let analysis = analyze_text_with_entry_and_nocter_home_files(
        r#"from std/process import abort

func main(): i32 {
    return abort()
}
"#,
        crate::entry::DEFAULT_ENTRY_NAME,
        &[std_process_file(), std_macos_file()],
    );
    let process_source = analysis
        .files
        .iter()
        .find(|file| {
            !file.is_root
                && file.ast.items.iter().any(|item| {
                    matches!(item, crate::ast::Item::Function(function) if function.name == "abort")
                })
        })
        .map(|file| file.ast.span.source)
        .unwrap();

    let ir = lower_executable_with_entry(&analysis, crate::entry::DEFAULT_ENTRY_NAME).unwrap();

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: CallTarget::same_file("main"),
                return_type: Type::I32,
                instructions: vec![Instruction::TailCall {
                    target: CallTarget::imported(process_source, "abort"),
                    arguments: vec![],
                }],
            },
            Function {
                name: "abort".to_string(),
                target: CallTarget::imported(process_source, "abort"),
                return_type: Type::Never,
                instructions: vec![Instruction::Trap],
            },
        ])
    );
}

#[test]
fn collects_loaded_imported_call_targets() {
    let analysis = analyze_text_with_entry_and_nocter_home_files(
        r#"from std/math import answer

func main(): i32 {
    let value = answer()
    return value
}
"#,
        crate::entry::DEFAULT_ENTRY_NAME,
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
    assert!(matches!(
        targets[0].source,
        super::imported_calls::ImportedCallSource::Loaded(source)
            if source != root.ast.span.source
    ));
}

#[test]
fn indexes_imported_function_signatures_by_call_target() {
    let analysis = analyze_text_with_entry_and_nocter_home_files(
        r#"from std/math import answer

func main(): i32 {
    let value = answer()
    return value
}
"#,
        crate::entry::DEFAULT_ENTRY_NAME,
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
fn indexes_slice_function_signature_parameter_types() {
    let analysis = analyze_text_with_entry(
        r#"func main(): i32 {
    return 0
}

func consume(bytes: &[u8], scratch: &+[u8]): i32 {
    return 0
}
"#,
        crate::entry::DEFAULT_ENTRY_NAME,
    );
    let root = analysis.root_file().unwrap();
    let index = FunctionIndex::new(&analysis, root.ast.span.source);
    let signatures = index.signatures();

    assert_eq!(
        signatures.parameter_types(&CallTarget::same_file("consume")),
        Some(vec![readonly_u8_slice_type(), readwrite_u8_slice_type()].as_slice())
    );
}

#[test]
fn indexes_indirect_aggregate_function_signature_return_type() {
    let analysis = analyze_text_with_entry(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func make(): Text {
    return Text{ start: 0, len: 0, capacity: 0 }
}
"#,
        crate::entry::DEFAULT_ENTRY_NAME,
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
}

#[test]
fn indexes_direct_aggregate_function_signature_return_type() {
    let analysis = analyze_text_with_entry(
        r#"struct Allocator {
    state: usize
    kind: u64
}

func main(): i32 {
    return 0
}

func page_allocator(): Allocator {
    return Allocator{ state: 0, kind: 0 }
}
"#,
        crate::entry::DEFAULT_ENTRY_NAME,
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
}

#[test]
fn lowers_indirect_aggregate_usize_struct_literal_return() {
    let function = lower_named_function(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func make(): Text {
    return Text{ start: 1, len: 2, capacity: 3 }
}
"#,
        "make",
    );

    assert_eq!(
        function,
        Function {
            name: "make".to_string(),
            target: CallTarget::same_file("make"),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 0,
                    value: usize_const(1),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 8,
                    value: usize_const(2),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 16,
                    value: usize_const(3),
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
    return Text{ start: 1, len: 2, capacity: 3 }
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
    return Text{ start: 1, len: 2, capacity: 3 }
}

func forward(): Text {
    let value = make()
    return value
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
fn lowers_indirect_aggregate_struct_literal_binding_return() {
    let function = lower_named_function(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func forward(): Text {
    let value = Text{ start: 1, len: 2, capacity: 3 }
    return value
}
"#,
        "forward",
    );

    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
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
fn lowers_readwrite_indirect_aggregate_struct_literal_binding_borrow_argument() {
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

func touch(value: &+Text): void {
    return
}

func forward(): Text {
    var value = Text{ start: 1, len: 2, capacity: 3 }
    touch(&+value)
    return value
}
"#,
        "forward",
        function_signatures(vec![(
            "touch",
            Type::Void,
            vec![Type::Borrow {
                is_readwrite: true,
                inner: Box::new(aggregate_type.clone()),
            }],
        )]),
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
    return Text{ start: 1, len: 2, capacity: 3 }
}

func touch(value: &+Text): void {
    return
}

func forward(): Text {
    var value = make()
    touch(&+value)
    return value
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
fn lowers_propagated_indirect_aggregate_call_binding_return() {
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

func make(): Text! {
    return Text{ start: 1, len: 2, capacity: 3 }
}

func forward(): Text! {
    var value = make()?
    return value
}
"#,
        "forward",
        function_signatures(vec![(
            "make",
            Type::Fallible(Box::new(aggregate_type.clone())),
            vec![],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "forward".to_string(),
            target: CallTarget::same_file("forward"),
            return_type: Type::Fallible(Box::new(aggregate_type)),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CallFallibleAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Return,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
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
fn lowers_direct_aggregate_usize_struct_literal_return() {
    let function = lower_named_function(
        r#"struct Pair {
    first: usize
    second: usize
}

func main(): i32 {
    return 0
}

func make(): Pair {
    return Pair{ first: 1, second: 2 }
}
"#,
        "make",
    );

    assert_eq!(
        function,
        Function {
            name: "make".to_string(),
            target: CallTarget::same_file("make"),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(16, 8),
                words: 2,
            },
            instructions: vec![
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
        }
    );
}

#[test]
fn lowers_direct_aggregate_struct_literal_binding_return() {
    let function = lower_named_function(
        r#"struct Allocator {
    state: usize
    kind: u64
}

func main(): i32 {
    return 0
}

func make(): Allocator {
    let allocator = Allocator{ state: 1, kind: 2 }
    return allocator
}
"#,
        "make",
    );

    assert_eq!(
        function,
        Function {
            name: "make".to_string(),
            target: CallTarget::same_file("make"),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(16, 8),
                words: 2,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
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
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(16, 8),
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
    return Allocator{ state: 0, kind: 0 }
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
fn lowers_pointer_from_addr_aggregate_field_return() {
    let function = lower_imported_named_function_with_nocter_home_files(
        r#"from std/text import make

func main(): i32 {
    return 0
}
"#,
        "make",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
"#,
            ),
            (
                "std/text.nct",
                r#"from std/ptr import from_addr

pub struct Text {
    ptr: *u8
    len: usize
    capacity: usize
}

pub func make(): Text {
    return Text{ ptr: from_addr(1), len: 2, capacity: 3 }
}
"#,
            ),
        ],
    );

    assert_eq!(function.name, "make");
    assert!(matches!(
        function.target,
        CallTarget::Imported { ref name, .. } if name == "make"
    ));
    assert_eq!(
        function.return_type,
        Type::Aggregate {
            layout: ValueLayout::new(24, 8),
        }
    );
    assert_eq!(
        function.instructions,
        vec![
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Return,
                offset: 0,
                value: usize_const(1),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Return,
                offset: 8,
                value: usize_const(2),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Return,
                offset: 16,
                value: usize_const(3),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_pointer_from_addr_aggregate_field_binding_return() {
    let function = lower_imported_named_function_with_nocter_home_files(
        r#"from std/text import make

func main(): i32 {
    return 0
}
"#,
        "make",
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
"#,
            ),
            (
                "std/text.nct",
                r#"from std/ptr import from_addr

pub struct Text {
    ptr: *u8
    len: usize
    capacity: usize
}

pub func make(): Text {
    let value = Text{ ptr: from_addr(1), len: 2, capacity: 3 }
    return value
}
"#,
            ),
        ],
    );

    assert_eq!(function.name, "make");
    assert!(matches!(
        function.target,
        CallTarget::Imported { ref name, .. } if name == "make"
    ));
    assert_eq!(
        function.return_type,
        Type::Aggregate {
            layout: ValueLayout::new(24, 8),
        }
    );
    assert_eq!(
        function.instructions,
        vec![
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
            Instruction::CopyAggregate {
                destination: AggregateLocation::Return,
                source: AggregateLocation::Slot(0),
                layout: ValueLayout::new(24, 8),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn rejects_function_with_more_than_eight_abi_parameter_words() {
    let diagnostics = lower_named_function_diagnostics_with_signatures(
        r#"func main(): i32 {
    return 0
}

func consume(a: &str, b: &str, c: &str, d: &str, e: usize): i32 {
    return 0
}
"#,
        "consume",
        context::FunctionSignatures::new(HashMap::new()),
    );

    assert_eq!(diagnostics[0].code, "E8007");
    assert!(
        diagnostics[0]
            .message
            .contains("up to 8 ABI parameter words"),
        "{diagnostics:?}"
    );
}

#[test]
fn lowers_imported_i32_call_target_when_boundary_is_bypassed() {
    let analysis = analyze_text_with_entry_and_nocter_home_files(
        r#"from std/math import answer

func main(): i32 {
    let value = answer()
    return value
}
"#,
        crate::entry::DEFAULT_ENTRY_NAME,
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
    let index = FunctionIndex::new(&analysis, root.ast.span.source);

    let function = entry::lower_entry_function(
        entry,
        index.signatures(),
        index.names(),
        root.ast.span.source,
        &root.resolved,
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
fn lowers_entry_i32_let_initializer_normal_call_with_arguments() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = add(20, 22)
    return value
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    call_i32(
                        I32Location::Local(0),
                        "add",
                        vec![i32_const(20), i32_const(22)]
                    ),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "add".to_string(),
                target: crate::ir::CallTarget::same_file("add".to_string()),
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
fn lowers_i32_let_initializer_normal_call_with_non_reordered_parameter_arguments() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func add(a: i32, b: i32): i32 {
    return a + b
}

func wrapper(a: i32, b: i32): i32 {
    let value = add(a, b)
    return value
}
"#,
        "wrapper",
        context::FunctionSignatures::new(HashMap::from([("add".to_string(), Type::I32)])),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::I32,
            instructions: vec![
                call_i32(
                    I32Location::Local(0),
                    "add",
                    vec![i32_param(0), i32_param(1)]
                ),
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
fn lowers_entry_i32_return_expression_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer() + 1
}

func answer(): i32 {
    return 41
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
                    call_i32(I32Location::Local(0), "answer", vec![]),
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_const(1),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(41), Instruction::Return],
            },
        ])
    );
}

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
fn lowers_scalar_borrow_call_argument_as_one_abi_word() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = 7
    let result = choose(&value, 42)
    return result
}

func choose(value: &i32, code: i32): i32 {
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
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: I32Value::Const(7),
                    },
                    Instruction::CallI32 {
                        destination: I32Location::Local(1),
                        target: CallTarget::same_file("choose"),
                        arguments: vec![
                            ScalarArgument::Borrow(BorrowArgument {
                                source: BorrowSource::I32(I32Location::Local(0)),
                            }),
                            ScalarArgument::I32(I32Value::Const(42)),
                        ],
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(1),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "choose".to_string(),
                target: crate::ir::CallTarget::same_file("choose".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_param(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_readwrite_scalar_borrow_call_argument_as_one_abi_word() {
    let ir = lower_text(
        r#"func main(): i32 {
    var value = 7
    let result = choose(&+value, 42)
    return result
}

func choose(value: &+i32, code: i32): i32 {
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
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: I32Value::Const(7),
                    },
                    Instruction::CallI32 {
                        destination: I32Location::Local(1),
                        target: CallTarget::same_file("choose"),
                        arguments: vec![
                            ScalarArgument::Borrow(BorrowArgument {
                                source: BorrowSource::I32(I32Location::Local(0)),
                            }),
                            ScalarArgument::I32(I32Value::Const(42)),
                        ],
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(1),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "choose".to_string(),
                target: crate::ir::CallTarget::same_file("choose".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_param(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn rejects_tail_call_with_borrow_argument() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    let value = 7
    return choose(&value, 42)
}

func choose(value: &i32, code: i32): i32 {
    return code
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E8006");
    assert!(
        diagnostics[0]
            .message
            .contains("tail call to function `choose` with borrow arguments")
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
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::StrLen(StrLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
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
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::Const(6),
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
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::StrLen(StrLocation::Local(0)),
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
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                            source: SliceLocation::Local(0),
                            index: usize_const(0),
                        })),
                        right: I32Value::U8ZeroExtend(Box::new(u8_const(1))),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_parameter_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func echo(byte: u8): u8 {
    return byte
}
"#,
        "echo",
    );

    assert_eq!(
        function,
        Function {
            name: "echo".to_string(),
            target: crate::ir::CallTarget::same_file("echo".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: u8_param(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_local_binding_and_normal_call() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func wrapper(): u8 {
    let byte: u8 = identity(7)
    return byte
}

func identity(byte: u8): u8 {
    return byte
}
"#,
        "wrapper",
        function_signatures(vec![("identity", Type::U8, vec![Type::U8])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::U8,
            instructions: vec![
                call_u8(
                    U8Location::Local(0),
                    "identity",
                    vec![ScalarArgument::U8(u8_const(7))],
                ),
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: u8_local(0),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_entry_u8_let_initializer_call_with_indexed_signature() {
    let ir = lower_text(
        r#"func main(): i32 {
    let byte: u8 = identity(7)
    return 0
}

func identity(byte: u8): u8 {
    return byte
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
                    call_u8(
                        U8Location::Local(0),
                        "identity",
                        vec![ScalarArgument::U8(u8_const(7))],
                    ),
                    set_return_i32(0),
                    Instruction::Return,
                ],
            },
            Function {
                name: "identity".to_string(),
                target: crate::ir::CallTarget::same_file("identity".to_string()),
                return_type: Type::U8,
                instructions: vec![
                    Instruction::SetU8 {
                        destination: U8Location::Return,
                        value: u8_param(0),
                    },
                    Instruction::Return,
                ],
            },
        ])
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
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: usize_const(0),
                        })),
                        right: I32Value::U8ZeroExtend(Box::new(u8_const(0x7F))),
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
            instructions: vec![Instruction::If {
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Equal,
                    left: I32Value::U8ZeroExtend(Box::new(U8Value::StaticStrIndex {
                        bytes: b"Nocter".to_vec(),
                        index: usize_const(0),
                    })),
                    right: I32Value::U8ZeroExtend(Box::new(u8_const(78))),
                },
                then_instructions: vec![set_return_i32(0), Instruction::Return],
                else_instructions: vec![set_return_i32(1), Instruction::Return],
            }],
        }])
    );
}

#[test]
fn lowers_u8_normal_call_comparison() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func check(byte: u8): bool {
    return identity(byte) != 0
}

func identity(byte: u8): u8 {
    return byte
}
"#,
        "check",
        function_signatures(vec![("identity", Type::U8, vec![Type::U8])]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "check".to_string(),
            target: crate::ir::CallTarget::same_file("check".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_u8(
                    U8Location::Local(0),
                    "identity",
                    vec![ScalarArgument::U8(u8_param(0))],
                ),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::NotEqual,
                        left: I32Value::U8ZeroExtend(Box::new(u8_local(0))),
                        right: I32Value::U8ZeroExtend(Box::new(u8_const(0))),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_index_conversion_to_i32_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(text: &str): i32 {
    return text[0] as i32
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
                    value: I32Value::U8ZeroExtend(Box::new(U8Value::StrIndex {
                        source: StrLocation::Parameter(0),
                        index: usize_const(0),
                    })),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_u8_index_conversion_to_usize_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func first(bytes: &[u8]): usize {
    return bytes[1] as usize
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
                    value: UsizeValue::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: usize_const(1),
                    })),
                },
                Instruction::Return,
            ],
        }
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
fn lowers_entry_i32_let_initializer_normal_call_addition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = answer() + 1
    return value
}

func answer(): i32 {
    return 41
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
                    call_i32(I32Location::Local(0), "answer", vec![]),
                    Instruction::AddI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(0),
                        right: i32_const(1),
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
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(41), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_return_expression_local_plus_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    let base = 5
    return base + answer()
}

func answer(): i32 {
    return 37
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
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(5),
                    },
                    call_i32(I32Location::Local(1), "answer", vec![]),
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_local(1),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(37), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_nested_return_addition_with_one_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    return (answer() + 1) + 2
}

func answer(): i32 {
    return 39
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
                    call_i32(I32Location::Local(1), "answer", vec![]),
                    Instruction::AddI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(1),
                        right: i32_const(1),
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_const(2),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(39), Instruction::Return],
            },
        ])
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
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
fn lowers_entry_i32_subtract_and_multiply_with_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    return answer() * 2 - offset()
}

func answer(): i32 {
    return 24
}

func offset(): i32 {
    return 6
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
                    call_i32(I32Location::Local(1), "answer", vec![]),
                    Instruction::MultiplyI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(1),
                        right: i32_const(2),
                    },
                    call_i32(I32Location::Local(2), "offset", vec![]),
                    Instruction::SubtractI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_local(2),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(24), Instruction::Return],
            },
            Function {
                name: "offset".to_string(),
                target: crate::ir::CallTarget::same_file("offset".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(6), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_divide_and_remainder_with_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    return total() / divisor() + dividend() % modulus()
}

func total(): i32 {
    return 84
}

func divisor(): i32 {
    return 2
}

func dividend(): i32 {
    return 85
}

func modulus(): i32 {
    return 43
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
                    call_i32(I32Location::Local(1), "total", vec![]),
                    call_i32(I32Location::Local(2), "divisor", vec![]),
                    Instruction::DivideI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(1),
                        right: i32_local(2),
                    },
                    call_i32(I32Location::Local(4), "dividend", vec![]),
                    call_i32(I32Location::Local(5), "modulus", vec![]),
                    Instruction::RemainderI32 {
                        destination: I32Location::Local(3),
                        left: i32_local(4),
                        right: i32_local(5),
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_local(3),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "total".to_string(),
                target: crate::ir::CallTarget::same_file("total".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(84), Instruction::Return],
            },
            Function {
                name: "divisor".to_string(),
                target: crate::ir::CallTarget::same_file("divisor".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(2), Instruction::Return],
            },
            Function {
                name: "dividend".to_string(),
                target: crate::ir::CallTarget::same_file("dividend".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(85), Instruction::Return],
            },
            Function {
                name: "modulus".to_string(),
                target: crate::ir::CallTarget::same_file("modulus".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(43), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_shifts_with_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    return (value() << left_count()) + (shifted() >> right_count())
}

func value(): i32 {
    return 5
}

func left_count(): i32 {
    return 3
}

func shifted(): i32 {
    return 8
}

func right_count(): i32 {
    return 1
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
                    call_i32(I32Location::Local(1), "value", vec![]),
                    call_i32(I32Location::Local(2), "left_count", vec![]),
                    Instruction::ShiftLeftI32 {
                        destination: I32Location::Local(0),
                        left: i32_local(1),
                        right: i32_local(2),
                    },
                    call_i32(I32Location::Local(4), "shifted", vec![]),
                    call_i32(I32Location::Local(5), "right_count", vec![]),
                    Instruction::ShiftRightI32 {
                        destination: I32Location::Local(3),
                        left: i32_local(4),
                        right: i32_local(5),
                    },
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_local(3),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "value".to_string(),
                target: crate::ir::CallTarget::same_file("value".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(5), Instruction::Return],
            },
            Function {
                name: "left_count".to_string(),
                target: crate::ir::CallTarget::same_file("left_count".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(3), Instruction::Return],
            },
            Function {
                name: "shifted".to_string(),
                target: crate::ir::CallTarget::same_file("shifted".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(8), Instruction::Return],
            },
            Function {
                name: "right_count".to_string(),
                target: crate::ir::CallTarget::same_file("right_count".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(1), Instruction::Return],
            },
        ])
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
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
fn lowers_entry_bool_not_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    let blocked = false
    let enabled = !blocked
    if enabled {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::Const(false),
            },
            Instruction::SetBool {
                destination: BoolLocation::Local(1),
                value: BoolValue::Not(Box::new(BoolValue::Location(BoolLocation::Local(0)))),
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(1)),
                then_instructions: vec![set_return_i32(0), Instruction::Return],
                else_instructions: vec![set_return_i32(1), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_entry_terminal_if_with_bool_and_condition() {
    let ir = lower_text(
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    if ready && !blocked {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::Const(true),
            },
            Instruction::SetBool {
                destination: BoolLocation::Local(1),
                value: BoolValue::Const(false),
            },
            Instruction::If {
                condition: BoolValue::Logical {
                    operator: BoolLogicalOperator::And,
                    left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                    right: Box::new(BoolValue::Not(Box::new(BoolValue::Location(
                        BoolLocation::Local(1),
                    )))),
                },
                then_instructions: vec![set_return_i32(0), Instruction::Return],
                else_instructions: vec![set_return_i32(1), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_entry_bool_equality_binding() {
    let ir = lower_text(
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = ready == blocked
    if same {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::Const(true),
            },
            Instruction::SetBool {
                destination: BoolLocation::Local(1),
                value: BoolValue::Const(false),
            },
            Instruction::SetBool {
                destination: BoolLocation::Local(2),
                value: BoolValue::BoolComparison {
                    operator: BoolComparisonOperator::Equal,
                    left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                    right: Box::new(BoolValue::Location(BoolLocation::Local(1))),
                },
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(2)),
                then_instructions: vec![set_return_i32(1), Instruction::Return],
                else_instructions: vec![set_return_i32(0), Instruction::Return],
            },
        ]
    );
}

#[test]
fn reports_unsupported_bool_equality_over_unary_operand() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = !ready == blocked
    if same {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8008");
    assert_eq!(
        diagnostics[0].message,
        "IR v0 can only lower bool equality/inequality operands that are bool literals or bool locals"
    );
}

#[test]
fn reports_unsupported_bool_equality_over_logical_operand() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    let same = (ready && !blocked) == ready
    if same {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8008");
    assert_eq!(
        diagnostics[0].message,
        "IR v0 can only lower bool equality/inequality operands that are bool literals or bool locals"
    );
}

#[test]
fn reports_unsupported_bool_equality_in_terminal_if_condition() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): i32 {
    let ready = true
    let blocked = false
    if !ready == blocked {
        return 1
    } else {
        return 0
    }
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8002");
    assert_eq!(
        diagnostics[0].message,
        "IR v0 can only lower bool equality/inequality operands that are bool literals or bool locals"
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
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
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![set_return_i32(7), Instruction::ReturnFallibleSuccess],
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
            instructions: vec![Instruction::ReturnFallibleSuccess],
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
            instructions: vec![set_return_i32(42), Instruction::ReturnFallibleSuccess],
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
                Instruction::CallFallibleI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::ReturnFallibleSuccess,
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
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(1),
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Propagate,
                },
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: I32Value::Location(I32Location::Local(0)),
                    right: I32Value::Location(I32Location::Local(1)),
                },
                Instruction::ReturnFallibleSuccess,
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
        Instruction::CallFallibleU8 {
            destination: U8Location::Local(0),
            ..
        }
    ));
    assert!(matches!(
        main.instructions[1],
        Instruction::CallFallibleUsize {
            destination: UsizeLocation::Local(1),
            ..
        }
    ));
    assert!(matches!(
        main.instructions[2],
        Instruction::CallFallibleBool {
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
        Instruction::CallFallibleStr {
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
        Instruction::CallFallibleSlice {
            destination: SliceLocation::Local(0),
            ..
        }
    ));
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
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Trap,
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
        Instruction::CallFallibleVoid { failure_mode, .. },
        Instruction::Return,
    ] = ir.functions[0].instructions.as_slice()
    else {
        panic!(
            "unexpected main instructions: {:?}",
            ir.functions[0].instructions
        );
    };
    assert_eq!(*failure_mode, FallibleFailureMode::Trap);
}

#[test]
fn lowers_fallible_void_function_static_error_failure() {
    let ir = lower_text_with_std_error(
        r#"from std/error import Error

func main(): void! {
    fail()?
}

func fail(): void! {
    return Error.new("app.inner", "inner failed")
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
fn lowers_fallible_i32_catch_failure_return() {
    let ir = lower_text_with_std_error(
        r#"from std/error import Error

func main(): i32! {
    let value = answer() catch error {
        return Error.new("app.answer", error.message)
    }
    return value
}

func answer(): i32! {
    return Error.new("app.inner", "inner failed")
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
                Instruction::CallFallibleI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: FallibleFailureMode::Catch {
                        code: StrLocation::Local(1),
                        message: StrLocation::Local(3),
                        instructions: vec![Instruction::ReturnFallibleFailure {
                            code: StrValue::StaticBytes(b"app.answer".to_vec()),
                            message: StrValue::Location(StrLocation::Local(3)),
                        }],
                    },
                },
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::Location(I32Location::Local(0)),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        }
    );
}

#[test]
fn lowers_fallible_write_text_raw_catch_failure_return() {
    let ir = lower_text_with_nocter_home_files(
        r#"from std/io_catch import print_catch

func main(): void! {
    print_catch("hello\n")?
}
"#,
        &[
            std_error_file(),
            std_io_impl_file(),
            (
                "std/io_catch.nct",
                r#"from std/error import Error
from std/io_impl import write_text_raw

pub func print_catch(text: &str): void! {
    write_text_raw(1, text) catch error {
        return Error.new("app.write", error.message)
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
                failure_mode: FallibleFailureMode::Catch {
                    code: StrLocation::Local(0),
                    message: StrLocation::Local(2),
                    instructions: vec![Instruction::ReturnFallibleFailure {
                        code: StrValue::StaticBytes(b"app.write".to_vec()),
                        message: StrValue::Location(StrLocation::Local(2)),
                    }],
                },
            },
            Instruction::ReturnFallibleSuccess,
        ]
    );
}

#[test]
fn lowers_fallible_entry_return_static_error_constructor() {
    let ir = lower_text_with_std_error(
        r#"from std/error import Error

func main(): i32! {
    return Error.new("app.failed", "failed")
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
fn lowers_fallible_entry_return_static_error_constructor_with_multi_line_message() {
    let ir = lower_text_with_std_error(
        r#"from std/error import Error

func main(): i32! {
    return Error.new("app.failed", """
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
        r#"from std/error import Error

func main(): i32! {
    return Error.new("app.failed", "failed\n")
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
fn reports_unsupported_fail_payload() {
    let diagnostics = lower_text_diagnostics_with_std_error(
        r#"from std/error import Error

func main(): i32! {
    return Error.new("app.failed", dynamic())
}

func dynamic(): &str {
    return "failed"
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8004");
}

#[test]
fn reports_unsupported_interpolated_string_binding_lowering() {
    let diagnostics = lower_text_diagnostics(
        r#"struct String {
    bytes: &[u8]
}

func main(): i32! {
    let text = "value ${1}"?
    return 0
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8008");
    assert!(diagnostics[0].message.contains("std/fmt.append_*"));
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
fn lowers_void_entry_with_void_call_statement() {
    let ir = lower_text(
        r#"func main(): void {
    effect()
}

func effect(): void {
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::Void,
                instructions: vec![call_void("effect", vec![]), Instruction::Return],
            },
            Function {
                name: "effect".to_string(),
                target: crate::ir::CallTarget::same_file("effect".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_leading_void_call_statement_before_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    effect()
    return 7
}

func effect(): void {
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
                    call_void("effect", vec![]),
                    set_return_i32(7),
                    Instruction::Return
                ],
            },
            Function {
                name: "effect".to_string(),
                target: crate::ir::CallTarget::same_file("effect".to_string()),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_void_function_with_void_call_statement() {
    let ir = lower_text(
        r#"func main(): i32 {
    run()
    return 7
}

func run(): void {
    effect()
}

func effect(): void {
}
"#,
    );

    assert_eq!(
        ir.functions[1],
        Function {
            name: "run".to_string(),
            target: crate::ir::CallTarget::same_file("run".to_string()),
            return_type: Type::Void,
            instructions: vec![call_void("effect", vec![]), Instruction::Return],
        }
    );
}

#[test]
fn lowers_fallible_void_entry_propagating_std_print() {
    let ir = lower_text_with_nocter_home_files(
        r#"from std/io import print

func main(): void! {
    print("hello\n")?
}
"#,
        &[std_io_file(), std_io_impl_file()],
    );

    let [main, print] = ir.functions.as_slice() else {
        panic!("unexpected lowered functions: {:?}", ir.functions);
    };

    assert_eq!(main.return_type, Type::Fallible(Box::new(Type::Void)));
    let [
        Instruction::CallFallibleVoid {
            target, arguments, ..
        },
        Instruction::ReturnFallibleSuccess,
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
            Instruction::ReturnFallibleSuccess,
        ]
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("add", vec![i32_const(20), i32_const(22)])],
            },
            Function {
                name: "add".to_string(),
                target: crate::ir::CallTarget::same_file("add".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("answer", vec![])],
            },
            Function {
                name: "answer".to_string(),
                target: crate::ir::CallTarget::same_file("answer".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("differs", vec![i32_const(40), i32_const(2)])],
            },
            Function {
                name: "differs".to_string(),
                target: crate::ir::CallTarget::same_file("differs".to_string()),
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
                target: crate::ir::CallTarget::same_file("main".to_string()),
                return_type: Type::I32,
                instructions: vec![tail_call("at_least", vec![i32_const(42), i32_const(40)])],
            },
            Function {
                name: "at_least".to_string(),
                target: crate::ir::CallTarget::same_file("at_least".to_string()),
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
fn lowers_bool_returning_function() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func enabled(): bool {
    let ready = true
    let blocked = false
    return ready && !blocked
}
"#,
        "enabled",
    );

    assert_eq!(
        function,
        Function {
            name: "enabled".to_string(),
            target: crate::ir::CallTarget::same_file("enabled".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(true),
                },
                Instruction::SetBool {
                    destination: BoolLocation::Local(1),
                    value: BoolValue::Const(false),
                },
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::Logical {
                        operator: BoolLogicalOperator::And,
                        left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                        right: Box::new(BoolValue::Not(Box::new(BoolValue::Location(
                            BoolLocation::Local(1),
                        )))),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_bool_returning_function_with_terminal_if() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func enabled(): bool {
    let ready = true
    if ready {
        return true
    } else {
        return false
    }
}
"#,
        "enabled",
    );

    assert_eq!(
        function,
        Function {
            name: "enabled".to_string(),
            target: crate::ir::CallTarget::same_file("enabled".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(true),
                },
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
        }
    );
}

#[test]
fn lowers_bool_returning_function_tail_call() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func enabled(): bool {
    return true
}

func mirrors_enabled(): bool {
    return enabled()
}
"#,
        "mirrors_enabled",
    );

    assert_eq!(
        function,
        Function {
            name: "mirrors_enabled".to_string(),
            target: crate::ir::CallTarget::same_file("mirrors_enabled".to_string()),
            return_type: Type::Bool,
            instructions: vec![tail_call("enabled", vec![])],
        }
    );
}

#[test]
fn rejects_tail_call_return_type_mismatch_during_lowering() {
    let diagnostics = lower_named_function_diagnostics_with_signatures(
        r#"func main(): i32 {
    return 0
}

func enabled(): i32 {
    return 1
}

func mirrors_enabled(): i32 {
    return enabled()
}
"#,
        "mirrors_enabled",
        context::FunctionSignatures::new(HashMap::from([("enabled".to_string(), Type::Bool)])),
    );

    assert_eq!(diagnostics[0].code, "E8006");
    assert_eq!(
        diagnostics[0].message,
        "IR v0 cannot lower tail call from function `mirrors_enabled` returning `i32` to function `enabled` returning `bool`"
    );
}

#[test]
fn lowers_entry_i32_nested_tail_call_argument() {
    let ir = lower_text(
        r#"func main(): i32 {
    return add(answer(), 1)
}

func answer(): i32 {
    return 41
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(0), "answer", vec![]),
            tail_call("add", vec![i32_local(0), i32_const(1)]),
        ]
    );
}

#[test]
fn lowers_entry_i32_multiple_nested_tail_call_arguments() {
    let ir = lower_text(
        r#"func main(): i32 {
    return add(left(), right())
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 22
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(0), "left", vec![]),
            call_i32(I32Location::Local(1), "right", vec![]),
            tail_call("add", vec![i32_local(0), i32_local(1)]),
        ]
    );
}

#[test]
fn lowers_entry_i32_let_initializer_nested_normal_call_argument() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = outer(inner())
    return value
}

func inner(): i32 {
    return 41
}

func outer(value: i32): i32 {
    return value + 1
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
                    call_i32(I32Location::Local(0), "inner", vec![]),
                    call_i32(I32Location::Local(0), "outer", vec![i32_local(0)]),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "inner".to_string(),
                target: crate::ir::CallTarget::same_file("inner".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(41), Instruction::Return],
            },
            Function {
                name: "outer".to_string(),
                target: crate::ir::CallTarget::same_file("outer".to_string()),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_param(0),
                        right: i32_const(1),
                    },
                    Instruction::Return,
                ],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_let_initializer_multiple_nested_normal_call_arguments() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = add(left(), right())
    return value
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 22
}

func add(a: i32, b: i32): i32 {
    return a + b
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(0), "left", vec![]),
            call_i32(I32Location::Local(1), "right", vec![]),
            call_i32(
                I32Location::Local(0),
                "add",
                vec![i32_local(0), i32_local(1)]
            ),
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_entry_i32_return_addition_with_nested_normal_call_argument() {
    let ir = lower_text(
        r#"func main(): i32 {
    return outer(inner()) + 1
}

func inner(): i32 {
    return 40
}

func outer(value: i32): i32 {
    return value + 1
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(1), "inner", vec![]),
            call_i32(I32Location::Local(0), "outer", vec![i32_local(1)]),
            Instruction::AddI32 {
                destination: I32Location::Return,
                left: i32_local(0),
                right: i32_const(1),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_entry_i32_return_expression_with_multiple_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    return left() + right()
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 22
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
                    call_i32(I32Location::Local(0), "left", vec![]),
                    call_i32(I32Location::Local(1), "right", vec![]),
                    Instruction::AddI32 {
                        destination: I32Location::Return,
                        left: i32_local(0),
                        right: i32_local(1),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "left".to_string(),
                target: crate::ir::CallTarget::same_file("left".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(20), Instruction::Return],
            },
            Function {
                name: "right".to_string(),
                target: crate::ir::CallTarget::same_file("right".to_string()),
                return_type: Type::I32,
                instructions: vec![set_return_i32(22), Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_entry_i32_let_initializer_with_multiple_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = left() + right()
    return value
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 22
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(0), "left", vec![]),
            call_i32(I32Location::Local(1), "right", vec![]),
            Instruction::AddI32 {
                destination: I32Location::Local(0),
                left: i32_local(0),
                right: i32_local(1),
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
fn lowers_entry_i32_multiple_normal_calls_without_colliding_with_local() {
    let ir = lower_text(
        r#"func main(): i32 {
    let base = 1
    return (left() + right()) + base
}

func left(): i32 {
    return 20
}

func right(): i32 {
    return 21
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(1),
            },
            call_i32(I32Location::Local(2), "left", vec![]),
            call_i32(I32Location::Local(3), "right", vec![]),
            Instruction::AddI32 {
                destination: I32Location::Local(1),
                left: i32_local(2),
                right: i32_local(3),
            },
            Instruction::AddI32 {
                destination: I32Location::Return,
                left: i32_local(1),
                right: i32_local(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_reordered_normal_call_arguments() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func first(a: i32, b: i32): i32 {
    return a
}

func wrapper(a: i32, b: i32): i32 {
    let value = first(b, a)
    return value
}
"#,
        "wrapper",
        context::FunctionSignatures::new(HashMap::from([("first".to_string(), Type::I32)])),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::I32,
            instructions: vec![
                call_i32(
                    I32Location::Local(0),
                    "first",
                    vec![i32_param(1), i32_param(0)]
                ),
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
fn lowers_reordered_tail_call_arguments() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func first(a: i32, b: i32): i32 {
    return a
}

func wrapper(a: i32, b: i32): i32 {
    return first(b, a)
}
"#,
        "wrapper",
        context::FunctionSignatures::new(HashMap::new()),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::I32,
            instructions: vec![tail_call("first", vec![i32_param(1), i32_param(0)])],
        }
    );
}

#[test]
fn lowers_entry_bool_let_initializer_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = ready()
    if value {
        return 0
    } else {
        return 1
    }
}

func ready(): bool {
    return true
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
                    call_bool(BoolLocation::Local(0), "ready", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(0), Instruction::Return],
                        else_instructions: vec![set_return_i32(1), Instruction::Return],
                    },
                ],
            },
            Function {
                name: "ready".to_string(),
                target: crate::ir::CallTarget::same_file("ready".to_string()),
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
fn lowers_bool_return_not_normal_call() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func ready(): bool {
    return false
}

func disabled(): bool {
    return !ready()
}
"#,
        "disabled",
    );

    assert_eq!(
        function,
        Function {
            name: "disabled".to_string(),
            target: crate::ir::CallTarget::same_file("disabled".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_bool(BoolLocation::Local(0), "ready", vec![]),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::Not(Box::new(BoolValue::Location(BoolLocation::Local(0)))),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_bool_let_initializer_not_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    let disabled = !ready()
    if disabled {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return false
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::Not(Box::new(BoolValue::Location(BoolLocation::Local(0)))),
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_bool_let_initializer_and_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = ready() && other()
    if value {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}

func other(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![
                    call_bool(BoolLocation::Local(0), "other", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![Instruction::SetBool {
                            destination: BoolLocation::Local(0),
                            value: BoolValue::Const(true),
                        }],
                        else_instructions: vec![Instruction::SetBool {
                            destination: BoolLocation::Local(0),
                            value: BoolValue::Const(false),
                        }],
                    },
                ],
                else_instructions: vec![Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(false),
                }],
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_bool_return_or_normal_calls() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func ready(): bool {
    return false
}

func other(): bool {
    return true
}

func enabled(): bool {
    return ready() || other()
}
"#,
        "enabled",
    );

    assert_eq!(
        function,
        Function {
            name: "enabled".to_string(),
            target: crate::ir::CallTarget::same_file("enabled".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_bool(BoolLocation::Local(0), "ready", vec![]),
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions: vec![Instruction::SetBool {
                        destination: BoolLocation::Return,
                        value: BoolValue::Const(true),
                    }],
                    else_instructions: vec![
                        call_bool(BoolLocation::Local(0), "other", vec![]),
                        Instruction::If {
                            condition: BoolValue::Location(BoolLocation::Local(0)),
                            then_instructions: vec![Instruction::SetBool {
                                destination: BoolLocation::Return,
                                value: BoolValue::Const(true),
                            }],
                            else_instructions: vec![Instruction::SetBool {
                                destination: BoolLocation::Return,
                                value: BoolValue::Const(false),
                            }],
                        },
                    ],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_bool_let_initializer_normal_call_comparison() {
    let ir = lower_text(
        r#"func main(): i32 {
    let value = ready() == true
    if value {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::BoolComparison {
                    operator: BoolComparisonOperator::Equal,
                    left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                    right: Box::new(BoolValue::Const(true)),
                },
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_bool_return_normal_call_comparison() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func left(): bool {
    return true
}

func right(): bool {
    return false
}

func differs(): bool {
    return left() != right()
}
"#,
        "differs",
        context::FunctionSignatures::new(HashMap::from([
            ("left".to_string(), Type::Bool),
            ("right".to_string(), Type::Bool),
        ])),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "differs".to_string(),
            target: crate::ir::CallTarget::same_file("differs".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_bool(BoolLocation::Local(0), "left", vec![]),
                call_bool(BoolLocation::Local(1), "right", vec![]),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::BoolComparison {
                        operator: BoolComparisonOperator::NotEqual,
                        left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                        right: Box::new(BoolValue::Location(BoolLocation::Local(1))),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_entry_i32_if_condition_normal_call_comparison() {
    let ir = lower_text(
        r#"func main(): i32 {
    if left() == right() {
        return 42
    } else {
        return 7
    }
}

func left(): bool {
    return true
}

func right(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "left", vec![]),
            call_bool(BoolLocation::Local(1), "right", vec![]),
            Instruction::If {
                condition: BoolValue::BoolComparison {
                    operator: BoolComparisonOperator::Equal,
                    left: Box::new(BoolValue::Location(BoolLocation::Local(0))),
                    right: Box::new(BoolValue::Location(BoolLocation::Local(1))),
                },
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_entry_i32_if_condition_i32_normal_call_comparison() {
    let ir = lower_text(
        r#"func main(): i32 {
    if answer() == 42 {
        return 42
    } else {
        return 7
    }
}

func answer(): i32 {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(0), "answer", vec![]),
            Instruction::If {
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Equal,
                    left: i32_local(0),
                    right: i32_const(42),
                },
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_bool_let_initializer_i32_normal_call_comparison() {
    let ir = lower_text(
        r#"func main(): i32 {
    let matched = answer() <= limit()
    if matched {
        return 42
    } else {
        return 7
    }
}

func answer(): i32 {
    return 40
}

func limit(): i32 {
    return 42
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(0), "answer", vec![]),
            call_i32(I32Location::Local(1), "limit", vec![]),
            Instruction::SetBool {
                destination: BoolLocation::Local(0),
                value: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::LessEqual,
                    left: i32_local(0),
                    right: i32_local(1),
                },
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_bool_return_i32_normal_call_comparison() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func left(): i32 {
    return 40
}

func right(): i32 {
    return 42
}

func less(): bool {
    return left() < right()
}
"#,
        "less",
        context::FunctionSignatures::new(HashMap::from([
            ("left".to_string(), Type::I32),
            ("right".to_string(), Type::I32),
        ])),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "less".to_string(),
            target: crate::ir::CallTarget::same_file("less".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                call_i32(I32Location::Local(0), "left", vec![]),
                call_i32(I32Location::Local(1), "right", vec![]),
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Less,
                        left: i32_local(0),
                        right: i32_local(1),
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_entry_i32_if_condition_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    if ready() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_entry_i32_if_condition_not_normal_call() {
    let ir = lower_text(
        r#"func main(): i32 {
    if !ready() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return false
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::If {
                condition: BoolValue::Not(Box::new(BoolValue::Location(BoolLocation::Local(0)))),
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_bool_if_condition_normal_call() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func ready(): bool {
    return true
}

func choose(): bool {
    if ready() {
        return false
    } else {
        return true
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
            return_type: Type::Bool,
            instructions: vec![
                call_bool(BoolLocation::Local(0), "ready", vec![]),
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions: vec![
                        Instruction::SetBool {
                            destination: BoolLocation::Return,
                            value: BoolValue::Const(false),
                        },
                        Instruction::Return,
                    ],
                    else_instructions: vec![
                        Instruction::SetBool {
                            destination: BoolLocation::Return,
                            value: BoolValue::Const(true),
                        },
                        Instruction::Return,
                    ],
                },
            ],
        }
    );
}

#[test]
fn lowers_entry_i32_if_condition_and_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    if ready() && other() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}

func other(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![
                    call_bool(BoolLocation::Local(0), "other", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(42), Instruction::Return],
                        else_instructions: vec![set_return_i32(7), Instruction::Return],
                    },
                ],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_entry_i32_if_condition_and_i32_call_comparison() {
    let ir = lower_text(
        r#"func main(): i32 {
    if answer() == 42 && ready() {
        return 42
    } else {
        return 7
    }
}

func answer(): i32 {
    return 42
}

func ready(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(0), "answer", vec![]),
            Instruction::If {
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Equal,
                    left: i32_local(0),
                    right: i32_const(42),
                },
                then_instructions: vec![
                    call_bool(BoolLocation::Local(0), "ready", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(42), Instruction::Return],
                        else_instructions: vec![set_return_i32(7), Instruction::Return],
                    },
                ],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_bool_let_initializer_and_i32_call_comparison() {
    let ir = lower_text(
        r#"func main(): i32 {
    let matched = answer() == 42 && ready()
    if matched {
        return 42
    } else {
        return 7
    }
}

func answer(): i32 {
    return 42
}

func ready(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_i32(I32Location::Local(0), "answer", vec![]),
            Instruction::If {
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Equal,
                    left: i32_local(0),
                    right: i32_const(42),
                },
                then_instructions: vec![
                    call_bool(BoolLocation::Local(0), "ready", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![Instruction::SetBool {
                            destination: BoolLocation::Local(0),
                            value: BoolValue::Const(true),
                        }],
                        else_instructions: vec![Instruction::SetBool {
                            destination: BoolLocation::Local(0),
                            value: BoolValue::Const(false),
                        }],
                    },
                ],
                else_instructions: vec![Instruction::SetBool {
                    destination: BoolLocation::Local(0),
                    value: BoolValue::Const(false),
                }],
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
    );
}

#[test]
fn lowers_entry_i32_if_condition_or_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    if ready() || other() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return false
}

func other(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![set_return_i32(42), Instruction::Return],
                else_instructions: vec![
                    call_bool(BoolLocation::Local(0), "other", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![set_return_i32(42), Instruction::Return],
                        else_instructions: vec![set_return_i32(7), Instruction::Return],
                    },
                ],
            },
        ]
    );
}

#[test]
fn lowers_entry_i32_if_condition_left_nested_and_normal_calls() {
    let ir = lower_text(
        r#"func main(): i32 {
    if ready() && other() && done() {
        return 42
    } else {
        return 7
    }
}

func ready(): bool {
    return true
}

func other(): bool {
    return true
}

func done(): bool {
    return true
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![
                    call_bool(BoolLocation::Local(0), "other", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![
                            call_bool(BoolLocation::Local(0), "done", vec![]),
                            Instruction::If {
                                condition: BoolValue::Location(BoolLocation::Local(0)),
                                then_instructions: vec![set_return_i32(42), Instruction::Return],
                                else_instructions: vec![set_return_i32(7), Instruction::Return],
                            },
                        ],
                        else_instructions: vec![set_return_i32(7), Instruction::Return],
                    },
                ],
                else_instructions: vec![set_return_i32(7), Instruction::Return],
            },
        ]
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

fn lower_text_with_std_error(text: &str) -> IrModule {
    lower_text_with_nocter_home_files(text, &[std_error_file()])
}

fn lower_text_with_nocter_home_files(text: &str, home_files: &[(&str, &str)]) -> IrModule {
    let entry_name = crate::entry::DEFAULT_ENTRY_NAME;
    let analysis = analyze_text_with_entry_and_nocter_home_files(text, entry_name, home_files);
    lower_executable_with_entry(&analysis, entry_name).unwrap()
}

fn lower_named_function(text: &str, function_name: &str) -> Function {
    lower_named_function_with_signatures(
        text,
        function_name,
        context::FunctionSignatures::new(HashMap::new()),
    )
    .unwrap()
}

fn lower_named_function_with_signatures(
    text: &str,
    function_name: &str,
    function_signatures: context::FunctionSignatures,
) -> Result<Function, Vec<Diagnostic>> {
    let analysis = analyze_text_with_entry(text, crate::entry::DEFAULT_ENTRY_NAME);
    let root = analysis.root_file().unwrap();
    let Some(crate::ast::Item::Function(function)) = root.ast.items.iter().find(|item| {
        matches!(item, crate::ast::Item::Function(function) if function.name == function_name)
    }) else {
        panic!("missing function `{function_name}`");
    };

    functions::lower_function(
        function,
        CallTarget::same_file(function_name),
        function_signatures,
        context::FunctionNames::default(),
        root.ast.span.source,
        &root.resolved,
    )
}

fn lower_imported_named_function_with_nocter_home_files(
    text: &str,
    function_name: &str,
    home_files: &[(&str, &str)],
) -> Function {
    let analysis = analyze_text_with_entry_and_nocter_home_files(
        text,
        crate::entry::DEFAULT_ENTRY_NAME,
        home_files,
    );
    let root = analysis.root_file().unwrap();
    let imported_source = analysis
        .files
        .iter()
        .find(|file| {
            !file.is_root
                && file.ast.items.iter().any(|item| {
                    matches!(item, crate::ast::Item::Function(function) if function.name == function_name)
                })
        })
        .map(|file| file.ast.span.source)
        .unwrap();
    let target = CallTarget::imported(imported_source, function_name);
    let index = FunctionIndex::new(&analysis, root.ast.span.source);
    let function = index.definition(&target).unwrap();

    functions::lower_function(
        function.declaration,
        target,
        index.signatures(),
        index.names(),
        root.ast.span.source,
        function.resolved,
    )
    .unwrap()
}

fn lower_named_function_diagnostics_with_signatures(
    text: &str,
    function_name: &str,
    function_signatures: context::FunctionSignatures,
) -> Vec<Diagnostic> {
    match lower_named_function_with_signatures(text, function_name, function_signatures) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

fn set_return_i32(value: i32) -> Instruction {
    Instruction::SetI32 {
        destination: I32Location::Return,
        value: i32_const(value),
    }
}

fn set_return_usize(value: u64) -> Instruction {
    Instruction::SetUsize {
        destination: UsizeLocation::Return,
        value: usize_const(value),
    }
}

fn tail_call(function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::TailCall {
        target: CallTarget::same_file(function),
        arguments: i32_arguments(arguments),
    }
}

fn call_i32(destination: I32Location, function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::CallI32 {
        destination,
        target: CallTarget::same_file(function),
        arguments: i32_arguments(arguments),
    }
}

fn call_usize(
    destination: UsizeLocation,
    function: &str,
    arguments: Vec<ScalarArgument>,
) -> Instruction {
    Instruction::CallUsize {
        destination,
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn call_u8(destination: U8Location, function: &str, arguments: Vec<ScalarArgument>) -> Instruction {
    Instruction::CallU8 {
        destination,
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn call_bool(
    destination: BoolLocation,
    function: &str,
    arguments: Vec<ScalarArgument>,
) -> Instruction {
    Instruction::CallBool {
        destination,
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn call_void(function: &str, arguments: Vec<ScalarArgument>) -> Instruction {
    Instruction::CallVoid {
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn call_str(
    destination: StrLocation,
    function: &str,
    arguments: Vec<ScalarArgument>,
) -> Instruction {
    Instruction::CallStr {
        destination,
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn call_slice(
    destination: SliceLocation,
    function: &str,
    arguments: Vec<ScalarArgument>,
) -> Instruction {
    Instruction::CallSlice {
        destination,
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn function_signatures(signatures: Vec<(&str, Type, Vec<Type>)>) -> context::FunctionSignatures {
    context::FunctionSignatures::from_call_targets(
        signatures
            .into_iter()
            .map(|(name, return_type, parameter_types)| {
                (
                    CallTarget::same_file(name),
                    context::FunctionSignature {
                        return_type,
                        parameter_types: Some(parameter_types),
                    },
                )
            })
            .collect(),
    )
}

fn readonly_u8_slice_type() -> Type {
    Type::Slice {
        is_readwrite: false,
    }
}

fn readwrite_u8_slice_type() -> Type {
    Type::Slice { is_readwrite: true }
}

fn i32_arguments(arguments: Vec<I32Value>) -> Vec<ScalarArgument> {
    arguments.into_iter().map(ScalarArgument::I32).collect()
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

fn u8_const(value: u8) -> U8Value {
    U8Value::Const(value)
}

fn u8_param(index: usize) -> U8Value {
    U8Value::Location(U8Location::Parameter(index))
}

fn u8_local(index: usize) -> U8Value {
    U8Value::Location(U8Location::Local(index))
}

fn usize_const(value: u64) -> UsizeValue {
    UsizeValue::Const(value)
}

fn usize_param(index: usize) -> UsizeValue {
    UsizeValue::Location(UsizeLocation::Parameter(index))
}

fn str_static(bytes: &[u8]) -> ScalarArgument {
    ScalarArgument::Str(str_static_value(bytes))
}

fn str_static_value(bytes: &[u8]) -> StrValue {
    StrValue::StaticBytes(bytes.to_vec())
}

fn usize_local(index: usize) -> UsizeValue {
    UsizeValue::Location(UsizeLocation::Local(index))
}

fn bool_param(index: usize) -> BoolValue {
    BoolValue::Location(BoolLocation::Parameter(index))
}

fn lower_text_diagnostics(text: &str) -> Vec<Diagnostic> {
    lower_text_diagnostics_with_entry(text, crate::entry::DEFAULT_ENTRY_NAME)
}

fn lower_text_diagnostics_with_std_error(text: &str) -> Vec<Diagnostic> {
    let entry_name = crate::entry::DEFAULT_ENTRY_NAME;
    let analysis =
        analyze_text_with_entry_and_nocter_home_files(text, entry_name, &[std_error_file()]);
    match lower_executable_with_entry(&analysis, entry_name) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

fn lower_text_diagnostics_with_entry(text: &str, entry_name: &str) -> Vec<Diagnostic> {
    let analysis = analyze_text_with_entry(text, entry_name);
    match lower_executable_with_entry(&analysis, entry_name) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

fn analyze_text_with_entry(text: &str, entry_name: &str) -> crate::analysis::CompileUnitAnalysis {
    analyze_text_with_entry_and_nocter_home_files(text, entry_name, &[])
}

fn analyze_text_with_entry_and_nocter_home_files(
    text: &str,
    entry_name: &str,
    home_files: &[(&str, &str)],
) -> crate::analysis::CompileUnitAnalysis {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let temp_root = make_temp_project();
    let nocter_home = make_nocter_home(&temp_root);
    write_nocter_home_files(&nocter_home, home_files);
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

fn std_error_file() -> (&'static str, &'static str) {
    (
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

impl Error {
    pub func new(code: ErrorCode, message: &str): Error {
        return new_error(code, message)
    }
}
"#,
    )
}

fn std_io_file() -> (&'static str, &'static str) {
    (
        "std/io.nct",
        r#"from std/io_impl import write_text_raw

pub func print(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    )
}

fn std_io_impl_file() -> (&'static str, &'static str) {
    (
        "targets/arm64-darwin/std/io_impl.nct",
        r#"pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    )
}

fn std_process_file() -> (&'static str, &'static str) {
    (
        "targets/arm64-darwin/std/process.nct",
        r#"from std/os/macos import trap

pub func abort(): never {
    trap()
}
"#,
    )
}

fn std_macos_file() -> (&'static str, &'static str) {
    (
        "targets/arm64-darwin/std/os/macos.nct",
        r#"pub(nocter) primitive trap(): never

pub(nocter) primitive unreachable(): never
"#,
    )
}

fn write_nocter_home_files(home: &Path, files: &[(&str, &str)]) {
    for (relative, text) in files {
        let path = home.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }
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
