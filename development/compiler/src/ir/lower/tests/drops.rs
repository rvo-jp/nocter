use super::*;

#[test]
fn does_not_drop_a_direct_owner_before_its_struct_initialization_completes() {
    let ir = lower_text(
        r#"struct Resource {
    fd: i32
    code: i32
}

impl Resource {
    drop &+self {
        return
    }
}

func code(): i32! {
    return 2
}

func main(): i32! {
    let resource = Resource { fd: 1, code: code()? }
    return resource.fd
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CallFallibleI32 {
            target,
            failure_mode: FallibleFailureMode::Propagate,
            ..
        } if target == &CallTarget::same_file("code")
    )));
    assert_eq!(
        main.instructions
            .iter()
            .filter(|instruction| matches!(
                instruction,
                Instruction::CallVoid { target, .. }
                    if target == &CallTarget::same_file("Resource.drop")
            ))
            .count(),
        1
    );
}

#[test]
fn partial_struct_cleanup_drops_fields_without_running_the_outer_destructor() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Bundle {
    file: File
    code: i32
}

impl Bundle {
    drop &+self {
        return
    }
}

func code(): i32! {
    return 2
}

func main(): i32! {
    let bundle = Bundle { file: File { fd: 1 }, code: code()? }
    return 0
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallFallibleI32 {
                target,
                failure_mode: FallibleFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("code") => Some(instructions),
            _ => None,
        });

    assert!(matches!(
        cleanup.map(Vec::as_slice),
        Some([Instruction::If {
            condition: BoolValue::Location(BoolLocation::Local(0)),
            then_instructions,
            else_instructions,
        }]) if else_instructions.is_empty()
            && matches!(
                then_instructions.as_slice(),
                [Instruction::CallVoid { target, arguments }]
                    if target == &CallTarget::same_file("File.drop")
                        && matches!(
                            arguments.as_slice(),
                            [ScalarArgument::Borrow(BorrowArgument {
                                source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 0 }
                            })]
                        )
            )
    ));
}

#[test]
fn lowers_method_call_receiver_as_implicit_readwrite_borrow() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    method &+self.touch(): void! {
        return
    }
}

func main(): i32! {
    var file = File { fd: 1 }
    file.touch()?
    return 0
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(main.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::CallFallibleVoid {
                target,
                arguments,
                failure_mode: FallibleFailureMode::Propagate,
            } if target == &CallTarget::same_file("File.touch")
                && arguments == &vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
                })]
        )
    }));
}

#[test]
fn lowers_method_call_temporary_receiver_as_implicit_readonly_borrow() {
    let ir = lower_text(
        r#"copy struct File {
    fd: i32
}

impl File {
    method &self.value(): i32 {
        return self.fd
    }
}

func main(): i32 {
    return make_file().value()
}

func make_file(): File {
    return File { fd: 42 }
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
                target: CallTarget::same_file("make_file"),
                arguments: vec![],
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CallI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("File.value"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
                })],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_explicit_drop_to_drop_member_call() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 3 }
    drop file
    return 0
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: CallTarget::same_file("main"),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(3),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    set_return_i32(0),
                    Instruction::Return,
                ],
            },
            Function {
                name: "File.drop".to_string(),
                target: CallTarget::same_file("File.drop"),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_concrete_generic_explicit_drop_to_drop_member_call() {
    let ir = lower_text(
        r#"struct Box<T> {
    value: T
}

impl Box<i32> {
    drop &+self {
        return
    }
}

func main(): i32 {
    var box = Box<i32> { value: 3 }
    drop box
    return 0
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: CallTarget::same_file("main"),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(3),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("Box<i32>.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    set_return_i32(0),
                    Instruction::Return,
                ],
            },
            Function {
                name: "Box<i32>.drop".to_string(),
                target: CallTarget::same_file("Box<i32>.drop"),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_imported_explicit_drop_to_imported_drop_member_call() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/file.File

func main(): i32 {
    var file = File { fd: 3 }
    drop file
    return 0
}
"#,
        &[(
            "std/file.nct",
            r#"pub struct File {
    pub fd: i32
}

impl File {
    drop &+self {
        return
    }
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
                    matches!(item, crate::ast::Item::Struct(struct_) if struct_.name == "File")
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
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(3),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::imported(imported_source, "File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    set_return_i32(0),
                    Instruction::Return,
                ],
            },
            Function {
                name: "File.drop".to_string(),
                target: CallTarget::imported(imported_source, "File.drop"),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ])
    );
    assert_ne!(imported_source, root.ast.span.source);
}

#[test]
fn lowers_scope_end_drop_to_drop_member_call() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 3 }
    return 0
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: CallTarget::same_file("main"),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(3),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(0),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "File.drop".to_string(),
                target: CallTarget::same_file("File.drop"),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_concrete_generic_scope_end_drop_to_drop_member_call() {
    let ir = lower_text(
        r#"struct Box<T> {
    value: T
}

impl Box<i32> {
    drop &+self {
        return
    }
}

func main(): i32 {
    var box = Box<i32> { value: 3 }
    return 0
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![
            Function {
                name: "main".to_string(),
                target: CallTarget::same_file("main"),
                return_type: Type::I32,
                instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(3),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(0),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("Box<i32>.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Function {
                name: "Box<i32>.drop".to_string(),
                target: CallTarget::same_file("Box<i32>.drop"),
                return_type: Type::Void,
                instructions: vec![Instruction::Return],
            },
        ])
    );
}

