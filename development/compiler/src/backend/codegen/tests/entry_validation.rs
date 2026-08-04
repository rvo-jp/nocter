use super::*;

#[test]
fn generates_exit_zero_for_return_i32_zero() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![set_return_i32(0), Instruction::Return],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x00, 0x00, 0x80, 0x52, // movz w0, #0
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_exit_code_for_return_i32_with_high_halfword() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![set_return_i32(0x1234_5678), Instruction::Return],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x00, 0xcf, 0x8a, 0x52, // movz w0, #0x5678
            0x80, 0x46, 0xa2, 0x72, // movk w0, #0x1234, lsl #16
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_exit_code_for_return_i32_negative_one() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![set_return_i32(-1), Instruction::Return],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0xe0, 0xff, 0x9f, 0x52, // movz w0, #0xffff
            0xe0, 0xff, 0xbf, 0x72, // movk w0, #0xffff, lsl #16
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_exit_code_for_fallible_success_return_i32() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Fallible(Box::new(Type::I32)),
        instructions: vec![set_return_i32(7), Instruction::ReturnOutcomeSuccess],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(code.read_only_data, b": \n");
    assert_eq!(
        code.text[code.text.len() - 16..],
        vec![
            0xe0, 0x00, 0x80, 0x52, // movz w0, #7
            0xe1, 0x03, 0x00, 0x2a, // mov w1, w0
            0x00, 0x00, 0x80, 0x52, // movz w0, #0
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_exit_code_for_fallible_success_return_usize() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Fallible(Box::new(Type::Usize)),
        instructions: vec![set_return_usize(7), Instruction::ReturnOutcomeSuccess],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X1, XReg::X0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X0, XReg::X1)
    ));
}

#[test]
fn rejects_direct_aggregate_return_wider_than_two_words_before_codegen() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Fallible(Box::new(Type::DirectAggregate {
                layout: ValueLayout::new(24, 8),
                words: 3,
            })),
            instructions: vec![Instruction::ReturnOutcomeSuccess],
        },
    ]);

    let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

    assert_eq!(diagnostics[0].code, "E9002");
    assert!(
        diagnostics[0]
            .message
            .contains("requires 3 direct ABI words")
    );
}

#[test]
fn rejects_normal_call_to_fallible_callee_before_codegen() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "answer".to_string(),
            target: crate::ir::CallTarget::same_file("answer".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![Instruction::ReturnOutcomeSuccess],
        },
    ]);

    let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

    assert_eq!(diagnostics[0].code, "E9002");
    assert!(
        diagnostics[0]
            .message
            .contains("normal call to function `answer` targets a fallible return")
    );
}

#[test]
fn rejects_normal_call_to_optional_callee_before_codegen() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![Instruction::CallI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("answer"),
                arguments: vec![],
            }],
        },
        Function {
            name: "answer".to_string(),
            target: CallTarget::same_file("answer"),
            return_type: Type::Optional(Box::new(Type::I32)),
            instructions: vec![set_return_i32(7), Instruction::ReturnOutcomeSuccess],
        },
    ]);

    let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

    assert_eq!(diagnostics[0].code, "E9002");
    assert!(
        diagnostics[0]
            .message
            .contains("normal call to function `answer` targets an optional return")
    );
}

#[test]
fn accepts_trapping_outcome_call_to_optional_callee() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallOutcomeI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("answer"),
                    arguments: vec![],
                    failure_mode: OutcomeFailureMode::Trap,
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "answer".to_string(),
            target: CallTarget::same_file("answer"),
            return_type: Type::Optional(Box::new(Type::I32)),
            instructions: vec![set_return_i32(7), Instruction::ReturnOutcomeSuccess],
        },
    ]);

    generate_arm64_darwin_entry(&module).unwrap();
}

#[test]
fn rejects_fallible_only_failure_mode_for_optional_call() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![Instruction::CallOutcomeI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("answer"),
                arguments: vec![],
                failure_mode: OutcomeFailureMode::Catch {
                    code: StrLocation::Local(0),
                    message: StrLocation::Local(1),
                    instructions: vec![Instruction::Trap],
                },
            }],
        },
        Function {
            name: "answer".to_string(),
            target: CallTarget::same_file("answer"),
            return_type: Type::Optional(Box::new(Type::I32)),
            instructions: vec![Instruction::ReturnOptionalNone],
        },
    ]);

    let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot catch optional absence"))
    );
}

#[test]
fn rejects_failure_return_for_optional_function() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "answer".to_string(),
            target: CallTarget::same_file("answer"),
            return_type: Type::Optional(Box::new(Type::I32)),
            instructions: vec![Instruction::ReturnFallibleFailure {
                code: StrValue::StaticBytes(b"E".to_vec()),
                message: StrValue::StaticBytes(b"failed".to_vec()),
            }],
        },
    ]);

    let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires a function return type containing a fallible outcome layer")
    }));
}

#[test]
fn rejects_call_return_shape_mismatch_before_codegen() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("title"),
                    arguments: vec![],
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "title".to_string(),
            target: crate::ir::CallTarget::same_file("title".to_string()),
            return_type: Type::Str,
            instructions: vec![Instruction::Return],
        },
    ]);

    let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

    assert_eq!(diagnostics[0].code, "E9002");
    assert!(diagnostics[0].message.contains("expected i32"));
    assert!(diagnostics[0].message.contains("got &str"));
}

#[test]
fn rejects_direct_aggregate_argument_word_count_mismatch_before_codegen() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                        layout: ValueLayout::new(16, 8),
                        words: 1,
                    })],
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        },
        Function {
            name: "consume".to_string(),
            target: crate::ir::CallTarget::same_file("consume".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        },
    ]);

    let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

    assert_eq!(diagnostics[0].code, "E9002");
    assert!(
        diagnostics[0]
            .message
            .contains("direct aggregate argument uses 1 ABI words")
    );
    assert!(diagnostics[0].message.contains("requires 2"));
}

#[test]
fn generates_exit_code_for_return_bool_true() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Bool,
        instructions: vec![
            Instruction::SetBool {
                destination: BoolLocation::Return,
                value: BoolValue::Const(true),
            },
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
            0x20, 0x00, 0x80, 0x52, // movz w0, #1
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_exit_zero_for_return_void() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Void,
        instructions: vec![Instruction::Return],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x05, 0x00, 0x00, 0x94, // bl main
            0x00, 0x00, 0x80, 0x52, // movz w0, #0
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}
