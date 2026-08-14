use super::*;

#[test]
fn lowers_optional_aggregate_call_propagation() {
    let header_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func forward(): Header? {
    return make()?
}

func make(): Header? {
    return none
}
"#,
        "forward",
        function_signatures(vec![(
            "make",
            Type::Optional(Box::new(header_type)),
            vec![],
        )]),
    )
    .unwrap();

    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallOutcomeDirectAggregate {
                target,
                failure_mode: OutcomeFailureMode::Propagate,
                ..
            } if *target == CallTarget::same_file("make")
        )),
        "{function:?}"
    );
}

#[test]
fn lowers_optional_direct_aggregate_otherwise_return() {
    let aggregate_type = Type::Optional(Box::new(Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func choose(): Header {
    return make() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
}

func make(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "choose",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    let Some(Instruction::CallOutcomeDirectAggregate {
        destination,
        target,
        arguments,
        layout,
        failure_mode: OutcomeFailureMode::Recover { instructions },
    }) = function.instructions.first()
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::DirectReturn);
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert_eq!(*layout, ValueLayout::new(16, 8));
    assert!(instructions.contains(&Instruction::StoreAggregateI32 {
        destination: AggregateLocation::DirectReturn,
        offset: 4,
        value: I32Value::Const(7),
    }));
    assert_eq!(function.instructions.last(), Some(&Instruction::Return));
}

#[test]
fn lowers_optional_indirect_aggregate_otherwise_return() {
    let aggregate_type = Type::Optional(Box::new(Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    return 0
}

func choose(): Triple {
    return make() otherwise { Triple { first: 1, second: 7, third: 3 } }
}

func make(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
}
"#,
        "choose",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    let Some(Instruction::CallOutcomeAggregate {
        destination,
        target,
        arguments,
        failure_mode: OutcomeFailureMode::Recover { instructions },
    }) = function.instructions.first()
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::Return);
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert!(instructions.contains(&Instruction::StoreAggregateUsize {
        destination: AggregateLocation::Return,
        offset: 8,
        value: UsizeValue::Const(7),
    }));
    assert_eq!(function.instructions.last(), Some(&Instruction::Return));
}

#[test]
fn lowers_optional_direct_aggregate_otherwise_return_with_scope_cleanup() {
    let aggregate_type = Type::Optional(Box::new(Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    }));
    let file_type = Type::DirectAggregate {
        layout: ValueLayout::new(4, 4),
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func choose(): Header {
    var file = File { fd: 3 }
    return make() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
}

func make(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "choose",
        function_signatures(vec![
            (
                "File.drop",
                Type::Void,
                vec![Type::Borrow {
                    is_readwrite: true,
                    inner: Box::new(file_type),
                }],
            ),
            ("make", aggregate_type, vec![]),
        ]),
    )
    .unwrap();

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let [
        Instruction::ReserveAggregateSlot {
            slot_index: 0,
            layout: ValueLayout { size: 4, align: 4 },
        },
        Instruction::StoreAggregateI32 {
            destination: AggregateLocation::Slot(0),
            offset: 0,
            value: I32Value::Const(3),
        },
        Instruction::ReserveAggregateSlot {
            slot_index: 1,
            layout: ValueLayout { size: 16, align: 8 },
        },
        Instruction::CallOutcomeDirectAggregate {
            destination: AggregateLocation::Slot(1),
            target,
            arguments,
            layout,
            failure_mode: OutcomeFailureMode::Recover { instructions },
        },
        top_drop,
        Instruction::CopyAggregate {
            destination: AggregateLocation::DirectReturn,
            source: AggregateLocation::Slot(1),
            layout: ValueLayout { size: 16, align: 8 },
        },
        Instruction::Return,
    ] = function.instructions.as_slice()
    else {
        panic!("{function:?}");
    };
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert_eq!(*layout, ValueLayout::new(16, 8));
    assert_eq!(top_drop, &drop_call);
    assert!(instructions.contains(&Instruction::StoreAggregateI32 {
        destination: AggregateLocation::Slot(1),
        offset: 4,
        value: i32_const(7),
    }));
}

#[test]
fn lowers_optional_direct_aggregate_otherwise_fallible_return_with_scope_cleanup() {
    let header_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    };
    let file_type = Type::DirectAggregate {
        layout: ValueLayout::new(4, 4),
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func choose(): Header! {
    var file = File { fd: 3 }
    return make() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
}

func make(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "choose",
        function_signatures(vec![
            (
                "File.drop",
                Type::Void,
                vec![Type::Borrow {
                    is_readwrite: true,
                    inner: Box::new(file_type),
                }],
            ),
            (
                "make",
                Type::Optional(Box::new(header_type.clone())),
                vec![],
            ),
        ]),
    )
    .unwrap();

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let copy_to_return = Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(1),
        layout: ValueLayout::new(16, 8),
    };
    let Some(Instruction::CallOutcomeDirectAggregate {
        destination,
        failure_mode: OutcomeFailureMode::Handle { instructions },
        ..
    }) = function
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallOutcomeDirectAggregate { .. }))
    else {
        panic!("{function:?}");
    };

    assert_eq!(*destination, AggregateLocation::Slot(1));
    assert!(instructions.as_slice().ends_with(&[
        drop_call.clone(),
        copy_to_return.clone(),
        Instruction::ReturnOutcomeSuccess,
    ]));
    assert!(function.instructions.ends_with(&[
        drop_call,
        copy_to_return,
        Instruction::ReturnOutcomeSuccess,
    ]));
}

