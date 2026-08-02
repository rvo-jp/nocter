use super::*;

#[test]
fn lowers_borrow_of_aggregate_slice_element_as_its_storage_address() {
    let function = lower_named_function(
        r#"copy struct Item {
    value: i32
}

func first(values: &[Item]): &Item {
    return &values[0]
}

func main(): i32 {
    return 0
}
"#,
        "first",
    );

    assert_eq!(
        function,
        Function {
            name: "first".to_string(),
            target: CallTarget::same_file("first"),
            return_type: Type::Borrow {
                is_readwrite: false,
                inner: Box::new(Type::DirectAggregate {
                    layout: ValueLayout::new(4, 4),
                    words: 1,
                }),
            },
            instructions: vec![
                Instruction::SetUsizeFromBorrow {
                    destination: UsizeLocation::Return,
                    source: BorrowSource::SliceIndex {
                        source: SliceLocation::Parameter(0),
                        index: SliceElementIndex::Const(0),
                        element: SliceElementAddressKind::Aggregate { stride: 4 },
                    },
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_optional_borrow_return_call_and_local_without_erasing_borrow_type() {
    let ir = lower_text(
        r#"copy struct Item {
    value: i32
}

func maybe(value: &Item, present: bool): (&Item)? {
    if present {
        return value
    }
    return none
}

func read(value: &Item): i32 {
    return value.value
}

func use_value(value: &Item): i32 {
    let found = maybe(value, true) otherwise { return 0 }
    return read(found)
}

func main(): i32 {
    let item = Item { value: 42 }
    return use_value(&item)
}
"#,
    );
    let maybe = ir
        .functions
        .iter()
        .find(|function| function.name == "maybe")
        .unwrap();
    let use_value = ir
        .functions
        .iter()
        .find(|function| function.name == "use_value")
        .unwrap();

    assert_eq!(
        maybe.return_type,
        Type::Fallible(Box::new(Type::Borrow {
            is_readwrite: false,
            inner: Box::new(Type::DirectAggregate {
                layout: ValueLayout::new(4, 4),
                words: 1,
            }),
        }))
    );
    assert!(
        maybe.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::If { then_instructions, .. }
            if then_instructions.contains(&Instruction::SetUsizeFromBorrow {
                destination: UsizeLocation::Return,
                source: BorrowSource::AggregateParameter(0),
            }) && then_instructions.contains(&Instruction::ReturnFallibleSuccess)
        )),
        "{maybe:?}"
    );

    assert!(
        matches!(
            use_value.instructions.as_slice(),
            [
                Instruction::CallFallibleBorrow {
                    destination: UsizeLocation::Local(0),
                    target,
                    failure_mode: FallibleFailureMode::Handle { .. },
                    ..
                },
                Instruction::CallI32 { arguments, .. },
                Instruction::Return,
            ] if *target == CallTarget::same_file("maybe")
                && arguments == &[ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::BorrowLocal(UsizeLocation::Local(0)),
                })]
        ),
        "{use_value:?}"
    );
}
