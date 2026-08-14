use super::*;

#[test]
fn constructs_borrow_capture_environment_through_mir() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func build(value: i32): i32 {
    let callback = (&value; input: i32): i32 { input + value }
    return value
}
"#,
        "build",
    );

    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::SetUsizeFromBorrow {
            source: BorrowSource::I32(I32Location::Parameter(0)),
            ..
        }
    )));
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::StoreAggregateUsize {
            destination: AggregateLocation::Slot(0),
            offset: 0,
            ..
        }
    )));
}

#[test]
fn moves_owned_capture_into_environment_range_through_mir() {
    let function = lower_named_function(
        r#"struct Boxed {
    value: i32
}

func main(): i32 {
    return 0
}

func build(value: Boxed): i32 {
    let callback = (move value;): i32 { value.value }
    return 1
}
"#,
        "build",
    );

    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CopyAggregateRange {
            destination: AggregateLocation::Slot(1),
            destination_offset: 0,
            source: AggregateLocation::Slot(0),
            source_offset: 0,
            layout,
        } if *layout == ValueLayout::new(4, 4)
    )));
}