#[test]
fn lowers_optional_indirect_aggregate_otherwise_return_with_scope_cleanup() {
    let aggregate_type = Type::Optional(Box::new(Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    }));
    let file_type = Type::DirectAggregate {
        layout: ValueLayout::new(4, 4),
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    return 0
}

func choose(): Triple {
    var file = File { fd: 3 }
    return make() otherwise { Triple { first: 1, second: 7, third: 3 } }
}

func make(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
}
"#,
        "choose",
        function_signatures(vec![
            (
                "File.drop",
                Type::Void,
                vec![Type::Borrow {
                    is_readwrite: true,
                    inner: Box::new(file_type),
                }],
            ),
            ("make", aggregate_type, vec![]),
        ]),
    )
    .unwrap();

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let [
        Instruction::ReserveAggregateSlot {
            slot_index: 0,
            layout: ValueLayout { size: 4, align: 4 },
        },
        Instruction::StoreAggregateI32 {
            destination: AggregateLocation::Slot(0),
            offset: 0,
            value: I32Value::Const(3),
        },
        Instruction::ReserveAggregateSlot {
            slot_index: 1,
            layout: ValueLayout { size: 24, align: 8 },
        },
        Instruction::CallOutcomeAggregate {
            destination: AggregateLocation::Slot(1),
            target,
            arguments,
            failure_mode: OutcomeFailureMode::Recover { instructions },
        },
        top_drop,
        Instruction::CopyAggregate {
            destination: AggregateLocation::Return,
            source: AggregateLocation::Slot(1),
            layout: ValueLayout { size: 24, align: 8 },
        },
        Instruction::Return,
    ] = function.instructions.as_slice()
    else {
        panic!("{function:?}");
    };
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert_eq!(top_drop, &drop_call);
    assert!(instructions.contains(&Instruction::StoreAggregateUsize {
        destination: AggregateLocation::Slot(1),
        offset: 8,
        value: usize_const(7),
    }));
}

