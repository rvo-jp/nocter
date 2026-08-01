use super::*;

#[test]
fn drops_complete_owned_temporary_when_a_later_argument_fails() {
    let ir = lower_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func code(): i32! {
    return 3
}

func consume(files: [File; 2], value: i32): void {
    return
}

func main(): void! {
    consume([File { fd: 1 }, File { fd: 2 }], code()?)
    return
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallFallibleI32 {
                target,
                failure_mode: FallibleFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("code") => Some(instructions),
            _ => None,
        });

    assert!(matches!(
        cleanup.map(Vec::as_slice),
        Some([
            Instruction::CallVoid { arguments: second, .. },
            Instruction::CallVoid { arguments: first, .. },
        ]) if matches!(
            second.as_slice(),
            [ScalarArgument::Borrow(BorrowArgument {
                source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 4 }
            })]
        ) && matches!(
            first.as_slice(),
            [ScalarArgument::Borrow(BorrowArgument {
                source: BorrowSource::AggregateSlotField { slot_index: 0, offset: 0 }
            })]
        )
    ));
}
