use super::*;

#[test]
fn generates_terminal_if_with_false_condition() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::If {
            condition: BoolValue::Const(false),
            then_instructions: vec![set_return_i32(1), Instruction::Return],
            else_instructions: vec![set_return_i32(2), Instruction::Return],
        }],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x03, 0x00, 0x00, 0x14, // b else
            0x20, 0x00, 0x80, 0x52, // movz w0, #1
            0xc0, 0x03, 0x5f, 0xd6, // ret
            0x40, 0x00, 0x80, 0x52, // movz w0, #2
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_while_with_false_condition_and_backward_branch() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::Const(false),
                body_instructions: vec![set_return_i32(1)],
            },
            set_return_i32(2),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x03, 0x00, 0x00, 0x14, // b while end
            0x20, 0x00, 0x80, 0x52, // movz w0, #1
            0xfe, 0xff, 0xff, 0x17, // b while condition
            0x40, 0x00, 0x80, 0x52, // movz w0, #2
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_while_break_to_loop_end() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::Const(true),
                body_instructions: vec![Instruction::Break],
            },
            set_return_i32(2),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x01, 0x00, 0x00, 0x14, // b while end
            0x40, 0x00, 0x80, 0x52, // movz w0, #2
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_while_continue_to_loop_start() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::While {
                condition_instructions: vec![],
                condition: BoolValue::Const(true),
                body_instructions: vec![Instruction::Continue],
            },
            set_return_i32(2),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x00, 0x00, 0x00, 0x14, // b while condition
            0x40, 0x00, 0x80, 0x52, // movz w0, #2
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_terminal_if_with_bool_local_condition() {
    let module = IrModule::new(vec![Function {
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
                then_instructions: vec![set_return_i32(7), Instruction::Return],
                else_instructions: vec![set_return_i32(9), Instruction::Return],
            },
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x29, 0x00, 0x80, 0x52, // movz w9, #1
            0xf0, 0x03, 0x09, 0x2a, // mov w16, w9
            0x1f, 0x02, 0x1f, 0x6b, // cmp w16, #0
            0x60, 0x00, 0x00, 0x54, // b.eq else
            0xe0, 0x00, 0x80, 0x52, // movz w0, #7
            0xc0, 0x03, 0x5f, 0xd6, // ret
            0x20, 0x01, 0x80, 0x52, // movz w0, #9
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_terminal_if_returning_bool() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Bool,
        instructions: vec![Instruction::If {
            condition: BoolValue::Const(false),
            then_instructions: vec![set_return_bool(true), Instruction::Return],
            else_instructions: vec![set_return_bool(false), Instruction::Return],
        }],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x03, 0x00, 0x00, 0x14, // b else
            0x20, 0x00, 0x80, 0x52, // movz w0, #1
            0xc0, 0x03, 0x5f, 0xd6, // ret
            0x00, 0x00, 0x80, 0x52, // movz w0, #0
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_terminal_if_with_i32_equality_condition() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::If {
            condition: BoolValue::I32Comparison {
                operator: I32ComparisonOperator::Equal,
                left: i32_const(1),
                right: i32_const(1),
            },
            then_instructions: vec![set_return_i32(7), Instruction::Return],
            else_instructions: vec![set_return_i32(9), Instruction::Return],
        }],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x31, 0x00, 0x80, 0x52, // movz w17, #1
            0x1f, 0x02, 0x11, 0x6b, // cmp w16, w17
            0x61, 0x00, 0x00, 0x54, // b.ne else
            0xe0, 0x00, 0x80, 0x52, // movz w0, #7
            0xc0, 0x03, 0x5f, 0xd6, // ret
            0x20, 0x01, 0x80, 0x52, // movz w0, #9
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_terminal_if_with_i32_less_condition() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::If {
            condition: BoolValue::I32Comparison {
                operator: I32ComparisonOperator::Less,
                left: i32_const(1),
                right: i32_const(2),
            },
            then_instructions: vec![set_return_i32(7), Instruction::Return],
            else_instructions: vec![set_return_i32(9), Instruction::Return],
        }],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x51, 0x00, 0x80, 0x52, // movz w17, #2
            0x1f, 0x02, 0x11, 0x6b, // cmp w16, w17
            0x6a, 0x00, 0x00, 0x54, // b.ge else
            0xe0, 0x00, 0x80, 0x52, // movz w0, #7
            0xc0, 0x03, 0x5f, 0xd6, // ret
            0x20, 0x01, 0x80, 0x52, // movz w0, #9
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn maps_i32_comparison_operators_to_arm64_conditions() {
    assert_eq!(
        branch_condition_for_true_comparison(I32ComparisonOperator::Equal),
        BranchCondition::Eq
    );
    assert_eq!(
        branch_condition_for_true_comparison(I32ComparisonOperator::NotEqual),
        BranchCondition::Ne
    );
    assert_eq!(
        branch_condition_for_true_comparison(I32ComparisonOperator::Less),
        BranchCondition::Lt
    );
    assert_eq!(
        branch_condition_for_true_comparison(I32ComparisonOperator::LessEqual),
        BranchCondition::Le
    );
    assert_eq!(
        branch_condition_for_true_comparison(I32ComparisonOperator::Greater),
        BranchCondition::Gt
    );
    assert_eq!(
        branch_condition_for_true_comparison(I32ComparisonOperator::GreaterEqual),
        BranchCondition::Ge
    );
}