#[test]
fn lowers_generic_impl_drop_with_concrete_self_type_substitutions() {
    let ir = lower_text_with_nocter_home_files(
        r#"use std/box_test.run

func main(): i32 {
    return run()
}
"#,
        &[
            (
                "std/ptr.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive pointee_size<T>(pointer: *T): usize
"#,
            ),
            (
                "std/box_test.nct",
                r#"use std/ptr.from_addr
use std/ptr.pointee_size

struct Box<T> {
    ptr: *T
    size: usize
}

impl<U> Box<U> {
    drop &+self {
        self.size = pointee_size(self.ptr)
        return
    }
}

pub func run(): i32 {
    var box = Box<u8> { ptr: from_addr(1), size: 0 }
    return 0
}
"#,
            ),
        ],
    );

    let run = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .expect("expected run function");
    assert!(
        run.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallVoid {
                target,
                ..
            } if call_target_name_is(target, "Box<u8>.drop")
        )),
        "{:?}",
        run.instructions
    );

    let drop = ir
        .functions
        .iter()
        .find(|function| function.name == "Box<u8>.drop")
        .expect("expected specialized generic drop function");
    assert!(call_target_name_is(&drop.target, "Box<u8>.drop"));
    assert!(
        drop.instructions
            .contains(&Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Parameter(0),
                offset: 8,
                value: UsizeValue::Const(1),
            }),
        "{:?}",
        drop.instructions
    );
    assert!(
        !drop.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallUsize { target, .. } if call_target_name_is(target, "pointee_size")
        )),
        "{:?}",
        drop.instructions
    );
}

#[test]
fn lowers_scope_end_drop_inside_nonterminal_if_branches() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    if true {
        var file = File { fd: 1 }
    } else {
        var file = File { fd: 2 }
    }
    return 0
}
"#,
    );

    let then_drop = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let else_drop = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(1),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(1),
                    },
                    then_drop,
                ],
                else_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(1),
                        offset: 0,
                        value: i32_const(2),
                    },
                    else_drop,
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_explicit_drop_inside_nonterminal_if_branch_without_scope_end_duplicate() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    if true {
        var file = File { fd: 1 }
        drop file
    }
    return 0
}
"#,
    );

    let explicit_drop = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(1),
                    },
                    explicit_drop,
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_explicit_drop_inside_nonterminal_if_branch_before_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    if true {
        drop file
        return 1
    }
    return 0
}
"#,
    );

    let drop_file = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    drop_file.clone(),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_const(1),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_file,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_explicit_drop_inside_nonterminal_if_branch_before_nested_return_if() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    if true {
        drop file
        if false {
            return 1
        } else {
            return 2
        }
    }
    return 0
}
"#,
    );

    let drop_file = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    drop_file.clone(),
                    Instruction::If {
                        condition: BoolValue::Const(false),
                        then_instructions: vec![
                            Instruction::SetI32 {
                                destination: I32Location::Return,
                                value: i32_const(1),
                            },
                            Instruction::Return,
                        ],
                        else_instructions: vec![
                            Instruction::SetI32 {
                                destination: I32Location::Return,
                                value: i32_const(2),
                            },
                            Instruction::Return,
                        ],
                    },
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_file,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_explicit_drop_inside_nonterminal_if_branch_before_return_suffix() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    if true {
        drop file
        touch()
        return 1
    }
    return 0
}

