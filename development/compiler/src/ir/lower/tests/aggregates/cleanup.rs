use super::*;

#[test]
fn lowers_struct_drop_then_owned_fields_in_reverse_declaration_order() {
    let ir = lower_text(
        r#"struct Resource {
    code: i32
}

destruct Resource(&+self) {
    return
}

struct Inner {
    first: Resource
    second: Resource
}

struct Outer {
    marker: i32
    inner: Inner
}

destruct Outer(&+self) {
    return
}

func main(): i32 {
    let outer = Outer {
        marker: 0,
        inner: Inner {
            first: Resource { code: 1 },
            second: Resource { code: 2 },
        },
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
    let drops: Vec<_> = main
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::CallVoid {
                target, arguments, ..
            } if matches!(target, CallTarget::SameFile(name) if name.ends_with(".drop")) => {
                Some((target.clone(), arguments.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        drops,
        vec![
            (
                CallTarget::same_file("Outer.drop"),
                vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
                })],
            ),
            (
                CallTarget::same_file("Resource.drop"),
                vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlotField {
                        slot_index: 0,
                        offset: 8,
                    },
                })],
            ),
            (
                CallTarget::same_file("Resource.drop"),
                vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlotField {
                        slot_index: 0,
                        offset: 4,
                    },
                })],
            ),
        ]
    );
}

#[test]
fn suppresses_scope_end_drop_for_moved_aggregate_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = make_file()
    drop file
    return 0
}

func make_file(): File {
    var file = File { fd: 3 }
    return move file
}
"#,
    );

    let make_file = ir
        .functions
        .iter()
        .find(|function| function.name == "make_file")
        .unwrap();
    assert_eq!(
        make_file.instructions,
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
            Instruction::CopyAggregate {
                destination: AggregateLocation::DirectReturn,
                source: AggregateLocation::Slot(0),
                layout: ValueLayout::new(4, 4),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn suppresses_scope_end_drop_for_moved_aggregate_binding() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    let source = File { fd: 3 }
    let target = move source
    return target.fd
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
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(1),
                source: AggregateLocation::Slot(0),
                layout: ValueLayout::new(4, 4),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Local(0),
                source: AggregateLocation::Slot(1),
                offset: 0,
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("File.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(1),
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
fn lowers_outer_aggregate_move_binding_inside_nonterminal_if_branch_before_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 3 }
    if true {
        var moved = move file
        return moved.fd
    }
    return 0
}
"#,
    );

    let drop_original = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_moved = Instruction::CallVoid {
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
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(1),
                        source: AggregateLocation::Slot(0),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Local(0),
                        source: AggregateLocation::Slot(1),
                        offset: 0,
                    },
                    drop_moved,
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
            drop_original,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_aggregate_move_binding_inside_nonterminal_if_branch_before_return_suffix() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 3 }
    if true {
        var moved = move file
        touch()
        return moved.fd
    }
    return 0
}

func touch(): void {
    return
}
"#,
    );

    let drop_original = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_moved = Instruction::CallVoid {
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
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(1),
                        source: AggregateLocation::Slot(0),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("touch"),
                        arguments: vec![],
                    },
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Local(0),
                        source: AggregateLocation::Slot(1),
                        offset: 0,
                    },
                    drop_moved,
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
            drop_original,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_aggregate_move_assignment_inside_nonterminal_if_branch_before_return_suffix() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var target = File { fd: 1 }
    var source = File { fd: 2 }
    if true {
        target = move source
        touch()
        return target.fd
    }
    return 0
}

func touch(): void {
    return
}
"#,
    );

    let drop_target = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_source = Instruction::CallVoid {
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
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(1),
                offset: 0,
                value: i32_const(2),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    drop_target.clone(),
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(0),
                        source: AggregateLocation::Slot(1),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("touch"),
                        arguments: vec![],
                    },
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Return,
                        source: AggregateLocation::Slot(0),
                        offset: 0,
                    },
                    drop_target.clone(),
                    Instruction::Return,
                ],
                else_instructions: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            drop_source,
            drop_target,
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_outer_aggregate_move_assignment_inside_nonterminal_if_branch_before_nested_return_if() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var target = File { fd: 1 }
    var source = File { fd: 2 }
    if true {
        target = move source
        if choose() {
            return target.fd
        } else {
            return 7
        }
    }
    return 0
}