#[test]
fn lowers_optional_direct_aggregate_otherwise_return_call_binding() {
    let aggregate_type = Type::Optional(Box::new(Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = make() otherwise { return 7 }

    return header.code
}

func make(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallOutcomeDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(16, 8),
                failure_mode: OutcomeFailureMode::Handle {
                    instructions: vec![set_return_i32(7), Instruction::Return],
                },
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_optional_indirect_aggregate_otherwise_return_call_binding() {
    let aggregate_type = Type::Optional(Box::new(Type::Aggregate {
        layout: ValueLayout::new(32, 8),
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Packet {
    prefix: usize
    header: Header
    tail: usize
}

func main(): i32 {
    let packet = make() otherwise { return 7 }

    return packet.header.code
}

func make(): Packet? {
    return Packet { prefix: 1, header: Header { tag: 7, ok: true, code: 42, len: 11 }, tail: 2 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::CallOutcomeAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
                failure_mode: OutcomeFailureMode::Handle {
                    instructions: vec![set_return_i32(7), Instruction::Return],
                },
            }),
        "{function:?}"
    );
}

#[test]
fn lowers_optional_direct_aggregate_otherwise_call_binding() {
    let aggregate_type = Type::Optional(Box::new(Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let header = make() otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
    return header.code
}

func make(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    let Some(Instruction::CallOutcomeDirectAggregate {
        destination,
        target,
        arguments,
        layout,
        failure_mode: OutcomeFailureMode::Recover { instructions },
    }) = function
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallOutcomeDirectAggregate { .. }))
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::Slot(0));
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert_eq!(*layout, ValueLayout::new(16, 8));
    assert!(instructions.contains(&Instruction::StoreAggregateI32 {
        destination: AggregateLocation::Slot(0),
        offset: 4,
        value: I32Value::Const(7),
    }));
}

#[test]
fn lowers_optional_indirect_aggregate_otherwise_call_binding() {
    let aggregate_type = Type::Optional(Box::new(Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = make() otherwise { Triple { first: 1, second: 7, third: 3 } }
    if value.second == 42 {
        return 42
    } else {
        return 7
    }
}

func make(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    let Some(Instruction::CallOutcomeAggregate {
        destination,
        target,
        arguments,
        failure_mode: OutcomeFailureMode::Recover { instructions },
    }) = function
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallOutcomeAggregate { .. }))
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::Slot(0));
    assert_eq!(*target, CallTarget::same_file("make"));
    assert!(arguments.is_empty());
    assert!(instructions.contains(&Instruction::StoreAggregateUsize {
        destination: AggregateLocation::Slot(0),
        offset: 8,
        value: UsizeValue::Const(7),
    }));
}

#[test]
fn lowers_optional_direct_aggregate_otherwise_call_binding_from_copy_local_fallback() {
    let aggregate_type = Type::Optional(Box::new(Type::DirectAggregate {
        layout: ValueLayout::new(16, 8),
        words: 2,
    }));
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    let fallback = Header { tag: 1, ok: false, code: 7, len: 2 }
    let header = make() otherwise { fallback }
    return header.code
}

func make(): Header? {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
        "main",
        function_signatures(vec![("make", aggregate_type, vec![])]),
    )
    .unwrap();

    let Some(Instruction::CallOutcomeDirectAggregate {
        destination,
        failure_mode: OutcomeFailureMode::Recover { instructions },
        ..
    }) = function
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallOutcomeDirectAggregate { .. }))
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::Slot(1));
    assert!(instructions.contains(&Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(1),
        destination_offset: 0,
        source: AggregateLocation::Slot(0),
        source_offset: 0,
        layout: ValueLayout::new(16, 8),
    }));
}

#[test]
fn lowers_optional_indirect_aggregate_otherwise_call_binding_from_call_fallback() {
    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let value = make() otherwise { fallback() }
    if value.second == 42 {
        return 42
    } else {
        return 7
    }
}

func make(): Triple? {
    return Triple { first: 1, second: 42, third: 3 }
}