func touch(): void {
    return
}
"#,
    );

    let drop_file = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    drop_file.clone(),
                    Instruction::CallVoid {
                        target: CallTarget::same_file("touch"),
                        arguments: vec![],
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_const(1),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_file,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_explicit_drop_inside_nonterminal_if_branch_before_never_suffix() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    if true {
        drop file
        abort()
    }
    return 0
}

func abort(): never {
    abort()
}
"#,
    );

    let drop_file = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    drop_file.clone(),
                    Instruction::TailCall {
                        target: CallTarget::same_file("abort"),
                        arguments: vec![],
                    },
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_file,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_return_never_expression_with_scope_cleanup() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    return abort()
}

func abort(): never {
    abort()
}
"#,
    );

    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        function,
        &Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(4, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: i32_const(1),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("File.drop"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlot(0),
                    })],
                },
                Instruction::TailCall {
                    target: CallTarget::same_file("abort"),
                    arguments: vec![],
                },
            ],
        }
    );
}

#[test]
fn lowers_terminal_if_never_branch_with_scope_cleanup() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    if true {
        var file = File { fd: 1 }
        abort()
    } else {
        return 0
    }
}

func abort(): never {
    abort()
}
"#,
    );

    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        function,
        &Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(1),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    Instruction::TailCall {
                        target: CallTarget::same_file("abort"),
                        arguments: vec![],
                    },
                ],
                else_instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_const(0),
                    },
                    Instruction::Return,
                ],
            }],
        }
    );
}

#[test]
fn lowers_return_inside_nonterminal_if_branch_with_outer_scope_cleanup() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    if ready() {
        return 7
    }
    return 0
}

func ready(): bool {
    return true
}
"#,
    );

    let drop_file = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            call_bool(BoolLocation::Local(0), "ready", vec![]),
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(7),
                    },
                    drop_file.clone(),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Local(0),
                value: i32_const(0),
            },
            drop_file,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_scope_end_drop_inside_nonterminal_while_body() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    while ready() {
        var file = File { fd: 1 }
        touch()
    }
    return 0
}

func ready(): bool {
    return false
}

func touch(): void {
    return
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
            Instruction::While {
                condition_instructions: vec![call_bool(BoolLocation::Local(0), "ready", vec![],)],
                condition: BoolValue::Location(BoolLocation::Local(0)),
                body_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(1),
                    },
                    call_void("touch", vec![]),
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_scope_end_drop_inside_nonterminal_loop_body() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    loop {
        var file = File { fd: 1 }
        break
    }
    return 0
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
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::Const(true),
                body_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(1),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    Instruction::Break,
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_explicit_drop_inside_nonterminal_while_body_without_scope_end_duplicate() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    while ready() {
        var file = File { fd: 1 }
        drop file
    }
    return 0
}

func ready(): bool {
    return false
}
"#,
    );

    let explicit_drop = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::While {
                condition_instructions: vec![call_bool(BoolLocation::Local(0), "ready", vec![],)],
                condition: BoolValue::Location(BoolLocation::Local(0)),
                body_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(1),
                    },
                    explicit_drop,
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_return_inside_nonterminal_while_body_with_body_scope_cleanup() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    while ready() {
        var file = File { fd: 1 }
        return 7
    }
    return 0
}

func ready(): bool {
    return false
}
"#,
    );

    let drop_file = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::While {
                condition_instructions: vec![call_bool(BoolLocation::Local(0), "ready", vec![],)],
                condition: BoolValue::Location(BoolLocation::Local(0)),
                body_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(1),
                    },
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(7),
                    },
                    drop_file,
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_break_inside_nonterminal_while_body_with_scope_cleanup() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    while ready() {
        var file = File { fd: 1 }
        break
    }
    return 0
}