func choose(): bool {
    return true
}
"#,
    );

    let drop_target = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_source = Instruction::CallVoid {
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
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(1),
                offset: 0,
                value: i32_const(2),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 2,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(2),
                        source: AggregateLocation::Slot(1),
                        layout: ValueLayout::new(4, 4),
                    },
                    drop_target.clone(),
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(0),
                        source: AggregateLocation::Slot(2),
                        layout: ValueLayout::new(4, 4),
                    },
                    call_bool(BoolLocation::Local(0), "choose", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![
                            Instruction::LoadAggregateI32 {
                                destination: I32Location::Local(0),
                                source: AggregateLocation::Slot(0),
                                offset: 0,
                            },
                            drop_target.clone(),
                            Instruction::SetI32 {
                                destination: I32Location::Return,
                                value: i32_local(0),
                            },
                            Instruction::Return,
                        ],
                        else_instructions: vec![
                            Instruction::SetI32 {
                                destination: I32Location::Local(0),
                                value: i32_const(7),
                            },
                            drop_target.clone(),
                            Instruction::SetI32 {
                                destination: I32Location::Return,
                                value: i32_local(0),
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
            drop_source,
            drop_target,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_replacement_drop_without_scope_unwind_before_never_suffix() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var target = File { fd: 1 }
    var source = File { fd: 2 }
    if true {
        target = move source
        abort()
    }
    return 0
}

func abort(): never {
    abort()
}
"#,
    );

    let drop_target = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_source = Instruction::CallVoid {
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
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(1),
                offset: 0,
                value: i32_const(2),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 2,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(2),
                        source: AggregateLocation::Slot(1),
                        layout: ValueLayout::new(4, 4),
                    },
                    drop_target.clone(),
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(0),
                        source: AggregateLocation::Slot(2),
                        layout: ValueLayout::new(4, 4),
                    },
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
            drop_source,
            drop_target,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_aggregate_terminal_if_never_branch_without_scope_cleanup() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = make()
    return file.fd
}

func make(): File {
    if true {
        var temp = File { fd: 2 }
        abort()
    } else {
        return File { fd: 1 }
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
        .find(|function| function.name == "make")
        .unwrap();
    assert_eq!(
        function,
        &Function {
            name: "make".to_string(),
            target: CallTarget::same_file("make"),
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(4, 4),
                words: 1,
            },
            instructions: vec![
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
                            value: i32_const(2),
                        },
                        Instruction::TailCall {
                            target: CallTarget::same_file("abort"),
                            arguments: vec![],
                        },
                    ],
                    else_instructions: vec![Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::DirectReturn,
                        offset: 0,
                        value: i32_const(1),
                    },],
                },
                Instruction::Return
            ],
        }
    );
}

#[test]
fn lowers_branch_local_aggregate_move_assignment_from_outer_before_return_suffix() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var source = File { fd: 2 }
    if true {
        var target = File { fd: 1 }
        target = move source
        touch()
        return target.fd
    }
    return source.fd
}

