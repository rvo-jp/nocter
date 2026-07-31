use super::control_flow::branch_condition_for_true_comparison;
use super::*;
use crate::abi::ValueLayout;
use crate::ir::{
    AggregateArgumentSource, AggregateLocation, BoolLocation, BoolValue, BorrowArgument,
    BorrowSource, CallTarget, DirectAggregateArgument, FallibleFailureMode, Function,
    I32ComparisonOperator, I32Location, I32Value, ScalarArgument, SliceElementAddressKind,
    SliceElementIndex, SliceLocation, SliceValue, StrLocation, StrValue, Type, U8Location, U8Value,
    UsizeLocation, UsizeValue,
};
use crate::source::SourceId;
use crate::target::arm64::BranchCondition;

#[test]
fn maps_imported_call_target_to_imported_function_symbol() {
    let source = SourceId::new(9);
    let symbol = FunctionSymbol::from_call_target(&CallTarget::imported(source, "answer"));

    assert_eq!(
        symbol,
        FunctionSymbol::Imported {
            source,
            name: "answer".to_string(),
        }
    );
    assert_eq!(symbol.description(), "answer from source 9");
}

#[test]
fn maps_function_definition_to_same_file_function_symbol() {
    let function = Function {
        name: "answer".to_string(),
        target: crate::ir::CallTarget::same_file("answer".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::Return],
    };

    assert_eq!(
        FunctionSymbol::from_function(&function),
        FunctionSymbol::SameFile("answer".to_string())
    );
}

#[test]
fn maps_imported_function_definition_to_imported_function_symbol() {
    let source = SourceId::new(11);
    let function = Function {
        name: "answer".to_string(),
        target: CallTarget::imported(source, "answer"),
        return_type: Type::I32,
        instructions: vec![Instruction::Return],
    };

    assert_eq!(
        FunctionSymbol::from_function(&function),
        FunctionSymbol::Imported {
            source,
            name: "answer".to_string(),
        }
    );
}

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
        instructions: vec![set_return_i32(7), Instruction::ReturnFallibleSuccess],
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
        instructions: vec![set_return_usize(7), Instruction::ReturnFallibleSuccess],
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
            instructions: vec![Instruction::ReturnFallibleSuccess],
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
            instructions: vec![Instruction::ReturnFallibleSuccess],
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
fn emits_framed_function_prologue_and_return_epilogue() {
    let function = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![set_return_i32(7), Instruction::Return],
    };
    let frame = FunctionFrame::Framed(FrameLayout::for_slot_counts(0, 0).unwrap());
    let mut emitter = EntryEmitter::new();

    emitter.emit_function_with_frame(&function, &frame).unwrap();

    assert_eq!(
        emitter.encoder.finish(),
        vec![
            0xff, 0x43, 0x00, 0xd1, // sub sp, sp, #16
            0xfe, 0x07, 0x00, 0xf9, // str x30, [sp, #8]
            0xe0, 0x00, 0x80, 0x52, // movz w0, #7
            0xfe, 0x07, 0x40, 0xf9, // ldr x30, [sp, #8]
            0xff, 0x43, 0x00, 0x91, // add sp, sp, #16
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn emits_framed_tail_call_epilogue_before_branch() {
    let main = Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![tail_call("answer", vec![])],
    };
    let answer = Function {
        name: "answer".to_string(),
        target: crate::ir::CallTarget::same_file("answer".to_string()),
        return_type: Type::I32,
        instructions: vec![set_return_i32(7), Instruction::Return],
    };
    let frame = FunctionFrame::Framed(FrameLayout::for_slot_counts(0, 0).unwrap());
    let mut emitter = EntryEmitter::new();

    emitter.emit_function_with_frame(&main, &frame).unwrap();
    emitter.emit_function(&answer).unwrap();
    let code = emitter.finish().unwrap();

    assert_eq!(
        code.text,
        vec![
            0xff, 0x43, 0x00, 0xd1, // sub sp, sp, #16
            0xfe, 0x07, 0x00, 0xf9, // str x30, [sp, #8]
            0xfe, 0x07, 0x40, 0xf9, // ldr x30, [sp, #8]
            0xff, 0x43, 0x00, 0x91, // add sp, sp, #16
            0x01, 0x00, 0x00, 0x14, // b answer
            0xe0, 0x00, 0x80, 0x52, // movz w0, #7
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn tail_call_rejects_borrow_arguments() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(7),
                },
                Instruction::TailCall {
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::I32(I32Location::Local(0)),
                    })],
                },
            ],
        },
        Function {
            name: "consume".to_string(),
            target: crate::ir::CallTarget::same_file("consume".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
    ]);

    let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

    assert_eq!(diagnostics[0].code, "E9003");
    assert!(diagnostics[0].message.contains("borrow arguments"));
}

#[test]
fn emits_trap_instruction() {
    let function = Function {
        name: "abort".to_string(),
        target: crate::ir::CallTarget::same_file("abort".to_string()),
        return_type: Type::Never,
        instructions: vec![Instruction::Trap],
    };
    let mut emitter = EntryEmitter::new();

    emitter.emit_function(&function).unwrap();

    assert_eq!(
        emitter.encoder.finish(),
        vec![
            0x00, 0x00, 0x20, 0xd4, // brk #0
        ]
    );
}