func ready(): bool {
    return false
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
            Instruction::While {
                condition_instructions: vec![call_bool(BoolLocation::Local(0), "ready", vec![],)],
                condition: BoolValue::Location(BoolLocation::Local(0)),
                body_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(1),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    Instruction::Break,
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_continue_inside_nonterminal_while_body_with_scope_cleanup() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    while ready() {
        var file = File { fd: 1 }
        continue
    }
    return 0
}

func ready(): bool {
    return false
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
            Instruction::While {
                condition_instructions: vec![call_bool(BoolLocation::Local(0), "ready", vec![],)],
                condition: BoolValue::Location(BoolLocation::Local(0)),
                body_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 0,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(0),
                        offset: 0,
                        value: i32_const(1),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    Instruction::Continue,
                ],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_scope_end_drop_before_tail_call() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 3 }
    return answer()
}

func answer(): i32 {
    return 0
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
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(3),
            },
            Instruction::CallI32 {
                destination: I32Location::Local(0),
                target: CallTarget::same_file("answer"),
                arguments: vec![],
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("File.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
                })],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_scope_end_drop_inside_terminal_if_branches() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 3 }
    if true {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(3),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(0),
                    },
                    drop_call.clone(),
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(1),
                    },
                    drop_call,
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
        ],
    );
}

#[test]
fn lowers_branch_explicit_drop_before_terminal_if_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 3 }
    if true {
        drop file
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(3),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![drop_call.clone(), set_return_i32(0), Instruction::Return],
                else_instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(1),
                    },
                    drop_call,
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
        ],
    );
}

#[test]
fn lowers_scope_end_drop_inside_usize_terminal_if_branches() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    let value: usize = choose(true)
    if value == 7 {
        return 0
    } else {
        return 1
    }
}

func choose(flag: bool): usize {
    var file = File { fd: 3 }
    if flag {
        return 7
    } else {
        return 9
    }
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let choose = ir
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
    assert_eq!(
        choose.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(3),
            },
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Parameter(0)),
                then_instructions: vec![
                    Instruction::SetUsize {
                        destination: UsizeLocation::Local(0),
                        value: usize_const(7),
                    },
                    drop_call.clone(),
                    Instruction::SetUsize {
                        destination: UsizeLocation::Return,
                        value: usize_local(0),
                    },
                    Instruction::Return,
                ],
                else_instructions: vec![
                    Instruction::SetUsize {
                        destination: UsizeLocation::Local(0),
                        value: usize_const(9),
                    },
                    drop_call,
                    Instruction::SetUsize {
                        destination: UsizeLocation::Return,
                        value: usize_local(0),
                    },
                    Instruction::Return,
                ],
            },
        ],
    );
}

#[test]
fn lowers_scope_end_drop_inside_void_terminal_if_branches() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): void {
    var file = File { fd: 3 }
    if true {
        return
    } else {
        return
    }
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(3),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![drop_call.clone(), Instruction::Return],
                else_instructions: vec![drop_call, Instruction::Return],
            },
        ],
    );
}

#[test]
fn lowers_scope_end_drop_inside_nested_terminal_if_branches() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 3 }
    if true {
        if false {
            return 0
        } else {
            return 1
        }
    } else {
        return 2
    }
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(3),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![Instruction::If {
                    condition: BoolValue::Const(false),
                    then_instructions: vec![
                        Instruction::SetI32 {
                            destination: I32Location::Local(0),
                            value: i32_const(0),
                        },
                        drop_call.clone(),
                        Instruction::SetI32 {
                            destination: I32Location::Return,
                            value: i32_local(0),
                        },
                        Instruction::Return,
                    ],
                    else_instructions: vec![
                        Instruction::SetI32 {
                            destination: I32Location::Local(0),
                            value: i32_const(1),
                        },
                        drop_call.clone(),
                        Instruction::SetI32 {
                            destination: I32Location::Return,
                            value: i32_local(0),
                        },
                        Instruction::Return,
                    ],
                }],
                else_instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(2),
                    },
                    drop_call,
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
        ],
    );
}