func fallback(): Triple {
    return Triple { first: 1, second: 7, third: 3 }
}
"#,
        "main",
        function_signatures(vec![
            (
                "make",
                Type::Optional(Box::new(aggregate_type.clone())),
                vec![],
            ),
            ("fallback", aggregate_type, vec![]),
        ]),
    )
    .unwrap();

    let Some(Instruction::CallOutcomeAggregate {
        destination,
        failure_mode: OutcomeFailureMode::Recover { instructions },
        ..
    }) = function
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallOutcomeAggregate { .. }))
    else {
        panic!("{function:?}");
    };
    assert_eq!(*destination, AggregateLocation::Slot(0));
    assert!(instructions.contains(&Instruction::CallAggregate {
        destination: AggregateLocation::Slot(1),
        target: CallTarget::same_file("fallback"),
        arguments: vec![],
    }));
    assert!(instructions.contains(&Instruction::CopyAggregateRange {
        destination: AggregateLocation::Slot(0),
        destination_offset: 0,
        source: AggregateLocation::Slot(1),
        source_offset: 0,
        layout: ValueLayout::new(24, 8),
    }));
}

#[test]
fn lowers_optional_aggregate_otherwise_value_arguments() {
    let header_layout = ValueLayout::new(16, 8);
    let triple_layout = ValueLayout::new(24, 8);
    let pair_layout = ValueLayout::new(8, 4);
    let header_type = Type::DirectAggregate {
        layout: header_layout,
        words: 2,
    };
    let triple_type = Type::Aggregate {
        layout: triple_layout,
    };
    let pair_type = Type::DirectAggregate {
        layout: pair_layout,
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: usize
    second: usize
    third: usize
}

func main(): i32 {
    let header_score = consume_header(maybe_header(false) otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } })
    let fallback = Triple { first: 2, second: 8, third: 4 }
    let triple_score = consume_triple(maybe_triple(false) otherwise { fallback })
    let pair_score = sum_pair(maybe_pair(false) otherwise { [3, 4] })
    return header_score + triple_score + pair_score
}

func consume_header(header: Header): i32 {
    return header.code
}

func consume_triple(triple: Triple): i32 {
    if triple.second == 8 {
        return 8
    }
    return 1
}

func sum_pair(pair: [i32; 2]): i32 {
    return pair[0] + pair[1]
}

func maybe_header(flag: bool): Header? {
    return none
}

func maybe_triple(flag: bool): Triple? {
    return none
}

func maybe_pair(flag: bool): [i32; 2]? {
    return none
}
"#,
        "main",
        function_signatures(vec![
            (
                "consume_header",
                Type::I32,
                vec![header_type.clone()],
            ),
            ("consume_triple", Type::I32, vec![triple_type.clone()]),
            ("sum_pair", Type::I32, vec![pair_type.clone()]),
            (
                "maybe_header",
                Type::Optional(Box::new(header_type.clone())),
                vec![Type::Bool],
            ),
            (
                "maybe_triple",
                Type::Optional(Box::new(triple_type.clone())),
                vec![Type::Bool],
            ),
            (
                "maybe_pair",
                Type::Optional(Box::new(pair_type.clone())),
                vec![Type::Bool],
            ),
        ]),
    )
    .unwrap();

    let header_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallOutcomeDirectAggregate {
            destination,
            target,
            layout,
            failure_mode: OutcomeFailureMode::Recover { instructions },
            ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_header") && *layout == header_layout)
            .then_some((*destination, instructions))
    });
    let Some((header_destination, header_fallback)) = header_call else {
        panic!("{function:?}");
    };
    let AggregateLocation::Slot(header_slot) = header_destination else {
        panic!("{function:?}");
    };
    assert!(header_fallback.contains(&Instruction::StoreAggregateI32 {
        destination: header_destination,
        offset: 4,
        value: i32_const(7),
    }));
    let consume_header_arguments = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallI32 {
            target, arguments, ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("consume_header")).then_some(arguments)
    });
    let Some(consume_header_arguments) = consume_header_arguments else {
        panic!("{function:?}");
    };
    assert_eq!(consume_header_arguments.len(), 1, "{function:?}");
    assert!(matches!(
        &consume_header_arguments[0],
        ScalarArgument::AggregateDirect(DirectAggregateArgument {
            source: AggregateArgumentSource::Slot(slot),
            layout,
            words,
        }) if *slot == header_slot && *layout == header_layout && *words == 2
    ));

    let triple_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallOutcomeAggregate {
            destination,
            target,
            failure_mode: OutcomeFailureMode::Recover { instructions },
            ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_triple")).then_some((*destination, instructions))
    });
    let Some((triple_destination, triple_fallback)) = triple_call else {
        panic!("{function:?}");
    };
    assert!(triple_fallback.iter().any(|instruction| matches!(
        instruction,
        Instruction::CopyAggregateRange {
            destination,
            destination_offset: 0,
            source,
            source_offset: 0,
            layout,
        } if *destination == triple_destination
            && *source == AggregateLocation::Slot(1)
            && *layout == triple_layout
    )));

    let pair_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallOutcomeDirectAggregate {
            destination,
            target,
            layout,
            failure_mode: OutcomeFailureMode::Recover { instructions },
            ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_pair") && *layout == pair_layout)
            .then_some((*destination, instructions))
    });
    let Some((pair_destination, pair_fallback)) = pair_call else {
        panic!("{function:?}");
    };
    assert!(pair_fallback.contains(&Instruction::StoreAggregateI32 {
        destination: pair_destination,
        offset: 4,
        value: i32_const(4),
    }));
}