#[test]
fn emits_static_str_return_data() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "title".to_string(),
            target: crate::ir::CallTarget::same_file("title".to_string()),
            return_type: Type::Str,
            instructions: vec![
                Instruction::SetStr {
                    destination: StrLocation::Return,
                    value: StrValue::StaticBytes(b"Nocter".to_vec()),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(code.read_only_data, b"Nocter");
}

#[test]
fn generates_framed_i32_normal_call_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                call_i32(I32Location::Return, "answer", vec![]),
                Instruction::Return,
            ],
        },
        Function {
            name: "answer".to_string(),
            target: crate::ir::CallTarget::same_file("answer".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(7), Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0xff, 0x43, 0x00, 0xd1, // sub sp, sp, #16
            0xfe, 0x07, 0x00, 0xf9, // str x30, [sp, #8]
            0x04, 0x00, 0x00, 0x94, // bl answer
            0xfe, 0x07, 0x40, 0xf9, // ldr x30, [sp, #8]
            0xff, 0x43, 0x00, 0x91, // add sp, sp, #16
            0xc0, 0x03, 0x5f, 0xd6, // ret
            0xe0, 0x00, 0x80, 0x52, // movz w0, #7
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_framed_void_normal_call_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                call_void("effect", vec![]),
                set_return_i32(7),
                Instruction::Return,
            ],
        },
        Function {
            name: "effect".to_string(),
            target: crate::ir::CallTarget::same_file("effect".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0xff, 0x43, 0x00, 0xd1, // sub sp, sp, #16
            0xfe, 0x07, 0x00, 0xf9, // str x30, [sp, #8]
            0x05, 0x00, 0x00, 0x94, // bl effect
            0xe0, 0x00, 0x80, 0x52, // movz w0, #7
            0xfe, 0x07, 0x40, 0xf9, // ldr x30, [sp, #8]
            0xff, 0x43, 0x00, 0x91, // add sp, sp, #16
            0xc0, 0x03, 0x5f, 0xd6, // ret
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_slice_normal_call_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: readonly_u8_slice_type(),
            instructions: vec![
                Instruction::CallSlice {
                    destination: SliceLocation::Return,
                    target: CallTarget::same_file("identity"),
                    arguments: vec![ScalarArgument::Slice(SliceValue::Location(
                        SliceLocation::Parameter(0),
                    ))],
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "identity".to_string(),
            target: crate::ir::CallTarget::same_file("identity".to_string()),
            return_type: readonly_u8_slice_type(),
            instructions: vec![
                Instruction::SetSlice {
                    destination: SliceLocation::Return,
                    value: SliceValue::Location(SliceLocation::Parameter(0)),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(!code.text.is_empty());
}

#[test]
fn generates_str_len_value_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
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
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0xe0, 0x03, 0x01, 0xaa])); // mov x0, x1
}

#[test]
fn generates_slice_len_value_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
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
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0xe0, 0x03, 0x01, 0xaa])); // mov x0, x1
}

#[test]
fn generates_slice_index_byte_load_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(1),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x20, 0x6a, 0x70, 0x38])); // ldrb w0, [x17, x16]
}

#[test]
fn generates_slice_index_usize_load_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: Box::new(UsizeValue::Const(1)),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 3),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_adds_x(XReg::X17, XReg::X17, XReg::X16),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_imm(XReg::X0, XReg::X17, 0),
    ));
}

#[test]
fn generates_slice_index_i32_load_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(1),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 2),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_w_reg(WReg::W0, XReg::X17, XReg::X16),
    ));
}

#[test]
fn generates_slice_index_bool_load_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(1),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_ldrb_w_reg(WReg::W0, XReg::X17, XReg::X16),
    ));
}

#[test]
fn generates_stack_passed_slice_index_byte_load_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::SliceIndex {
                        source: SliceLocation::Parameter(8),
                        index: UsizeValue::Const(1),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0xf1, 0x07, 0x40, 0xf9])); // ldr x17, [sp, #8]
    assert!(contains_instruction(&code.text, [0xf1, 0x03, 0x40, 0xf9])); // ldr x17, [sp, #0]
    assert!(contains_instruction(&code.text, [0x20, 0x6a, 0x70, 0x38])); // ldrb w0, [x17, x16]
}

#[test]
fn generates_static_str_index_byte_load_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Return,
                    value: U8Value::StaticStrIndex {
                        bytes: b"Nocter".to_vec(),
                        index: UsizeValue::Const(3),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x20, 0x6a, 0x70, 0x38])); // ldrb w0, [x17, x16]
    assert_eq!(code.read_only_data, b"Nocter");
}

#[test]
fn generates_u8_to_i32_zero_extend_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "first".to_string(),
            target: crate::ir::CallTarget::same_file("first".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: I32Value::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: UsizeValue::Const(1),
                    })),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x20, 0x6a, 0x70, 0x38])); // ldrb w0, [x17, x16]
}

#[test]
fn generates_u8_comparison_from_hand_built_ir() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "check".to_string(),
            target: crate::ir::CallTarget::same_file("check".to_string()),
            return_type: Type::Bool,
            instructions: vec![
                Instruction::SetBool {
                    destination: BoolLocation::Return,
                    value: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: UsizeValue::Const(0),
                        })),
                        right: I32Value::U8ZeroExtend(Box::new(U8Value::Const(0x7F))),
                    },
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x30, 0x6a, 0x70, 0x38])); // ldrb w16, [x17, x16]
    assert!(contains_instruction(&code.text, [0x20, 0x00, 0x80, 0x52])); // mov w0, #1
}

#[test]
fn normal_i32_call_spills_and_reloads_scalar_locals() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: i32_const(40),
                },
                call_i32(I32Location::Local(1), "add_two", vec![i32_local(0)]),
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: i32_local(0),
                    right: i32_local(1),
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "add_two".to_string(),
            target: crate::ir::CallTarget::same_file("add_two".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: i32_param(0),
                    right: i32_const(2),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0xff, 0x83, 0x00, 0xd1, // sub sp, sp, #32
            0xfe, 0x0f, 0x00, 0xf9, // str x30, [sp, #24]
            0x09, 0x05, 0x80, 0x52, // movz w9, #40
            0xe9, 0x03, 0x00, 0xf9, // str x9, [sp, #0]
            0xea, 0x07, 0x00, 0xf9, // str x10, [sp, #8]
            0xf0, 0x03, 0x09, 0x2a, // mov w16, w9
            0xf0, 0x13, 0x00, 0xb9, // str w16, [sp, #16]
            0xe0, 0x13, 0x40, 0xb9, // ldr w0, [sp, #16]
            0x0c, 0x00, 0x00, 0x94, // bl add_two
            0xe9, 0x03, 0x40, 0xf9, // ldr x9, [sp, #0]
            0xea, 0x07, 0x40, 0xf9, // ldr x10, [sp, #8]
            0xea, 0x03, 0x00, 0x2a, // mov w10, w0
            0xf0, 0x03, 0x09, 0x2a, // mov w16, w9
            0xe0, 0x03, 0x0a, 0x2a, // mov w0, w10
            0x00, 0x02, 0x00, 0x2b, // adds w0, w16, w0
            0x47, 0x00, 0x00, 0x54, // b.vc +8
            0x00, 0x00, 0x20, 0xd4, // brk #0
            0xfe, 0x0f, 0x40, 0xf9, // ldr x30, [sp, #24]
            0xff, 0x83, 0x00, 0x91, // add sp, sp, #32
            0xc0, 0x03, 0x5f, 0xd6, // ret
            0xf0, 0x03, 0x00, 0x2a, // mov w16, w0
            0x40, 0x00, 0x80, 0x52, // movz w0, #2
            0x00, 0x02, 0x00, 0x2b, // adds w0, w16, w0
            0x47, 0x00, 0x00, 0x54, // b.vc +8
            0x00, 0x00, 0x20, 0xd4, // brk #0
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn normal_call_can_pass_scalar_parameter_borrow() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("caller"),
                arguments: vec![ScalarArgument::I32(i32_const(7))],
            }],
        },
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
                        ScalarArgument::I32(i32_const(42)),
                    ],
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "choose".to_string(),
            target: crate::ir::CallTarget::same_file("choose".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(42), Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(!code.text.is_empty());
}

