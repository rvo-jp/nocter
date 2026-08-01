use super::*;

#[test]
fn lowers_function_with_stack_passed_parameter_word() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func consume(a: &str, b: &str, c: &str, d: &str, e: usize): usize {
    return e
}
"#,
        "consume",
    );

    assert_eq!(
        function,
        Function {
            name: "consume".to_string(),
            target: CallTarget::same_file("consume"),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: usize_param(8),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_call_with_stack_passed_argument_word_as_normal_return_call() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return 0
}

func run(): usize {
    return consume(1, 2, 3, 4, 5, 6, 7, 8, 9)
}

func consume(
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: usize,
    g: usize,
    h: usize,
    i: usize,
): usize {
    return i
}
"#,
        "run",
        function_signatures(vec![(
            "consume",
            Type::Usize,
            vec![
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
                Type::Usize,
            ],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "run".to_string(),
            target: CallTarget::same_file("run"),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::CallUsize {
                    destination: UsizeLocation::Return,
                    target: CallTarget::same_file("consume"),
                    arguments: vec![
                        ScalarArgument::Usize(usize_const(1)),
                        ScalarArgument::Usize(usize_const(2)),
                        ScalarArgument::Usize(usize_const(3)),
                        ScalarArgument::Usize(usize_const(4)),
                        ScalarArgument::Usize(usize_const(5)),
                        ScalarArgument::Usize(usize_const(6)),
                        ScalarArgument::Usize(usize_const(7)),
                        ScalarArgument::Usize(usize_const(8)),
                        ScalarArgument::Usize(usize_const(9)),
                    ],
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_stack_passed_never_call_as_normal_call_then_trap() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32 {
    return abort(1, 2, 3, 4, 5, 6, 7, 8, 9)
}

func abort(
    a: i32,
    b: i32,
    c: i32,
    d: i32,
    e: i32,
    f: i32,
    g: i32,
    h: i32,
    i: i32,
): never {
    abort(a, b, c, d, e, f, g, h, i)
}
"#,
        "main",
        function_signatures(vec![(
            "abort",
            Type::Never,
            vec![
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
            ],
        )]),
    )
    .unwrap();

    assert_eq!(
        function,
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallVoid {
                    target: CallTarget::same_file("abort"),
                    arguments: vec![
                        ScalarArgument::I32(i32_const(1)),
                        ScalarArgument::I32(i32_const(2)),
                        ScalarArgument::I32(i32_const(3)),
                        ScalarArgument::I32(i32_const(4)),
                        ScalarArgument::I32(i32_const(5)),
                        ScalarArgument::I32(i32_const(6)),
                        ScalarArgument::I32(i32_const(7)),
                        ScalarArgument::I32(i32_const(8)),
                        ScalarArgument::I32(i32_const(9)),
                    ],
                },
                Instruction::Trap,
            ],
        }
    );
}

#[test]
fn lowers_split_register_stack_direct_aggregate_call_argument() {
    let pair_type = Type::DirectAggregate {
        layout: ValueLayout::new(16, 4),
        words: 2,
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Pair {
    a: i32
    b: i32
    c: i32
    d: i32
}

func main(): i32 {
    return consume(1, 2, 3, 4, 5, 6, 7, Pair { a: 1, b: 2, c: 3, d: 4 })
}

func consume(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, pair: Pair): i32 {
    return pair.c
}
"#,
        "main",
        function_signatures(vec![(
            "consume",
            Type::I32,
            vec![
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                Type::I32,
                pair_type,
            ],
        )]),
    )
    .unwrap();

    assert!(
        function.instructions.contains(&Instruction::CallI32 {
            destination: I32Location::Return,
            target: CallTarget::same_file("consume"),
            arguments: vec![
                ScalarArgument::I32(i32_const(1)),
                ScalarArgument::I32(i32_const(2)),
                ScalarArgument::I32(i32_const(3)),
                ScalarArgument::I32(i32_const(4)),
                ScalarArgument::I32(i32_const(5)),
                ScalarArgument::I32(i32_const(6)),
                ScalarArgument::I32(i32_const(7)),
                ScalarArgument::AggregateDirect(DirectAggregateArgument {
                    source: AggregateArgumentSource::Slot(0),
                    layout: ValueLayout::new(16, 4),
                    words: 2,
                }),
            ],
        }),
        "{function:?}"
    );
}