func touch(): void {
    return
}
"#,
    );

    let drop_source = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let drop_target = Instruction::CallVoid {
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
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(2),
            },
            Instruction::If {
                condition: BoolValue::Const(true),
                then_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(1),
                        offset: 0,
                        value: i32_const(1),
                    },
                    drop_target.clone(),
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(1),
                        source: AggregateLocation::Slot(0),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("touch"),
                        arguments: vec![],
                    },
                    Instruction::LoadAggregateI32 {
                        destination: I32Location::Return,
                        source: AggregateLocation::Slot(1),
                        offset: 0,
                    },
                    drop_target,
                    Instruction::Return,
                ],
                else_instructions: vec![],
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 0,
            },
            drop_source,
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_local_aggregate_move_inside_nonterminal_if_branch() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    if true {
        var file = File { fd: 1 }
        var moved = move file
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
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(1),
                        source: AggregateLocation::Slot(0),
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(1),
                        })],
                    },
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
fn lowers_assignment_to_nonterminal_while_body_local_aggregate_with_replacement_drop() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    while false {
        var file = File { fd: 1 }
        file = File { fd: 2 }
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
                condition: BoolValue::Const(false),
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
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(4, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(1),
                        offset: 0,
                        value: i32_const(2),
                    },
                    Instruction::CallVoid {
                        target: CallTarget::same_file("File.drop"),
                        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::AggregateSlot(0),
                        })],
                    },
                    Instruction::CopyAggregate {
                        destination: AggregateLocation::Slot(0),
                        source: AggregateLocation::Slot(1),
                        layout: ValueLayout::new(4, 4),
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
fn lowers_explicit_aggregate_move_in_terminal_if_condition() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func consume(file: File): bool {
    return true
}

func main(): i32 {
    var file = File { fd: 1 }
    if consume(move file) {
        return 0
    } else {
        return 1
    }
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
            call_bool(
                BoolLocation::Local(0),
                "consume",
                vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(4, 4),
                    words: 1,
                })],
            ),
            Instruction::If {
                condition: BoolValue::Location(BoolLocation::Local(0)),
                then_instructions: vec![Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_const(0),
                }],
                else_instructions: vec![Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_const(1),
                }],
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_explicit_aggregate_move_in_terminal_bool_if_condition() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func consume(file: File): bool {
    return true
}

func pick(): bool {
    var file = File { fd: 1 }
    if consume(move file) {
        return true
    } else {
        return false
    }
}

func main(): i32 {
    if pick() {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let pick = ir
        .functions
        .iter()
        .find(|function| function.name == "pick")
        .unwrap();
    assert_eq!(
        pick.instructions,
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
            call_bool(
                BoolLocation::Local(0),
                "consume",
                vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(4, 4),
                    words: 1,
                })],
            ),
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
            Instruction::Return,
        ],
    );
}

#[test]
fn transfers_scope_end_drop_to_by_value_aggregate_parameter() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 3 }
    consume(move file)
    return 0
}

func consume(file: File): void {
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
                target: CallTarget::same_file("consume"),
                arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(4, 4),
                    words: 1,
                })],
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    );

    let consume = ir
        .functions
        .iter()
        .find(|function| function.name == "consume")
        .unwrap();
    assert_eq!(
        consume.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::DirectParameter { start_index: 0 },
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("File.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
                })],
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn suppresses_scope_end_drop_for_moved_aggregate_tail_return_argument() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 3 }
    return consume(move file)
}

func consume(file: File): i32 {
    return file.fd
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
            Instruction::TailCall {
                target: CallTarget::same_file("consume"),
                arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(4, 4),
                    words: 1,
                })],
            },
        ],
    );

    let consume = ir
        .functions
        .iter()
        .find(|function| function.name == "consume")
        .unwrap();
    assert_eq!(
        consume.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::DirectParameter { start_index: 0 },
                layout: ValueLayout::new(4, 4),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 0,
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("File.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
                })],
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_aggregate_reinitialization_after_explicit_drop_without_replacement_drop() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 1 }
    drop file
    file = File { fd: 42 }
    return file.fd
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
                value: i32_const(1),
            },
            drop_call.clone(),
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(42),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Local(0),
                source: AggregateLocation::Slot(0),
                offset: 0,
            },
            drop_call,
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_local(0),
            },
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_replacement_drop_for_moved_aggregate_assignment() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var source = File { fd: 1 }
    var target = File { fd: 2 }
    target = move source
    return 0
}
"#,
    );

    let drop_target = Instruction::CallVoid {
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
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(1),
                offset: 0,
                value: i32_const(2),
            },
            drop_target.clone(),
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(1),
                source: AggregateLocation::Slot(0),
                layout: ValueLayout::new(4, 4),
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            drop_target,
            Instruction::Return,
        ],
    );
}