#[test]
fn normal_call_can_pass_stack_scalar_parameter_borrow() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
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
                            source: BorrowSource::I32(I32Location::Parameter(8)),
                        }),
                        ScalarArgument::I32(i32_const(42)),
                    ],
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "choose".to_string(),
            target: crate::ir::CallTarget::same_file("choose".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(42), Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(!code.text.is_empty());
}

#[test]
fn aggregate_call_passes_destination_slot_in_x8() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
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
                set_return_i32(0),
                Instruction::Return,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 16,
                    value: UsizeValue::Const(7),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0xe8, 0x03, 0x00, 0x91])); // add x8, sp, #0
    assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
}

#[test]
fn aggregate_return_call_restores_saved_x8_destination() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "forward".to_string(),
            target: crate::ir::CallTarget::same_file("forward".to_string()),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
            instructions: vec![
                Instruction::CallAggregate {
                    destination: AggregateLocation::Return,
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 16,
                    value: UsizeValue::Const(7),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(!contains_instruction(&code.text, [0xe8, 0x03, 0x00, 0x91])); // add x8, sp, #0
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X8, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X8, 0)
    ));
    assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
}

#[test]
fn aggregate_scalar_field_stores_use_field_width() {
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
            return_type: Type::Aggregate {
                layout: ValueLayout::new(32, 8),
            },
            instructions: vec![
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Return,
                    offset: 0,
                    value: U8Value::Const(7),
                },
                Instruction::StoreAggregateBool {
                    destination: AggregateLocation::Return,
                    offset: 1,
                    value: BoolValue::Const(true),
                },
                Instruction::StoreAggregateU16 {
                    destination: AggregateLocation::Return,
                    offset: 2,
                    value: 0xaabb,
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Return,
                    offset: 4,
                    value: I32Value::Const(42),
                },
                Instruction::StoreAggregateU32 {
                    destination: AggregateLocation::Return,
                    offset: 12,
                    value: 0xaabb_ccdd,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 16,
                    value: UsizeValue::Const(11),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W16, XReg::X8, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W16, XReg::X8, 1)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strh_w_imm(WReg::W16, XReg::X8, 2)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W16, XReg::X8, 4)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W16, XReg::X8, 12)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X16, XReg::X8, 16)
    ));
}

#[test]
fn pointer_u8_store_uses_byte_store() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::StoreU8ToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(4),
                value: U8Value::Const(0),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn pointer_i32_store_uses_word_store() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::StoreI32ToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(4),
                value: I32Value::Const(42),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn pointer_usize_store_uses_word_store() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::StoreUsizeToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(8),
                value: UsizeValue::Const(42),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X2, XReg::X0, 0)
    ));
}

#[test]
fn pointer_bool_store_uses_byte_store() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::StoreBoolToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(1),
                value: BoolValue::Const(true),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn pointer_str_store_uses_two_word_stores() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::StoreStrToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(16),
                value: StrValue::StaticBytes(b"arg".to_vec()),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X2, XReg::X0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X3, XReg::X0, 8)
    ));
}

#[test]
fn pointer_aggregate_copy_stores_slot_bytes() {
    let layout = ValueLayout { size: 4, align: 4 };
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout,
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: I32Value::Const(42),
            },
            Instruction::CopyAggregateToPointer {
                pointer: UsizeValue::Const(4096),
                offset: UsizeValue::Const(4),
                source: AggregateLocation::Slot(0),
                layout,
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W16, XReg::X9, 0)
    ));
}

#[test]
fn slice_u8_index_store_uses_byte_store() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreU8ToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: UsizeValue::Const(0),
                    value: U8Value::Const(7),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn slice_i32_index_store_uses_word_store() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreI32ToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: UsizeValue::Const(1),
                    value: I32Value::Const(42),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    let value_offset = instruction_offset(&code.text, &encoded_mov_u32_to_w(WReg::W2, 42))
        .expect("expected i32 store value materialization");
    let scale_offset = instruction_offset(&code.text, &encoded_lsl_x_imm(XReg::X16, XReg::X16, 2))
        .expect("expected i32 slice index scaling");
    assert!(
        value_offset < scale_offset,
        "slice index stores must materialize the assigned value before computing the destination address"
    );
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 2),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn slice_i32_index_borrow_argument_uses_element_address() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "forward".to_string(),
            target: crate::ir::CallTarget::same_file("forward".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("touch"),
                    arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::SliceIndex {
                            source: SliceLocation::Parameter(0),
                            index: SliceElementIndex::Const(1),
                            element: SliceElementAddressKind::I32,
                        },
                    })],
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "touch".to_string(),
            target: crate::ir::CallTarget::same_file("touch".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 2),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_adds_x(XReg::X16, XReg::X8, XReg::X16),
    ));
}

#[test]
fn slice_usize_index_store_uses_word_store() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreUsizeToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: UsizeValue::Const(1),
                    value: UsizeValue::Const(42),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 3),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X2, XReg::X0, 0)
    ));
}

#[test]
fn slice_bool_index_store_uses_byte_store() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreBoolToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: UsizeValue::Const(0),
                    value: BoolValue::Const(true),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_imm(WReg::W2, XReg::X0, 0)
    ));
}

#[test]
fn slice_str_index_store_uses_two_word_stores() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "fill".to_string(),
            target: crate::ir::CallTarget::same_file("fill".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreStrToSliceIndex {
                    destination: SliceLocation::Parameter(0),
                    index: UsizeValue::Const(1),
                    value: StrValue::StaticBytes(b"arg".to_vec()),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 4),
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X2, XReg::X0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X3, XReg::X0, 8)
    ));
}

#[test]
fn open_read_uses_open_syscall() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Fallible(Box::new(Type::I32)),
        instructions: vec![
            Instruction::OpenRead {
                destination: I32Location::Return,
                path: UsizeValue::Const(4096),
                failure_mode: FallibleFailureMode::Trap,
            },
            Instruction::ReturnFallibleSuccess,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_bytes(
        &code.text,
        &encoded_mov_u32_to_w(WReg::W16, DARWIN_OPEN_SYSCALL)
    ));
    assert!(contains_instruction(&code.text, [0x01, 0x10, 0x00, 0xd4])); // svc #0x80
}

