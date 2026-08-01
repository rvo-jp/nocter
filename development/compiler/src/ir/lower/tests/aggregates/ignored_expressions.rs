use super::*;

#[test]
fn lowers_ignored_direct_aggregate_call_expression_statement_with_drop() {
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
    make()
    return 0
}

func make(): File {
    return File { fd: 1 }
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
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("File.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
                })],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_ignored_aggregate_literal_expression_statement() {
    let ir = lower_text(
        r#"struct Value {
    code: i32
}

func main(): i32 {
    Value { code: 1 }
    return 0
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
            Instruction::StoreAggregateI32 {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: i32_const(1),
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_ignored_aggregate_literal_expression_statement_with_drop() {
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
    File { fd: 1 }
    return 0
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
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_ignored_alias_aggregate_call_expression_statement_with_drop() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

type Handle = File

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    make()
    return 0
}

func make(): Handle {
    return File { fd: 1 }
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
                target: CallTarget::same_file("make"),
                arguments: vec![],
                layout: ValueLayout::new(4, 4),
            },
            Instruction::CallVoid {
                target: CallTarget::same_file("File.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::AggregateSlot(0),
                })],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_ignored_indirect_aggregate_call_expression_statement() {
    let ir = lower_text(
        r#"copy struct Big {
    a: usize
    b: usize
    c: usize
}

func main(): i32 {
    make()
    return 0
}

func make(): Big {
    return Big { a: 1, b: 2, c: 3 }
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(24, 8),
            },
            Instruction::CallAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
            },
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: i32_const(0),
            },
            Instruction::Return,
        ]
    );
}
