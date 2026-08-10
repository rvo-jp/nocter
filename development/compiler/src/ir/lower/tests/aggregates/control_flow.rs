use super::*;

#[test]
fn lowers_nonterminal_if_branch_aggregate_slots_with_distinct_layouts() {
    let ir = lower_text(
        r#"struct Small {
    value: i32
}

instance Small {
    drop &+self {
        return
    }
}

struct Wide {
    left: i32
    right: i32
}

instance Wide {
    drop &+self {
        return
    }
}

func main(): i32 {
    if true {
        var small = Small { value: 1 }
    } else {
        var wide = Wide { left: 2, right: 3 }
    }
    return 0
}
"#,
    );

    let small_drop = Instruction::CallVoid {
        target: CallTarget::same_file("Small.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let wide_drop = Instruction::CallVoid {
        target: CallTarget::same_file("Wide.drop"),
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
                    small_drop,
                ],
                else_instructions: vec![
                    Instruction::ReserveAggregateSlot {
                        slot_index: 1,
                        layout: ValueLayout::new(8, 4),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(1),
                        offset: 0,
                        value: i32_const(2),
                    },
                    Instruction::StoreAggregateI32 {
                        destination: AggregateLocation::Slot(1),
                        offset: 4,
                        value: i32_const(3),
                    },
                    wide_drop,
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