#[test]
fn aggregate_scalar_field_stores_to_slot_use_frame_offsets() {
    let layout = ValueLayout::new(8, 4);
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout,
            },
            Instruction::StoreAggregateU8 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: U8Value::Const(7),
            },
            Instruction::StoreAggregateBool {
                destination: AggregateLocation::Slot(0),
                offset: 1,
                value: BoolValue::Const(true),
            },
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 4,
                value: I32Value::Const(42),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W16, 1)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_w_sp(WReg::W16, 4)
    ));
}

#[test]
fn aggregate_scalar_field_loads_from_slot_use_frame_offsets() {
    let layout = ValueLayout::new(16, 8);
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout,
            },
            Instruction::LoadAggregateU8 {
                destination: U8Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 0,
            },
            Instruction::LoadAggregateBool {
                destination: BoolLocation::Return,
                source: AggregateLocation::Slot(0),
                offset: 1,
            },
            Instruction::LoadAggregateI32 {
                destination: I32Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 4,
            },
            Instruction::LoadAggregateUsize {
                destination: UsizeLocation::Return,
                source: AggregateLocation::Slot(0),
                offset: 8,
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldrb_w_sp(WReg::W0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldrb_w_sp(WReg::W0, 1)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_w_sp(WReg::W0, 4)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X0, 8)
    ));
}

#[test]
fn direct_aggregate_parameter_i32_field_load_extracts_register_word() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "code".to_string(),
            target: crate::ir::CallTarget::same_file("code".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    offset: 4,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 32)
    ));
}

#[test]
fn direct_aggregate_parameter_i32_field_load_after_call_reads_spilled_word() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "code".to_string(),
            target: crate::ir::CallTarget::same_file("code".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    offset: 0,
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "effect".to_string(),
            target: crate::ir::CallTarget::same_file("effect".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_mov_w(WReg::W0, WReg::W16)
    ));
}

#[test]
fn direct_aggregate_parameter_i32_field_load_reads_shifted_second_word() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "code".to_string(),
            target: crate::ir::CallTarget::same_file("code".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 2 },
                    offset: 12,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X16, XReg::X3)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 32)
    ));
}

#[test]
fn direct_aggregate_parameter_u8_field_load_masks_selected_byte() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "read".to_string(),
            target: crate::ir::CallTarget::same_file("read".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::LoadAggregateU8 {
                    destination: U8Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 0 },
                    offset: 2,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 16)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 56)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 56)
    ));
}

#[test]
fn direct_aggregate_parameter_u8_field_load_reads_shifted_start_register() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "read".to_string(),
            target: crate::ir::CallTarget::same_file("read".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::LoadAggregateU8 {
                    destination: U8Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 2 },
                    offset: 9,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X16, XReg::X3)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 56)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 56)
    ));
}

#[test]
fn direct_aggregate_parameter_u8_field_load_reads_stack_word() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "read".to_string(),
            target: crate::ir::CallTarget::same_file("read".to_string()),
            return_type: Type::U8,
            instructions: vec![
                Instruction::LoadAggregateU8 {
                    destination: U8Location::Return,
                    source: AggregateLocation::DirectParameter { start_index: 8 },
                    offset: 4,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 32)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsl_x_imm(XReg::X16, XReg::X16, 56)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X16, XReg::X16, 56)
    ));
}

#[test]
fn direct_aggregate_parameter_usize_field_load_reads_stack_word() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "read".to_string(),
            target: crate::ir::CallTarget::same_file("read".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::LoadAggregateUsize {
                    destination: UsizeLocation::Return,
                    source: AggregateLocation::DirectParameter { start_index: 8 },
                    offset: 8,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X0, 8)
    ));
}

#[test]
fn aggregate_range_copy_from_stack_passed_direct_parameter_reads_unaligned_bytes() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(8, 1),
            },
            Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 0,
                source: AggregateLocation::DirectParameter { start_index: 8 },
                source_offset: 4,
                layout: ValueLayout::new(5, 1),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X17, 16)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X17, 24)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_lsr_x_imm(XReg::X17, XReg::X17, 32)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W17, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W17, 4)
    ));
}

#[test]
fn aggregate_range_copy_from_stack_passed_direct_parameter_after_call_uses_spills() {
    let layout = ValueLayout::new(9, 1);
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "identity".to_string(),
            target: crate::ir::CallTarget::same_file("identity".to_string()),
            return_type: Type::DirectAggregate { layout, words: 2 },
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::DirectReturn,
                    destination_offset: 0,
                    source: AggregateLocation::DirectParameter { start_index: 8 },
                    source_offset: 0,
                    layout,
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "effect".to_string(),
            target: crate::ir::CallTarget::same_file("effect".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X0, XReg::X16)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X17, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X1, XReg::X16)
    ));
}

#[test]
fn aggregate_range_copy_to_borrowed_parameter_after_call_uses_spilled_parameter_pointer() {
    let layout = ValueLayout::new(16, 8);
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "set_header".to_string(),
            target: crate::ir::CallTarget::same_file("set_header".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: UsizeValue::Const(7),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: UsizeValue::Const(42),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::Parameter(0),
                    destination_offset: 8,
                    source: AggregateLocation::Slot(0),
                    source_offset: 0,
                    layout,
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "effect".to_string(),
            target: crate::ir::CallTarget::same_file("effect".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X17, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X16, XReg::X17, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_imm(XReg::X16, XReg::X17, 16)
    ));
}

#[test]
fn aggregate_copy_from_slot_to_return_copies_words_to_x8_destination() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "forward".to_string(),
            target: crate::ir::CallTarget::same_file("forward".to_string()),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
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
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Aggregate {
                layout: ValueLayout::new(24, 8),
            },
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 0,
                    value: UsizeValue::Const(7),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X8, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X8, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 8)
    ));
    assert!(contains_instruction(&code.text, [0x10, 0x01, 0x00, 0xf9])); // str x16, [x8, #0]
    assert!(contains_instruction(&code.text, [0x10, 0x09, 0x00, 0xf9])); // str x16, [x8, #16]
}

#[test]
fn aggregate_range_copy_to_direct_return_second_word_is_allowed() {
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
            return_type: Type::DirectAggregate {
                layout: ValueLayout::new(16, 8),
                words: 2,
            },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(1, 1),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: U8Value::Const(42),
                },
                Instruction::CopyAggregateRange {
                    destination: AggregateLocation::DirectReturn,
                    destination_offset: 8,
                    source: AggregateLocation::Slot(0),
                    source_offset: 0,
                    layout: ValueLayout::new(1, 1),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldrb_w_sp(WReg::W16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X1, XReg::X16)
    ));
}