#[test]
fn lowers_optional_aggregate_otherwise_struct_literal_fields() {
    let header_layout = ValueLayout::new(16, 8);
    let triple_layout = ValueLayout::new(24, 8);
    let pair_layout = ValueLayout::new(8, 4);
    let packet_layout = ValueLayout::new(56, 8);
    let header_type = Type::DirectAggregate {
        layout: header_layout,
        words: 2,
    };
    let triple_type = Type::Aggregate {
        layout: triple_layout,
    };
    let pair_type = Type::DirectAggregate {
        layout: pair_layout,
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: usize
    second: usize
    third: usize
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
    pair: [i32; 2]
}

func main(): i32 {
    let fallback = Triple { first: 2, second: 8, third: 4 }
    let packet = Packet {
        prefix: 1,
        header: maybe_header(false) otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } },
        triple: maybe_triple(false) otherwise { fallback },
        pair: maybe_pair(false) otherwise { [3, 4] },
    }
    return packet.header.code + packet.pair[1]
}

func maybe_header(flag: bool): Header? {
    return none
}

func maybe_triple(flag: bool): Triple? {
    return none
}

func maybe_pair(flag: bool): [i32; 2]? {
    return none
}
"#,
        "main",
        function_signatures(vec![
            (
                "maybe_header",
                Type::Optional(Box::new(header_type.clone())),
                vec![Type::Bool],
            ),
            (
                "maybe_triple",
                Type::Optional(Box::new(triple_type.clone())),
                vec![Type::Bool],
            ),
            (
                "maybe_pair",
                Type::Optional(Box::new(pair_type.clone())),
                vec![Type::Bool],
            ),
        ]),
    )
    .unwrap();

    assert!(
        function
            .instructions
            .contains(&Instruction::ReserveAggregateSlot {
                slot_index: 1,
                layout: packet_layout,
            }),
        "{function:?}"
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CallOutcomeDirectAggregate {
                destination: AggregateLocation::Slot(2),
                target: CallTarget::same_file("maybe_header"),
                arguments: vec![ScalarArgument::Bool(BoolValue::Const(false))],
                layout: header_layout,
                failure_mode: OutcomeFailureMode::Recover {
                    instructions: vec![
                        Instruction::StoreAggregateU8 {
                            destination: AggregateLocation::Slot(2),
                            offset: 0,
                            value: u8_const(1),
                        },
                        Instruction::StoreAggregateBool {
                            destination: AggregateLocation::Slot(2),
                            offset: 1,
                            value: BoolValue::Const(false),
                        },
                        Instruction::StoreAggregateI32 {
                            destination: AggregateLocation::Slot(2),
                            offset: 4,
                            value: i32_const(7),
                        },
                        Instruction::StoreAggregateUsize {
                            destination: AggregateLocation::Slot(2),
                            offset: 8,
                            value: usize_const(2),
                        },
                    ],
                },
            })
    );
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 8,
                source: AggregateLocation::Slot(2),
                source_offset: 0,
                layout: header_layout,
            })
    );

    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CallOutcomeAggregate {
            destination: AggregateLocation::Slot(3),
            target,
            failure_mode: OutcomeFailureMode::Recover { instructions },
            ..
        } if target == &CallTarget::same_file("maybe_triple")
            && instructions.contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(3),
                destination_offset: 0,
                source: AggregateLocation::Slot(0),
                source_offset: 0,
                layout: triple_layout,
            })
    )));
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 24,
                source: AggregateLocation::Slot(3),
                source_offset: 0,
                layout: triple_layout,
            })
    );

    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CallOutcomeDirectAggregate {
            destination: AggregateLocation::Slot(4),
            target,
            layout,
            failure_mode: OutcomeFailureMode::Recover { instructions },
            ..
        } if target == &CallTarget::same_file("maybe_pair")
            && *layout == pair_layout
            && instructions.contains(&Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(4),
                offset: 4,
                value: i32_const(4),
            })
    )));
    assert!(
        function
            .instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(1),
                destination_offset: 48,
                source: AggregateLocation::Slot(4),
                source_offset: 0,
                layout: pair_layout,
            })
    );
}

