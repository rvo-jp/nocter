use super::*;

#[test]
fn lowers_fallible_aggregate_catch_field_read_in_comparison() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    return run()?
}

func run(): i32! {
    if (source() catch error {
        return Error.new("app.source", error.message)
    }).code == 42 {
        return 42
    } else {
        return 1
    }
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let run = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(run, AggregateLocation::Slot(0), "source");
    assert!(run.instructions.contains(&Instruction::LoadAggregateI32 {
        destination: I32Location::Local(0),
        source: AggregateLocation::Slot(0),
        offset: 4,
    }));
    assert!(run.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::If {
                condition: BoolValue::I32Comparison {
                    operator: I32ComparisonOperator::Equal,
                    left,
                    right,
                },
                ..
            } if left == &i32_local(0) && right == &i32_const(42)
        )
    }));
}

#[test]
fn lowers_fallible_aggregate_catch_call_binding() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    return run()?
}

func run(): i32! {
    let value = source() catch error {
        return Error.new("app.source", error.message)
    }
    return value.code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(0),
        "source",
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnOutcomeSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_call_return() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    let value = forward()?
    return value.code
}

func forward(): Header! {
    return source() catch error {
        return Error.new("app.source", error.message)
    }
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "forward")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::DirectReturn,
        "source",
    );
    assert_eq!(
        main.instructions.last(),
        Some(&Instruction::ReturnOutcomeSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_value_argument() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    return run()?
}

func run(): i32! {
    return consume(source() catch error {
        return Error.new("app.source", error.message)
    })
}

func consume(header: Header): i32 {
    return header.code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(0),
        "source",
    );
    assert!(main.instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::CallI32 { target, .. } if target == &CallTarget::same_file("consume"))
    }));
}

#[test]
fn lowers_fallible_aggregate_catch_member_field_read() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    return run()?
}

func run(): i32! {
    return (source() catch error {
        return Error.new("app.source", error.message)
    }).code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(0),
        "source",
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnOutcomeSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_member_binding() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
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

func main(): i32! {
    return run()?
}

func run(): i32! {
    let header = (source() catch error {
        return Error.new("app.source", error.message)
    }).header
    return header.code
}

func source(): Packet! {
    return Packet {
        prefix: 1,
        header: Header { tag: 7, ok: true, code: 42, len: 11 },
        tail: 2,
    }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert!(main.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::CallOutcomeAggregate {
                destination: AggregateLocation::Slot(1),
                target,
                arguments,
                failure_mode: OutcomeFailureMode::Catch { .. },
            } if target == &CallTarget::same_file("source") && arguments.is_empty()
        )
    }));
    assert!(
        main.instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 0,
                source: AggregateLocation::Slot(1),
                source_offset: 8,
                layout: ValueLayout::new(16, 8),
            })
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnOutcomeSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_assignment() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32! {
    return run()?
}

func run(): i32! {
    var value = Header { tag: 1, ok: false, code: 2, len: 3 }
    value = source() catch error {
        return Error.new("app.source", error.message)
    }
    return value.code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(0),
        "source",
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnOutcomeSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_member_assignment() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
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

func main(): i32! {
    return run()?
}

func run(): i32! {
    var packet = Packet {
        prefix: 1,
        header: Header { tag: 1, ok: false, code: 2, len: 3 },
        tail: 4,
    }
    packet.header = source() catch error {
        return Error.new("app.source", error.message)
    }
    return packet.header.code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(1),
        "source",
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnOutcomeSuccess)
    );
}

#[test]
fn lowers_fallible_aggregate_catch_struct_literal_field() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

copy struct Header {
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

func main(): i32! {
    return run()?
}

func run(): i32! {
    let packet = Packet {
        prefix: 1,
        header: source() catch error {
            return Error.new("app.source", error.message)
        },
        tail: 2,
    }
    return packet.header.code
}

func source(): Header! {
    return Header { tag: 7, ok: true, code: 42, len: 11 }
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    assert_contains_fallible_direct_aggregate_catch_call(
        main,
        AggregateLocation::Slot(1),
        "source",
    );
    assert!(
        main.instructions
            .contains(&Instruction::ReturnOutcomeSuccess)
    );
}

#[test]
fn lowers_pending_aggregate_drop_for_catch_failure_return_cleanup() {
    let ir = lower_text_with_std_error(
        r#"use std/error.Error

struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32! {
    var file = File { fd: 3 }
    let value = answer() catch error {
        return Error.new("app.answer", error.message)
    }
    return value
}

func answer(): i32! {
    return Error.new("app.inner", "inner failed")
}
"#,
    );

    let drop_call = Instruction::CallVoid {
        target: CallTarget::same_file("File.drop"),
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateSlot(0),
        })],
    };
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let Some(Instruction::CallOutcomeI32 {
        failure_mode:
            OutcomeFailureMode::Catch {
                code,
                message,
                instructions,
            },
        ..
    }) = main
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallOutcomeI32 { .. }))
    else {
        panic!("missing fallible i32 catch call: {main:?}");
    };
    assert_eq!(*code, StrLocation::Local(1));
    assert_eq!(*message, StrLocation::Local(3));
    assert_eq!(
        instructions,
        &vec![
            drop_call,
            Instruction::ReturnFallibleFailure {
                code: StrValue::StaticBytes(b"app.answer".to_vec()),
                message: StrValue::Location(StrLocation::Local(3)),
            },
        ],
    );
}