#[test]
fn aggregate_range_copy_to_direct_return_rejects_range_past_second_word() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::DirectAggregate {
            layout: ValueLayout::new(16, 8),
            words: 2,
        },
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(9, 1),
            },
            Instruction::CopyAggregateRange {
                destination: AggregateLocation::DirectReturn,
                destination_offset: 8,
                source: AggregateLocation::Slot(0),
                source_offset: 0,
                layout: ValueLayout::new(9, 1),
            },
            Instruction::Return,
        ],
    }]);

    let diagnostics = generate_arm64_darwin_entry(&module).unwrap_err();

    assert!(
        diagnostics[0]
            .message
            .contains("direct aggregate return range exceeds two ABI words"),
        "{diagnostics:?}"
    );
}

#[test]
fn aggregate_copy_from_slot_to_slot_copies_words_between_stack_slots() {
    let layout = ValueLayout::new(24, 8);
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout,
            },
            Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout,
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(1),
                offset: 0,
                value: UsizeValue::Const(7),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(1),
                offset: 8,
                value: UsizeValue::Const(8),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(1),
                offset: 16,
                value: UsizeValue::Const(9),
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::Slot(1),
                layout,
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 24)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 32)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 40)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 16)
    ));
}

#[test]
fn aggregate_borrow_argument_passes_slot_address() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
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
        },
        Function {
            name: "touch".to_string(),
            target: crate::ir::CallTarget::same_file("touch".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0xf0, 0x23, 0x00, 0x91])); // add x16, sp, #8
    assert!(contains_instruction(&code.text, [0xf0, 0x03, 0x00, 0xf9])); // str x16, [sp, #0]
    assert!(contains_instruction(&code.text, [0xe0, 0x03, 0x40, 0xf9])); // ldr x0, [sp, #0]
}

#[test]
fn direct_aggregate_argument_passes_slot_words() {
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
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(40),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: usize_const(2),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                        layout: ValueLayout::new(16, 8),
                        words: 2,
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

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 16)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X16, 24)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X1, 8)
    ));
}

#[test]
fn direct_aggregate_argument_zero_extends_partial_final_word() {
    let layout = ValueLayout::new(9, 1);
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(0x0102_0304_0506_0708),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: U8Value::Const(0xaa),
                },
                Instruction::CallVoid {
                    target: CallTarget::same_file("consume"),
                    arguments: vec![ScalarArgument::AggregateDirect(DirectAggregateArgument {
                        source: AggregateArgumentSource::Slot(0),
                        layout,
                        words: 2,
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

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldrb_w_sp(WReg::W16, 24)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X1, 8)
    ));
}

#[test]
fn indirect_aggregate_parameter_copy_reads_from_parameter_pointer() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "length".to_string(),
            target: crate::ir::CallTarget::same_file("length".to_string()),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(0),
                    source: AggregateLocation::Parameter(0),
                    layout: ValueLayout::new(24, 8),
                },
                Instruction::LoadAggregateUsize {
                    destination: UsizeLocation::Return,
                    source: AggregateLocation::Slot(0),
                    offset: 8,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_imm(XReg::X16, XReg::X0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_imm(XReg::X16, XReg::X0, 8)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_imm(XReg::X16, XReg::X0, 16)
    ));
}

#[test]
fn borrowed_aggregate_parameter_field_read_loads_from_parameter_pointer() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "code".to_string(),
            target: crate::ir::CallTarget::same_file("code".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Return,
                    source: AggregateLocation::Parameter(0),
                    offset: 4,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_ldr_w_imm(WReg::W0, XReg::X0, 4)
    ));
}

#[test]
fn readwrite_borrowed_aggregate_parameter_field_write_stores_to_parameter_pointer() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "set_code".to_string(),
            target: crate::ir::CallTarget::same_file("set_code".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Parameter(0),
                    offset: 4,
                    value: I32Value::Const(99),
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W16, XReg::X0, 4)
    ));
}

#[test]
fn borrowed_aggregate_parameter_field_write_after_call_uses_spilled_parameter_pointer() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "set_code".to_string(),
            target: crate::ir::CallTarget::same_file("set_code".to_string()),
            return_type: Type::Void,
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("effect"),
                    arguments: vec![],
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Parameter(0),
                    offset: 4,
                    value: I32Value::Const(99),
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "effect".to_string(),
            target: crate::ir::CallTarget::same_file("effect".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X16, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X17, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_str_w_imm(WReg::W16, XReg::X17, 4)
    ));
}

#[test]
fn direct_aggregate_call_result_stores_partial_final_word_to_slot() {
    let layout = ValueLayout::new(9, 1);
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "forward".to_string(),
            target: crate::ir::CallTarget::same_file("forward".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::CallDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout,
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::DirectAggregate { layout, words: 2 },
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(0x0102_0304_0506_0708),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: U8Value::Const(0xaa),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout,
                },
                Instruction::Return,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X0, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W1, 8)
    ));
}

#[test]
fn fallible_aggregate_call_passes_destination_slot_and_checks_status() {
    let aggregate = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "forward".to_string(),
            target: crate::ir::CallTarget::same_file("forward".to_string()),
            return_type: Type::Fallible(Box::new(aggregate.clone())),
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
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Fallible(Box::new(aggregate)),
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Return,
                    offset: 0,
                    value: UsizeValue::Const(7),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0xe8, 0x23, 0x00, 0x91])); // add x8, sp, #8
    assert!(contains_instruction(
        &code.text,
        encoded_ldr_x_sp(XReg::X8, 0)
    ));
    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(&code.text, [0x10, 0x01, 0x00, 0xf9])); // str x16, [x8, #0]
}

#[test]
fn fallible_direct_aggregate_call_stores_success_payload_words() {
    let aggregate = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "forward".to_string(),
            target: crate::ir::CallTarget::same_file("forward".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(16, 8),
                },
                Instruction::CallFallibleDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout: ValueLayout::new(16, 8),
                    failure_mode: FallibleFailureMode::Propagate,
                },
                set_return_i32(0),
                Instruction::ReturnFallibleSuccess,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Fallible(Box::new(aggregate)),
            instructions: vec![
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::DirectReturn,
                    offset: 0,
                    value: UsizeValue::Const(7),
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::DirectReturn,
                    offset: 8,
                    value: UsizeValue::Const(9),
                },
                Instruction::ReturnFallibleSuccess,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(&code.text, [0xe1, 0x03, 0x00, 0xf9])); // str x1, [sp, #0]
    assert!(contains_instruction(&code.text, [0xe2, 0x07, 0x00, 0xf9])); // str x2, [sp, #8]
}

