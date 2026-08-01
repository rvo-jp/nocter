use super::*;

#[test]
fn partial_payload_enum_field_replacement_preserves_the_old_field() {
    let ir = lower_text(
        r#"struct File {
    code: i32
}

impl File {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: [File; 2])
    failed
}

struct Wrapper {
    prefix: i32
    result: Result
}

func make_file(): File! {
    return File { code: 22 }
}

func main(): i32! {
    var wrapper = Wrapper {
        prefix: 1,
        result: Result.ok([File { code: 10 }, File { code: 11 }]),
    }
    wrapper.result = Result.ok([File { code: 20 }, make_file()?])
    return 42
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
            Instruction::CallFallibleAggregate {
                target,
                failure_mode: FallibleFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            }
            | Instruction::CallFallibleDirectAggregate {
                target,
                failure_mode: FallibleFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("make_file") => Some(instructions),
            _ => None,
        })
        .expect("expected field replacement initialization cleanup");

    let partial_cleanup_index = cleanup
        .iter()
        .position(|instruction| cleanup_loads_payload_from_slot(instruction, 1, 4))
        .expect("expected partial replacement cleanup");
    let old_field_cleanup_index = cleanup
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                Instruction::LoadAggregateU8 {
                    source: AggregateLocation::Slot(0),
                    offset: 4,
                    ..
                }
            )
        })
        .expect("expected existing wrapper field cleanup on propagation");
    assert!(partial_cleanup_index < old_field_cleanup_index, "{main:?}");
    assert!(
        main.instructions
            .contains(&Instruction::CopyAggregateRange {
                destination: AggregateLocation::Slot(0),
                destination_offset: 4,
                source: AggregateLocation::Slot(1),
                source_offset: 0,
                layout: ValueLayout::new(12, 4),
            })
    );
}

fn cleanup_loads_payload_from_slot(
    instruction: &Instruction,
    slot_index: usize,
    offset: u32,
) -> bool {
    match instruction {
        Instruction::CallVoid { arguments, .. } => matches!(
            arguments.as_slice(),
            [ScalarArgument::Borrow(BorrowArgument {
                source: BorrowSource::AggregateSlotField {
                    slot_index: actual_slot,
                    offset: actual_offset,
                },
            })] if *actual_slot == slot_index && *actual_offset == offset
        ),
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => {
            then_instructions
                .iter()
                .any(|instruction| cleanup_loads_payload_from_slot(instruction, slot_index, offset))
                || else_instructions.iter().any(|instruction| {
                    cleanup_loads_payload_from_slot(instruction, slot_index, offset)
                })
        }
        _ => false,
    }
}