#[test]
fn lowers_scope_end_drop_after_staged_aggregate_field_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32! {
    var file = File { fd: 1 }
    file = File { fd: 42 }
    return file.fd
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
                value: i32_const(1),
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: ValueLayout::new(4, 4),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(1),
                offset: 0,
                value: i32_const(42),
            },
            drop_call.clone(),
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::Slot(1),
                layout: ValueLayout::new(4, 4),
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 0,
            },
            drop_call,
            Instruction::ReturnOutcomeSuccess,
        ],
    );
}

#[test]
fn lowers_direct_aggregate_terminal_if_call_return_after_scope_drop() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

struct Pair {
    first: usize
    second: usize
}

func main(): i32 {
    let pair = choose(true)
    return 0
}

func make_pair(first: usize, second: usize): Pair {
    return Pair { first: first, second: second }
}

func choose(flag: bool): Pair {
    var file = File { fd: 3 }
    if flag {
        return make_pair(1, 2)
    } else {
        return make_pair(3, 4)
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
    let pair_layout = ValueLayout::new(16, 8);
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
    assert_eq!(
        function,
        &Function {
            name: "choose".to_string(),
            target: CallTarget::same_file("choose"),
            return_type: Type::DirectAggregate {
                layout: pair_layout,
                words: 2,
            },
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
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Parameter(0)),
                    then_instructions: vec![Instruction::CallDirectAggregate {
                        destination: AggregateLocation::DirectReturn,
                        target: CallTarget::same_file("make_pair"),
                        arguments: vec![
                            ScalarArgument::Usize(usize_const(1)),
                            ScalarArgument::Usize(usize_const(2)),
                        ],
                        layout: pair_layout,
                    }],
                    else_instructions: vec![Instruction::CallDirectAggregate {
                        destination: AggregateLocation::DirectReturn,
                        target: CallTarget::same_file("make_pair"),
                        arguments: vec![
                            ScalarArgument::Usize(usize_const(3)),
                            ScalarArgument::Usize(usize_const(4)),
                        ],
                        layout: pair_layout,
                    }],
                },
                drop_call,
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_direct_aggregate_terminal_if_moved_local_return_after_scope_drop() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

struct Pair {
    first: usize
    second: usize
}

func main(): i32 {
    let pair = choose(true)
    return 0
}

func choose(flag: bool): Pair {
    var file = File { fd: 3 }
    let left = Pair { first: 1, second: 2 }
    let right = Pair { first: 3, second: 4 }
    if flag {
        return move left
    } else {
        return move right
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
    let pair_layout = ValueLayout::new(16, 8);
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
    assert_eq!(
        function,
        &Function {
            name: "choose".to_string(),
            target: CallTarget::same_file("choose"),
            return_type: Type::DirectAggregate {
                layout: pair_layout,
                words: 2,
            },
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
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: pair_layout,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(1),
                    offset: 0,
                    value: usize_const(1),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(1),
                    offset: 8,
                    value: usize_const(2),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 2,
                    layout: pair_layout,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(2),
                    offset: 0,
                    value: usize_const(3),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(2),
                    offset: 8,
                    value: usize_const(4),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Parameter(0)),
                    then_instructions: vec![Instruction::CopyAggregate {
                        destination: AggregateLocation::DirectReturn,
                        source: AggregateLocation::Slot(1),
                        layout: pair_layout,
                    }],
                    else_instructions: vec![Instruction::CopyAggregate {
                        destination: AggregateLocation::DirectReturn,
                        source: AggregateLocation::Slot(2),
                        layout: pair_layout,
                    }],
                },
                drop_call,
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_direct_aggregate_terminal_if_leading_drop_and_void_call_before_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

struct Pair {
    first: usize
    second: usize
}

func main(): i32 {
    let pair = choose(true)
    return 0
}

func choose(flag: bool): Pair {
    var file = File { fd: 3 }
    if flag {
        drop file
        return Pair { first: 1, second: 2 }
    } else {
        touch(&+file)
        return Pair { first: 3, second: 4 }
    }
}

func touch(file: &+File): void {
    return
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let touch_call = Instruction::CallVoid {
        target: CallTarget::same_file("touch"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let pair_layout = ValueLayout::new(16, 8);
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
    assert_eq!(
        function,
        &Function {
            name: "choose".to_string(),
            target: CallTarget::same_file("choose"),
            return_type: Type::DirectAggregate {
                layout: pair_layout,
                words: 2,
            },
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
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Parameter(0)),
                    then_instructions: vec![
                        drop_call.clone(),
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
                    else_instructions: vec![
                        touch_call,
                        Instruction::ReserveAggregateSlot {
                            slot_index: 1,
                            layout: pair_layout,
                        },
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::Slot(1),
                            offset: 0,
                            value: usize_const(3),
                        },
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::Slot(1),
                            offset: 8,
                            value: usize_const(4),
                        },
                        drop_call,
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::DirectReturn,
                            source: AggregateLocation::Slot(1),
                            layout: pair_layout,
                        },
                        Instruction::Return,
                    ],
                },
            ],
        }
    );
}

#[test]
fn lowers_direct_aggregate_terminal_if_branch_local_binding_drop_before_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

struct Pair {
    first: usize
    second: usize
}

func main(): i32 {
    let pair = choose(true)
    return 0
}

func choose(flag: bool): Pair {
    if flag {
        var file = File { fd: 1 }
        return Pair { first: 1, second: 2 }
    } else {
        var file = File { fd: 2 }
        return Pair { first: 3, second: 4 }
    }
}
"#,
    );

    let then_drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let else_drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(1),
        })],
    };
    let pair_layout = ValueLayout::new(16, 8);
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
    assert_eq!(
        function,
        &Function {
            name: "choose".to_string(),
            target: CallTarget::same_file("choose"),
            return_type: Type::DirectAggregate {
                layout: pair_layout,
                words: 2,
            },
            instructions: vec![
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Parameter(0)),
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
                        then_drop_call,
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
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::DirectReturn,
                            offset: 0,
                            value: usize_const(3),
                        },
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::DirectReturn,
                            offset: 8,
                            value: usize_const(4),
                        },
                        else_drop_call,
                    ],
                },
                Instruction::Return
            ],
        }
    );
}

