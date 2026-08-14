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

#[test]
fn lowers_closure_callable_body_through_shared_mir_cache() {
    let fixture = analyze_text_fixture(
        r#"func main(): i32 {
    let factor = 2
    let callback = (&factor; value: i32): i32 { value * factor }
    return callback(3)
}
"#,
    );
    let module = lower_executable(&fixture.analysis, &fixture.sources).unwrap();

    assert_eq!(fixture.analysis.mir_bodies.len(), 2);
    let closure = module
        .functions
        .iter()
        .find(|function| function.name.starts_with("<closure@"))
        .expect("reachable closure callable should be lowered");
    assert!(
        closure
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::MultiplyI32 { .. }))
    );
}

#[test]
fn lowers_readwrite_capture_access_as_pointer_backed_places() {
    let fixture = analyze_text_fixture(
        r#"func main(): i32 {
    var total = 1
    var callback = (&+total; value: i32): i32 {
        total = total + value
        total
    }
    return callback(2)
}
"#,
    );
    let module = lower_executable(&fixture.analysis, &fixture.sources).unwrap();
    let closure = module
        .functions
        .iter()
        .find(|function| function.name.starts_with("<closure@"))
        .expect("reachable closure callable should be lowered");

    assert!(
        closure
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::LoadI32FromPointer { .. }))
    );
    assert!(
        closure
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::StoreI32ToPointer { .. }))
    );
}
