use super::*;

fn assert_runtime_aggregate_drop_state(ir: &IrModule) {
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("fixture must lower main");
    assert!(contains_bool_state(&main.instructions, true), "{main:?}");
    assert!(contains_bool_state(&main.instructions, false), "{main:?}");
    assert!(contains_guarded_drop(&main.instructions), "{main:?}");
}

fn contains_bool_state(instructions: &[Instruction], state: bool) -> bool {
    instructions.iter().any(|instruction| match instruction {
        Instruction::SetBool {
            value: BoolValue::Const(value),
            ..
        } => *value == state,
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => {
            contains_bool_state(then_instructions, state)
                || contains_bool_state(else_instructions, state)
        }
        Instruction::While {
            condition_instructions,
            body_instructions,
            ..
        } => {
            contains_bool_state(condition_instructions, state)
                || contains_bool_state(body_instructions, state)
        }
        _ => false,
    })
}

fn contains_guarded_drop(instructions: &[Instruction]) -> bool {
    instructions.iter().any(|instruction| match instruction {
        Instruction::If {
            condition: BoolValue::Location(_),
            then_instructions,
            else_instructions,
        } if else_instructions.is_empty() => then_instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallVoid { target, .. }
                    if target == &CallTarget::same_file("File.drop")
            )
        }),
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => contains_guarded_drop(then_instructions) || contains_guarded_drop(else_instructions),
        Instruction::While {
            condition_instructions,
            body_instructions,
            ..
        } => {
            contains_guarded_drop(condition_instructions)
                || contains_guarded_drop(body_instructions)
        }
        _ => false,
    })
}

fn assert_checked_edge_aggregate_drop(ir: &IrModule) {
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("fixture must lower main");
    assert!(!contains_bool_state(&main.instructions, true), "{main:?}");
    assert!(!contains_bool_state(&main.instructions, false), "{main:?}");
    assert!(
        main.instructions.iter().any(|instruction| {
            let Instruction::While {
                condition_instructions,
                ..
            } = instruction
            else {
                return false;
            };
            condition_instructions.iter().any(|instruction| {
                let Instruction::If {
                    then_instructions,
                    else_instructions,
                    ..
                } = instruction
                else {
                    return false;
                };
                then_instructions
                    .iter()
                    .chain(else_instructions)
                    .any(|instruction| {
                        matches!(
                            instruction,
                            Instruction::CallVoid { target, .. }
                                if target == &CallTarget::same_file("File.drop")
                        )
                    })
            })
        }),
        "{main:?}"
    );
}

#[test]
fn skips_unreachable_scope_drop_after_terminal_nested_if_in_nonterminal_loop_body() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    loop {
        var file = File { fd: 1 }
        if done() {
            break
        } else {
            continue
        }
    }
    return 0
}

func done(): bool {
    return true
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
                    call_bool(BoolLocation::Local(0), "done", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![
                            Instruction::CallVoid {
                                target: CallTarget::same_file("File.drop"),
                                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                                    source: BorrowSource::AggregateSlot(0),
                                })],
                            },
                            Instruction::Break,
                        ],
                        else_instructions: vec![
                            Instruction::CallVoid {
                                target: CallTarget::same_file("File.drop"),
                                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                                    source: BorrowSource::AggregateSlot(0),
                                })],
                            },
                            Instruction::Continue,
                        ],
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
fn lowers_outer_explicit_drop_inside_nonterminal_while_body() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 1 }
    while false {
        drop file
    }
    return 0
}
"#,
    );

    assert_runtime_aggregate_drop_state(&ir);
}

#[test]
fn lowers_outer_explicit_drop_before_loop_control_even_with_later_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 1 }
    while false {
        drop file
        break
        return 1
    }
    return 0
}
"#,
    );

    assert_runtime_aggregate_drop_state(&ir);
}

#[test]
fn lowers_outer_explicit_drop_before_nested_loop_control_even_with_later_return() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 1 }
    while false {
        drop file
        if true {
            break
        }
        return 1
    }
    return 0
}
"#,
    );

    assert_runtime_aggregate_drop_state(&ir);
}

#[test]
fn lowers_outer_aggregate_move_assignment_before_loop_control_even_with_later_return() {
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
    while false {
        target = move source
        break
        return 1
    }
    return 0
}
"#,
    );

    assert_checked_edge_aggregate_drop(&ir);
}

#[test]
fn lowers_branch_local_aggregate_move_assignment_from_outer_before_loop_control() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var source = File { fd: 2 }
    while false {
        var target = File { fd: 1 }
        target = move source
        break
        return 1
    }
    return 0
}
"#,
    );

    assert_checked_edge_aggregate_drop(&ir);
}