#[test]
fn fallible_direct_aggregate_call_stores_partial_final_payload_word() {
    let layout = ValueLayout::new(9, 1);
    let aggregate = Type::DirectAggregate { layout, words: 2 };
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "forward".to_string(),
            target: crate::ir::CallTarget::same_file("forward".to_string()),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::CallFallibleDirectAggregate {
                    destination: AggregateLocation::Slot(0),
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout,
                    failure_mode: FallibleFailureMode::Propagate,
                },
                set_return_i32(0),
                Instruction::ReturnFallibleSuccess,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Fallible(Box::new(aggregate)),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(0x0102_0304_0506_0708),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: U8Value::Const(0xaa),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout,
                },
                Instruction::ReturnFallibleSuccess,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(
        &code.text,
        encoded_str_x_sp(XReg::X1, 0)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_strb_w_sp(WReg::W2, 8)
    ));
}

#[test]
fn fallible_direct_aggregate_call_forwards_partial_payload_to_direct_return() {
    let layout = ValueLayout::new(9, 1);
    let aggregate = Type::DirectAggregate { layout, words: 2 };
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![set_return_i32(0), Instruction::Return],
        },
        Function {
            name: "forward".to_string(),
            target: crate::ir::CallTarget::same_file("forward".to_string()),
            return_type: aggregate.clone(),
            instructions: vec![
                Instruction::CallFallibleDirectAggregate {
                    destination: AggregateLocation::DirectReturn,
                    target: CallTarget::same_file("make"),
                    arguments: vec![],
                    layout,
                    failure_mode: FallibleFailureMode::Trap,
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "make".to_string(),
            target: crate::ir::CallTarget::same_file("make".to_string()),
            return_type: Type::Fallible(Box::new(aggregate)),
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout,
                },
                Instruction::StoreAggregateUsize {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: usize_const(0x0102_0304_0506_0708),
                },
                Instruction::StoreAggregateU8 {
                    destination: AggregateLocation::Slot(0),
                    offset: 8,
                    value: U8Value::Const(0xaa),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::DirectReturn,
                    source: AggregateLocation::Slot(0),
                    layout,
                },
                Instruction::ReturnFallibleSuccess,
            ],
        },
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X0, XReg::X1)
    ));
    assert!(contains_instruction(
        &code.text,
        encoded_mov_x(XReg::X1, XReg::X2)
    ));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn generated_i32_normal_call_stages_reordered_arguments() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![tail_call("wrapper", vec![i32_const(5), i32_const(42)])],
        },
        Function {
            name: "wrapper".to_string(),
            target: crate::ir::CallTarget::same_file("wrapper".to_string()),
            return_type: Type::I32,
            instructions: vec![
                call_i32(
                    I32Location::Local(0),
                    "second",
                    vec![i32_param(1), i32_param(0)],
                ),
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_local(0),
                },
                Instruction::Return,
            ],
        },
        Function {
            name: "second".to_string(),
            target: crate::ir::CallTarget::same_file("second".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Return,
                    value: i32_param(1),
                },
                Instruction::Return,
            ],
        },
    ]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-reordered-normal-call-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(5));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn generated_str_literal_argument_uses_two_abi_words() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("consume"),
                arguments: vec![
                    ScalarArgument::Str(StrValue::StaticBytes(b"Nocter".to_vec())),
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
    ]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    assert_eq!(code.read_only_data, b"Nocter");
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-str-argument-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(42));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn generated_bool_normal_call_preserves_local_condition() {
    let module = IrModule::new(vec![
        Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                call_bool(BoolLocation::Local(0), "ready", vec![]),
                Instruction::If {
                    condition: BoolValue::Location(BoolLocation::Local(0)),
                    then_instructions: vec![set_return_i32(7), Instruction::Return],
                    else_instructions: vec![set_return_i32(9), Instruction::Return],
                },
            ],
        },
        Function {
            name: "ready".to_string(),
            target: crate::ir::CallTarget::same_file("ready".to_string()),
            return_type: Type::Bool,
            instructions: vec![set_return_bool(true), Instruction::Return],
        },
    ]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-bool-normal-call-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(7));
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
fn generates_same_file_function_call() {
    let module = IrModule::new(vec![
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
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x01, 0x00, 0x00, 0x14, // b answer
            0xe0, 0x00, 0x80, 0x52, // movz w0, #7
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_i32_tail_call_with_arguments_and_add() {
    let module = IrModule::new(vec![
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
    ]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0xff, 0x83, 0x00, 0xd1, // sub sp, sp, #32
            0xfe, 0x0f, 0x00, 0xf9, // str x30, [sp, #24]
            0x90, 0x02, 0x80, 0x52, // movz w16, #20
            0xf0, 0x03, 0x00, 0xb9, // str w16, [sp, #0]
            0xd0, 0x02, 0x80, 0x52, // movz w16, #22
            0xf0, 0x0b, 0x00, 0xb9, // str w16, [sp, #8]
            0xe0, 0x03, 0x40, 0xb9, // ldr w0, [sp, #0]
            0xe1, 0x0b, 0x40, 0xb9, // ldr w1, [sp, #8]
            0xfe, 0x0f, 0x40, 0xf9, // ldr x30, [sp, #24]
            0xff, 0x83, 0x00, 0x91, // add sp, sp, #32
            0x01, 0x00, 0x00, 0x14, // b add
            0xf0, 0x03, 0x00, 0x2a, // mov w16, w0
            0xe0, 0x03, 0x01, 0x2a, // mov w0, w1
            0x00, 0x02, 0x00, 0x2b, // adds w0, w16, w0
            0x47, 0x00, 0x00, 0x54, // b.vc +8
            0x00, 0x00, 0x20, 0xd4, // brk #0
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_i32_local_binding_return() {
    let module = IrModule::new(vec![Function {
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
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x49, 0x05, 0x80, 0x52, // movz w9, #42
            0xe0, 0x03, 0x09, 0x2a, // mov w0, w9
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_i32_local_addition_binding_return() {
    let module = IrModule::new(vec![Function {
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
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert_eq!(
        code.text,
        vec![
            0x04, 0x00, 0x00, 0x94, // bl main
            0x30, 0x00, 0x80, 0x52, // movz w16, #1
            0x10, 0x40, 0xa0, 0x72, // movk w16, #0x0200, lsl #16
            0x01, 0x10, 0x00, 0xd4, // svc #0x80
            0x09, 0x05, 0x80, 0x52, // movz w9, #40
            0xf0, 0x03, 0x09, 0x2a, // mov w16, w9
            0x4a, 0x00, 0x80, 0x52, // movz w10, #2
            0x0a, 0x02, 0x0a, 0x2b, // adds w10, w16, w10
            0x47, 0x00, 0x00, 0x54, // b.vc +8
            0x00, 0x00, 0x20, 0xd4, // brk #0
            0xe0, 0x03, 0x0a, 0x2a, // mov w0, w10
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn generates_i32_addition_with_overflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::AddI32 {
                destination: I32Location::Return,
                left: i32_const(40),
                right: i32_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0x2b])); // adds w0, w16, w0
    assert!(contains_instruction(&code.text, [0x47, 0x00, 0x00, 0x54])); // b.vc +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_i32_subtraction_with_overflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::SubtractI32 {
                destination: I32Location::Return,
                left: i32_const(40),
                right: i32_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0x6b])); // subs w0, w16, w0
    assert!(contains_instruction(&code.text, [0x47, 0x00, 0x00, 0x54])); // b.vc +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_i32_multiplication_with_overflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::MultiplyI32 {
                destination: I32Location::Return,
                left: i32_const(21),
                right: i32_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x11, 0x7e, 0x20, 0x9b])); // smull x17, w16, w0
    assert!(contains_instruction(&code.text, [0x30, 0x7e, 0x40, 0x93])); // sxtw x16, w17
    assert!(contains_instruction(&code.text, [0x3f, 0x02, 0x10, 0xeb])); // cmp x17, x16
    assert!(contains_instruction(&code.text, [0x40, 0x00, 0x00, 0x54])); // b.eq +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_i32_shift_left_with_count_traps() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ShiftLeftI32 {
                destination: I32Location::Return,
                left: i32_const(5),
                right: i32_const(3),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x4a, 0x00, 0x00, 0x54])); // b.ge +8
    assert!(contains_instruction(&code.text, [0x4b, 0x00, 0x00, 0x54])); // b.lt +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x22, 0xc0, 0x1a])); // lslv w0, w16, w0
}

#[test]
fn generates_i32_shift_right_with_count_traps() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ShiftRightI32 {
                destination: I32Location::Return,
                left: i32_const(8),
                right: i32_const(1),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x4a, 0x00, 0x00, 0x54])); // b.ge +8
    assert!(contains_instruction(&code.text, [0x4b, 0x00, 0x00, 0x54])); // b.lt +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x2a, 0xc0, 0x1a])); // asrv w0, w16, w0
}

