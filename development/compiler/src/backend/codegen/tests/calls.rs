use super::*;
use crate::ir::IntegerBinaryOperator;

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
                Instruction::I32Binary {
                    operator: IntegerBinaryOperator::Add,
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
                Instruction::I32Binary {
                    operator: IntegerBinaryOperator::Add,
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
                Instruction::I32Binary {
                    operator: IntegerBinaryOperator::Add,
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