#[test]
fn lowers_optional_aggregate_otherwise_assignments() {
    let header_layout = ValueLayout::new(16, 8);
    let triple_layout = ValueLayout::new(20, 4);
    let header_type = Type::DirectAggregate {
        layout: header_layout,
        words: 2,
    };
    let triple_type = Type::Aggregate {
        layout: triple_layout,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: i32
    second: i32
    third: i32
    fourth: i32
    fifth: i32
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
}

func main(): i32 {
    var header = Header { tag: 0, ok: false, code: 0, len: 0 }
    let fallback = Triple { first: 2, second: 8, third: 1, fourth: 1, fifth: 4 }
    var packet = Packet {
        prefix: 5,
        header: Header { tag: 3, ok: false, code: 3, len: 3 },
        triple: Triple { first: 1, second: 1, third: 1, fourth: 1, fifth: 1 },
    }
    header = maybe_header(false) otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
    packet.header = maybe_header(true) otherwise { Header { tag: 9, ok: false, code: 90, len: 9 } }
    packet.triple = maybe_triple(false) otherwise { fallback }
    return header.code + packet.header.code + packet.triple.second
}

func maybe_header(flag: bool): Header? {
    return none
}

func maybe_triple(flag: bool): Triple? {
    return none
}
"#,
        "main",
        function_signatures(vec![
            (
                "maybe_header",
                Type::Optional(Box::new(header_type.clone())),
                vec![Type::Bool],
            ),
            (
                "maybe_triple",
                Type::Optional(Box::new(triple_type.clone())),
                vec![Type::Bool],
            ),
        ]),
    )
    .unwrap();

    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallOutcomeDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target,
                arguments,
                layout,
                failure_mode: OutcomeFailureMode::Recover { instructions },
            } if target == &CallTarget::same_file("maybe_header")
                && arguments.as_slice() == [ScalarArgument::Bool(BoolValue::Const(false))]
                && *layout == header_layout
                && instructions.contains(&Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: i32_const(7),
                })
        )),
        "{function:?}"
    );

    let field_header_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallOutcomeDirectAggregate {
            destination,
            target,
            arguments,
            layout,
            failure_mode: OutcomeFailureMode::Recover { instructions },
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_header")
            && arguments.as_slice() == [ScalarArgument::Bool(BoolValue::Const(true))]
            && *layout == header_layout)
            .then_some((*destination, instructions))
    });
    let Some((header_destination, header_fallback)) = field_header_call else {
        panic!("{function:?}");
    };
    assert!(header_fallback.contains(&Instruction::StoreAggregateI32 {
        destination: header_destination,
        offset: 4,
        value: i32_const(90),
    }));
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CopyAggregateRange {
            destination_offset: 8,
            source,
            source_offset: 0,
            layout,
            ..
        } if *source == header_destination && *layout == header_layout
    )));

    let triple_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallOutcomeAggregate {
            destination,
            target,
            failure_mode: OutcomeFailureMode::Recover { instructions },
            ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_triple")).then_some((*destination, instructions))
    });
    let Some((triple_destination, triple_fallback)) = triple_call else {
        panic!("{function:?}");
    };
    assert!(triple_fallback.contains(&Instruction::CopyAggregateRange {
        destination: triple_destination,
        destination_offset: 0,
        source: AggregateLocation::Slot(1),
        source_offset: 0,
        layout: triple_layout,
    }));
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CopyAggregateRange {
            destination_offset: 24,
            source,
            source_offset: 0,
            layout,
            ..
        } if *source == triple_destination && *layout == triple_layout
    )));
}