#[test]
fn generates_i32_division_with_safety_traps() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::DivideI32 {
                destination: I32Location::Return,
                left: i32_const(84),
                right: i32_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x0e, 0xc0, 0x1a])); // sdiv w0, w16, w0
}

#[test]
fn generates_i32_remainder_with_safety_traps() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::RemainderI32 {
                destination: I32Location::Return,
                left: i32_const(85),
                right: i32_const(43),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x11, 0x0e, 0xc0, 0x1a])); // sdiv w17, w16, w0
    assert!(contains_instruction(&code.text, [0x20, 0xc2, 0x00, 0x1b])); // msub w0, w17, w0, w16
}

#[test]
fn generates_u8_addition_with_range_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::AddU8 {
                destination: U8Location::Return,
                left: u8_const(40),
                right: u8_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0x0b])); // add w0, w16, w0
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_u8_subtraction_with_range_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::SubtractU8 {
                destination: U8Location::Return,
                left: u8_const(40),
                right: u8_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0x4b])); // sub w0, w16, w0
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_u8_multiplication_with_range_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::MultiplyU8 {
                destination: U8Location::Return,
                left: u8_const(21),
                right: u8_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x7e, 0x00, 0x1b])); // mul w0, w16, w0
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_u8_division_with_zero_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::DivideU8 {
                destination: U8Location::Return,
                left: u8_const(84),
                right: u8_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0x6b])); // cmp w0, wzr
    assert!(contains_instruction(&code.text, [0x41, 0x00, 0x00, 0x54])); // b.ne +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x0a, 0xc0, 0x1a])); // udiv w0, w16, w0
}

#[test]
fn generates_u8_remainder_with_zero_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::RemainderU8 {
                destination: U8Location::Return,
                left: u8_const(85),
                right: u8_const(43),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0x6b])); // cmp w0, wzr
    assert!(contains_instruction(&code.text, [0x41, 0x00, 0x00, 0x54])); // b.ne +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x11, 0x0a, 0xc0, 0x1a])); // udiv w17, w16, w0
}

#[test]
fn generates_u8_shift_left_with_count_and_range_traps() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::ShiftLeftU8 {
                destination: U8Location::Return,
                left: u8_const(5),
                right: u8_const(3),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x22, 0xc0, 0x1a])); // lslv w0, w16, w0
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_u8_shift_right_with_count_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::U8,
        instructions: vec![
            Instruction::ShiftRightU8 {
                destination: U8Location::Return,
                left: u8_const(8),
                right: u8_const(1),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x26, 0xc0, 0x1a])); // lsrv w0, w16, w0
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_usize_addition_with_overflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::AddUsize {
                destination: UsizeLocation::Return,
                left: usize_const(40),
                right: usize_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0xab])); // adds x0, x16, x0
    assert!(contains_instruction(&code.text, [0x43, 0x00, 0x00, 0x54])); // b.cc +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_usize_subtraction_with_underflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::SubtractUsize {
                destination: UsizeLocation::Return,
                left: usize_const(40),
                right: usize_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x00, 0x02, 0x00, 0xeb])); // subs x0, x16, x0
    assert!(contains_instruction(&code.text, [0x42, 0x00, 0x00, 0x54])); // b.cs +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
}

#[test]
fn generates_usize_multiplication_with_overflow_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::MultiplyUsize {
                destination: UsizeLocation::Return,
                left: usize_const(21),
                right: usize_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x11, 0x7e, 0xc0, 0x9b])); // umulh x17, x16, x0
    assert!(contains_instruction(&code.text, [0x3f, 0x02, 0x1f, 0xeb])); // cmp x17, xzr
    assert!(contains_instruction(&code.text, [0x40, 0x00, 0x00, 0x54])); // b.eq +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x7e, 0x00, 0x9b])); // mul x0, x16, x0
}

#[test]
fn generates_usize_division_with_zero_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::DivideUsize {
                destination: UsizeLocation::Return,
                left: usize_const(84),
                right: usize_const(2),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(&code.text, [0x41, 0x00, 0x00, 0x54])); // b.ne +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x0a, 0xc0, 0x9a])); // udiv x0, x16, x0
}

#[test]
fn generates_usize_remainder_with_zero_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::RemainderUsize {
                destination: UsizeLocation::Return,
                left: usize_const(85),
                right: usize_const(43),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x1f, 0xeb])); // cmp x0, xzr
    assert!(contains_instruction(&code.text, [0x41, 0x00, 0x00, 0x54])); // b.ne +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x11, 0x0a, 0xc0, 0x9a])); // udiv x17, x16, x0
    assert!(contains_instruction(&code.text, [0x20, 0xc2, 0x00, 0x9b])); // msub x0, x17, x0, x16
}