#[test]
fn lowers_outer_aggregate_move_binding_inside_nonterminal_if_branch() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    var file = File { fd: 1 }
    if true {
        var moved = move file
    }
    return 0
}
"#,
    );

    assert_runtime_aggregate_drop_state(&ir);
}

#[test]
fn skips_unreachable_scope_drop_after_terminal_nested_if_in_nonterminal_while_body() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    while ready() {
        var file = File { fd: 1 }
        if done() {
            break
        } else {
            continue
        }
    }
    return 0
}

func ready(): bool {
    return false
}

func done(): bool {
    return true
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
                    call_bool(BoolLocation::Local(0), "done", vec![]),
                    Instruction::If {
                        condition: BoolValue::Location(BoolLocation::Local(0)),
                        then_instructions: vec![
                            Instruction::CallVoid {
                                target: CallTarget::same_file("File.drop"),
                                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                                    source: BorrowSource::AggregateSlot(0),
                                })],
                            },
                            Instruction::Break,
                        ],
                        else_instructions: vec![
                            Instruction::CallVoid {
                                target: CallTarget::same_file("File.drop"),
                                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                                    source: BorrowSource::AggregateSlot(0),
                                })],
                            },
                            Instruction::Continue,
                        ],
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
fn lowers_explicit_aggregate_move_in_nonterminal_while_condition() {
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
    while consume(move file) {
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
        .expect("fixture must lower main");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::While { condition_instructions, .. }
            if condition_instructions.iter().any(|instruction| matches!(
                instruction,
                Instruction::CallBool { target, arguments, .. }
                    if target == &CallTarget::same_file("consume")
                        && matches!(arguments.as_slice(), [ScalarArgument::AggregateDirect(_)])
            ))
    )));
    assert!(!main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CallVoid { target, .. }
            if target == &CallTarget::same_file("File.drop")
    )));
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
        "native lowering cannot lower tail call from function `mirrors_enabled` returning `i32` to function `enabled` returning `bool`"
    );
}

#[test]
fn rejects_normal_call_return_abi_mismatch_during_lowering() {
    let diagnostics = lower_named_function_diagnostics_with_signatures(
        r#"func main(): i32 {
    return 0
}

func answer(): i32 {
    return 1
}

func uses_answer(): i32 {
    let value = answer()
    return value
}
"#,
        "uses_answer",
        context::FunctionSignatures::from_call_targets(HashMap::from([(
            CallTarget::same_file("answer"),
            context::FunctionSignature {
                return_type: Type::I32,
                parameter_types: Some(vec![]),
                parameter_abi_word_count: Some(0),
                success_return_passing: Some(ReturnPassing::Direct { words: 2 }),
            },
        )])),
    );

    assert_eq!(diagnostics[0].code, "E8006");
    assert_eq!(
        diagnostics[0].message,
        "native lowering call return ABI mismatch for function `answer`: expected callee success return to use `1 direct ABI word`, got `2 direct ABI words`"
    );
}

#[test]
fn reports_unsupported_entry_body() {
    let diagnostics = lower_text_diagnostics(
        r#"func main(): void {
    1
    return
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8002");
}

#[test]
fn skips_unreachable_entry_tail_after_return() {
    let ir = lower_text(
        r#"func main(): i32 {
    return 0
    let header: [u8; 2] = [1, 2]
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![set_return_i32(0), Instruction::Return]
    );
}

#[test]
fn skips_unreachable_entry_tail_after_exhaustive_match_statement() {
    let ir = lower_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    match choice {
        Choice.yes {
            return 0
        }

        Choice.no {
            return 1
        }
    }
    let header: [u8; 2] = [1, 2]
    return 2
}
"#,
    );

    assert!(
        ir.functions[0]
            .instructions
            .iter()
            .all(|instruction| !matches!(
                instruction,
                Instruction::ReserveAggregateSlot { .. } | Instruction::StoreAggregateU8 { .. }
            )),
        "{:?}",
        ir.functions[0].instructions
    );
    assert!(
        ir.functions[0]
            .instructions
            .iter()
            .all(|instruction| !matches!(
                instruction,
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::Const(2)
                }
            )),
        "{:?}",
        ir.functions[0].instructions
    );
}

#[test]
fn skips_unreachable_callable_tail_after_return() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return helper()
}

func helper(): i32 {
    return 7
    let header: [u8; 2] = [1, 2]
}
"#,
        "helper",
    );

    assert_eq!(
        function.instructions,
        vec![set_return_i32(7), Instruction::Return]
    );
}