#[test]
fn lowers_optional_aggregate_otherwise_member_roots() {
    let packet_layout = ValueLayout::new(48, 8);
    let triple_layout = ValueLayout::new(20, 4);
    let packet_type = Type::Aggregate {
        layout: packet_layout,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: i32
    second: i32
    third: i32
    fourth: i32
    fifth: i32
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
}

func main(): i32 {
    let fallback = Packet {
        prefix: 5,
        header: Header { tag: 1, ok: false, code: 7, len: 2 },
        triple: Triple { first: 2, second: 8, third: 1, fourth: 1, fifth: 4 },
    }
    let code = (maybe_packet(false) otherwise { fallback }).header.code
    let triple = (maybe_packet(true) otherwise { fallback }).triple
    return code + triple.second
}

func maybe_packet(flag: bool): Packet? {
    return none
}
"#,
        "main",
        function_signatures(vec![(
            "maybe_packet",
            Type::Optional(Box::new(packet_type.clone())),
            vec![Type::Bool],
        )]),
    )
    .unwrap();

    let scalar_member_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallOutcomeAggregate {
            destination,
            target,
            arguments,
            failure_mode: OutcomeFailureMode::Recover { instructions },
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_packet")
            && arguments.as_slice() == [ScalarArgument::Bool(BoolValue::Const(false))])
        .then_some((*destination, instructions))
    });
    let Some((scalar_source, scalar_fallback)) = scalar_member_call else {
        panic!("{function:?}");
    };
    assert!(scalar_fallback.contains(&Instruction::CopyAggregateRange {
        destination: scalar_source,
        destination_offset: 0,
        source: AggregateLocation::Slot(0),
        source_offset: 0,
        layout: packet_layout,
    }));
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::LoadAggregateI32 {
            source,
            offset: 12,
            ..
        } if *source == scalar_source
    )));

    let aggregate_member_call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallOutcomeAggregate {
            destination,
            target,
            arguments,
            ..
        } = instruction
        else {
            return None;
        };
        (target == &CallTarget::same_file("maybe_packet")
            && arguments.as_slice() == [ScalarArgument::Bool(BoolValue::Const(true))])
        .then_some(*destination)
    });
    let Some(aggregate_source) = aggregate_member_call else {
        panic!("{function:?}");
    };
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CopyAggregateRange {
            destination_offset: 0,
            source,
            source_offset: 24,
            layout,
            ..
        } if *source == aggregate_source && *layout == triple_layout
    )));
}