#[test]
fn generates_usize_shift_left_with_count_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::ShiftLeftUsize {
                destination: UsizeLocation::Return,
                left: usize_const(5),
                right: usize_const(3),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x11, 0xeb])); // cmp x0, x17
    assert!(contains_instruction(&code.text, [0x43, 0x00, 0x00, 0x54])); // b.cc +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x22, 0xc0, 0x9a])); // lslv x0, x16, x0
}

#[test]
fn generates_usize_shift_right_with_count_trap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::Usize,
        instructions: vec![
            Instruction::ShiftRightUsize {
                destination: UsizeLocation::Return,
                left: usize_const(8),
                right: usize_const(1),
            },
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(contains_instruction(&code.text, [0x1f, 0x00, 0x11, 0xeb])); // cmp x0, x17
    assert!(contains_instruction(&code.text, [0x43, 0x00, 0x00, 0x54])); // b.cc +8
    assert!(contains_instruction(&code.text, [0x00, 0x00, 0x20, 0xd4])); // brk #0
    assert!(contains_instruction(&code.text, [0x00, 0x26, 0xc0, 0x9a])); // lsrv x0, x16, x0
}

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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn generated_str_write_runs() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::WriteStr {
                fd: I32Value::Const(1),
                text: StrValue::StaticBytes(b"hello\n".to_vec()),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-str-write-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"hello\n");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn generated_slice_write_runs() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::WriteSlice {
                fd: I32Value::Const(1),
                bytes: SliceValue::StrBytes(StrValue::StaticBytes(b"bytes\n".to_vec())),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-slice-write-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"bytes\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn write_str_emits_loop_for_partial_writes() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::WriteStr {
                fd: I32Value::Const(1),
                text: StrValue::StaticBytes(b"hello\n".to_vec()),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);
    let code = generate_arm64_darwin_entry(&module).unwrap();

    assert!(
        contains_backward_unconditional_branch(&code.text),
        "write lowering should loop until the requested byte count is exhausted"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn generated_close_fd_closes_stdout() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::CloseFd {
                fd: I32Value::Const(1),
            },
            Instruction::WriteStr {
                fd: I32Value::Const(1),
                text: StrValue::StaticBytes(b"hidden\n".to_vec()),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-close-fd-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn generated_read_zero_bytes_runs() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: crate::ir::CallTarget::same_file("main".to_string()),
        return_type: Type::I32,
        instructions: vec![
            Instruction::ReadSlice {
                destination: UsizeLocation::Local(0),
                fd: I32Value::Const(0),
                buffer: SliceValue::StrBytes(StrValue::StaticBytes(Vec::new())),
                failure_mode: FallibleFailureMode::Trap,
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);
    let code = generate_arm64_darwin_entry(&module).unwrap();
    let image = crate::target::macho::write_arm64_macos_executable_with_data(
        &code.text,
        &code.read_only_data,
    );
    let executable = write_temp_executable("codegen-read-zero-runs", &image.bytes);

    let output = std::process::Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(executable);

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn write_temp_executable(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let unique = format!(
        "nocter-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let executable = std::env::temp_dir().join(unique);
    std::fs::write(&executable, bytes).unwrap();
    let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions).unwrap();
    executable
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

fn set_return_bool(value: bool) -> Instruction {
    Instruction::SetBool {
        destination: BoolLocation::Return,
        value: BoolValue::Const(value),
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

fn call_void(function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::CallVoid {
        target: CallTarget::same_file(function),
        arguments: i32_arguments(arguments),
    }
}

fn call_bool(destination: BoolLocation, function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::CallBool {
        destination,
        target: CallTarget::same_file(function),
        arguments: i32_arguments(arguments),
    }
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

fn readonly_u8_slice_type() -> Type {
    Type::Slice {
        is_readwrite: false,
    }
}

fn usize_const(value: u64) -> UsizeValue {
    UsizeValue::Const(value)
}

fn contains_instruction(text: &[u8], instruction: [u8; 4]) -> bool {
    text.windows(4).any(|window| window == instruction)
}

fn instruction_offset(text: &[u8], instruction: &[u8]) -> Option<usize> {
    text.windows(instruction.len())
        .position(|window| window == instruction)
}

fn contains_backward_unconditional_branch(text: &[u8]) -> bool {
    text.chunks_exact(4).any(|chunk| {
        let word = u32::from_le_bytes(chunk.try_into().unwrap());
        let is_unconditional_branch = (word & 0xfc00_0000) == 0x1400_0000;
        let offset_is_negative = (word & 0x0200_0000) != 0;
        is_unconditional_branch && offset_is_negative
    })
}

fn contains_bytes(text: &[u8], bytes: &[u8]) -> bool {
    text.windows(bytes.len()).any(|window| window == bytes)
}

fn encoded_mov_u32_to_w(register: WReg, value: u32) -> Vec<u8> {
    let mut encoder = Encoder::new();
    emit_mov_u32_to_w(&mut encoder, register, value);
    encoder.finish()
}

fn encoded_ldr_x_sp(register: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldr_x_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_ldr_x_imm(register: XReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldr_x_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_mov_x(destination: XReg, source: XReg) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_mov_x(destination, source);
    encoded_instruction(encoder)
}

fn encoded_mov_w(destination: WReg, source: WReg) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_mov_w(destination, source);
    encoded_instruction(encoder)
}

fn encoded_lsl_x_imm(destination: XReg, source: XReg, shift: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_lsl_x_imm(destination, source, shift);
    encoded_instruction(encoder)
}

fn encoded_adds_x(destination: XReg, left: XReg, right: XReg) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_adds_x(destination, left, right);
    encoded_instruction(encoder)
}

fn encoded_lsr_x_imm(destination: XReg, source: XReg, shift: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_lsr_x_imm(destination, source, shift);
    encoded_instruction(encoder)
}

fn encoded_ldr_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldr_w_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_ldr_w_reg(register: WReg, base: XReg, offset: XReg) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldr_w_reg(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_ldr_w_sp(register: WReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldr_w_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_ldrb_w_sp(register: WReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldrb_w_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_ldrb_w_reg(register: WReg, base: XReg, offset: XReg) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldrb_w_reg(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_str_x_sp(register: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_str_x_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_str_w_sp(register: WReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_str_w_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_strb_w_sp(register: WReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_strb_w_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_str_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_str_w_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_strb_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_strb_w_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_strh_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_strh_w_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_str_x_imm(register: XReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_str_x_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_instruction(encoder: Encoder) -> [u8; 4] {
    encoder.finish().try_into().unwrap()
}