#[test]
fn lowers_branch_explicit_drop_before_nested_terminal_if() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var file = File { fd: 3 }
    if true {
        drop file
        if false {
            return 0
        } else {
            return 1
        }
    } else {
        return 2
    }
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert_eq!(
        main.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(3),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    drop_call.clone(),
                    Instruction::If {
                        condition: BoolValue::Const(false),
                        then_instructions: vec![set_return_i32(0), Instruction::Return],
                        else_instructions: vec![set_return_i32(1), Instruction::Return],
                    },
                ],
                else_instructions: vec![
                    Instruction::SetI32 {
                        destination: I32Location::Local(0),
                        value: i32_const(2),
                    },
                    drop_call,
                    Instruction::SetI32 {
                        destination: I32Location::Return,
                        value: i32_local(0),
                    },
                    Instruction::Return,
                ],
            },
        ],
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
fn lowers_return_call_with_borrow_argument_as_normal_call() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func caller(): i32 {
    let value = 7
    return choose(&value, 42)
}

func choose(value: &i32, code: i32): i32 {
    return code
}
"#,
        "caller",
        function_signatures(vec![(
            "choose",
            Type::I32,
            vec![
                Type::Borrow {
                    is_readwrite: false,
                    inner: Box::new(Type::I32),
                },
                Type::I32,
            ],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "caller".to_string(),
            target: crate::ir::CallTarget::same_file("caller".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: I32Value::Const(7),
                },
                Instruction::CallI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("choose"),
                    arguments: vec![
                        ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::I32(I32Location::Local(0)),
                        }),
                        ScalarArgument::I32(I32Value::Const(42)),
                    ],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_readonly_temporary_scalar_borrow_call_argument() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func caller(): i32 {
    return choose(&answer(), 42)
}

func answer(): i32 {
    return 7
}

func choose(value: &i32, code: i32): i32 {
    return code
}
"#,
        "caller",
        function_signatures(vec![
            ("answer", Type::I32, vec![]),
            (
                "choose",
                Type::I32,
                vec![
                    Type::Borrow {
                        is_readwrite: false,
                        inner: Box::new(Type::I32),
                    },
                    Type::I32,
                ],
            ),
        ]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "caller".to_string(),
            target: crate::ir::CallTarget::same_file("caller".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                },
                Instruction::SetI32 {
                    destination: I32Location::Local(1),
                    value: I32Value::Location(I32Location::Local(0)),
                },
                Instruction::CallI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("choose"),
                    arguments: vec![
                        ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::I32(I32Location::Local(1)),
                        }),
                        ScalarArgument::I32(I32Value::Const(42)),
                    ],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn indexes_scalar_borrow_alias_parameter_for_calls() {
    let ir = lower_text(
        r#"type IntBorrow = &i32

func main(): i32 {
    let value = 7
    return choose(&value, 42)
}

func choose(value: IntBorrow, code: i32): i32 {
    return code
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
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: I32Value::Const(7),
                },
                Instruction::CallI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("choose"),
                    arguments: vec![
                        ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::I32(I32Location::Local(0)),
                        }),
                        ScalarArgument::I32(I32Value::Const(42)),
                    ],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_scalar_parameter_borrow_call_argument() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func caller(value: i32): i32 {
    return choose(&value, 42)
}

func choose(value: &i32, code: i32): i32 {
    return code
}
"#,
        "caller",
        function_signatures(vec![(
            "choose",
            Type::I32,
            vec![
                Type::Borrow {
                    is_readwrite: false,
                    inner: Box::new(Type::I32),
                },
                Type::I32,
            ],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "caller".to_string(),
            target: crate::ir::CallTarget::same_file("caller".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("choose"),
                    arguments: vec![
                        ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::I32(I32Location::Parameter(0)),
                        }),
                        ScalarArgument::I32(I32Value::Const(42)),
                    ],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_void_entry_scope_end_drop_before_implicit_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): void {
    let file = File { fd: 1 }
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
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("File.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
                })],
            },
            Instruction::Return,
        ]
    );
}