#[test]
fn lowers_direct_aggregate_terminal_if_branch_assignment_before_moved_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = choose(true)
    drop file
    return 0
}

func choose(flag: bool): File {
    var file = File { fd: 1 }
    if flag {
        file = File { fd: 2 }
        return move file
    } else {
        file = File { fd: 3 }
        return move file
    }
}
"#,
    );

    let layout = ValueLayout::new(4, 4);
    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let function = ir
        .functions
        .iter()
        .find(|function| function.name == "choose")
        .unwrap();
    assert_eq!(
        function,
        &Function {
            name: "choose".to_string(),
            target: CallTarget::same_file("choose"),
            return_type: Type::DirectAggregate { layout, words: 1 },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: i32_const(1),
                },
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Parameter(0)),
                    then_instructions: vec![
                        Instruction::ReserveAggregateSlot {
                            slot_index: 1,
                            layout,
                        },
                        Instruction::StoreAggregateI32 {
                            destination: AggregateLocation::Slot(1),
                            offset: 0,
                            value: i32_const(2),
                        },
                        drop_call.clone(),
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::Slot(0),
                            source: AggregateLocation::Slot(1),
                            layout,
                        },
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::DirectReturn,
                            source: AggregateLocation::Slot(0),
                            layout,
                        },
                    ],
                    else_instructions: vec![
                        Instruction::ReserveAggregateSlot {
                            slot_index: 2,
                            layout,
                        },
                        Instruction::StoreAggregateI32 {
                            destination: AggregateLocation::Slot(2),
                            offset: 0,
                            value: i32_const(3),
                        },
                        drop_call,
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::Slot(0),
                            source: AggregateLocation::Slot(2),
                            layout,
                        },
                        Instruction::CopyAggregate {
                            destination: AggregateLocation::DirectReturn,
                            source: AggregateLocation::Slot(0),
                            layout,
                        },
                    ],
                },
                Instruction::Return,
            ],
        }
    );
}
