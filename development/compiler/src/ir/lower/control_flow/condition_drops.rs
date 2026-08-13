use super::*;

pub(super) fn aggregate_argument_slots_in_instructions(
    instructions: &[Instruction],
) -> HashSet<usize> {
    let mut slots = HashSet::new();
    for instruction in instructions {
        match instruction {
            Instruction::CallI32 { arguments, .. }
            | Instruction::CallOutcomeI32 { arguments, .. }
            | Instruction::CallU8 { arguments, .. }
            | Instruction::CallOutcomeU8 { arguments, .. }
            | Instruction::CallUsize { arguments, .. }
            | Instruction::CallOutcomeUsize { arguments, .. }
            | Instruction::CallBool { arguments, .. }
            | Instruction::CallOutcomeBool { arguments, .. }
            | Instruction::CallStr { arguments, .. }
            | Instruction::CallOutcomeStr { arguments, .. }
            | Instruction::CallSlice { arguments, .. }
            | Instruction::CallOutcomeSlice { arguments, .. }
            | Instruction::CallVoid { arguments, .. }
            | Instruction::CallAggregate { arguments, .. }
            | Instruction::CallOutcomeAggregate { arguments, .. }
            | Instruction::CallDirectAggregate { arguments, .. }
            | Instruction::CallOutcomeDirectAggregate { arguments, .. }
            | Instruction::TailCall { arguments, .. } => {
                collect_aggregate_argument_slots(arguments, &mut slots);
            }
            Instruction::If {
                then_instructions,
                else_instructions,
                ..
            } => {
                slots.extend(aggregate_argument_slots_in_instructions(then_instructions));
                slots.extend(aggregate_argument_slots_in_instructions(else_instructions));
            }
            Instruction::While {
                condition_instructions,
                body_instructions,
                ..
            } => {
                slots.extend(aggregate_argument_slots_in_instructions(
                    condition_instructions,
                ));
                slots.extend(aggregate_argument_slots_in_instructions(body_instructions));
            }
            _ => {}
        }
    }
    slots
}

fn collect_aggregate_argument_slots(arguments: &[ScalarArgument], slots: &mut HashSet<usize>) {
    for argument in arguments {
        let source = match argument {
            ScalarArgument::AggregateIndirect(argument) => &argument.source,
            ScalarArgument::AggregateDirect(argument) => &argument.source,
            _ => continue,
        };
        if let AggregateArgumentSource::Slot(slot_index) = source {
            slots.insert(*slot_index);
        }
    }
}

pub(super) fn remove_condition_moved_aggregate_drops(
    instructions: &mut Vec<Instruction>,
    moved_slots: &HashSet<usize>,
) {
    if moved_slots.is_empty() {
        return;
    }
    for instruction in instructions.iter_mut() {
        match instruction {
            Instruction::If {
                then_instructions,
                else_instructions,
                ..
            } => {
                remove_condition_moved_aggregate_drops(then_instructions, moved_slots);
                remove_condition_moved_aggregate_drops(else_instructions, moved_slots);
            }
            Instruction::While {
                condition_instructions,
                body_instructions,
                ..
            } => {
                remove_condition_moved_aggregate_drops(condition_instructions, moved_slots);
                remove_condition_moved_aggregate_drops(body_instructions, moved_slots);
            }
            Instruction::CallOutcomeI32 { failure_mode, .. }
            | Instruction::CallOutcomeU8 { failure_mode, .. }
            | Instruction::CallOutcomeUsize { failure_mode, .. }
            | Instruction::CallOutcomeBool { failure_mode, .. }
            | Instruction::CallOutcomeStr { failure_mode, .. }
            | Instruction::CallOutcomeSlice { failure_mode, .. }
            | Instruction::CallOutcomeAggregate { failure_mode, .. }
            | Instruction::CallOutcomeDirectAggregate { failure_mode, .. } => {
                remove_condition_moved_aggregate_drops_from_failure_mode(failure_mode, moved_slots);
            }
            _ => {}
        }
    }
    instructions.retain(|instruction| !is_condition_moved_aggregate_drop(instruction, moved_slots));
}

fn remove_condition_moved_aggregate_drops_from_failure_mode(
    failure_mode: &mut OutcomeFailureMode,
    moved_slots: &HashSet<usize>,
) {
    match failure_mode {
        OutcomeFailureMode::PropagateWithCleanup { instructions, .. }
        | OutcomeFailureMode::Recover { instructions }
        | OutcomeFailureMode::Handle { instructions }
        | OutcomeFailureMode::Catch { instructions, .. } => {
            remove_condition_moved_aggregate_drops(instructions, moved_slots);
        }
        OutcomeFailureMode::Propagate | OutcomeFailureMode::Trap => {}
    }
}

fn is_condition_moved_aggregate_drop(
    instruction: &Instruction,
    moved_slots: &HashSet<usize>,
) -> bool {
    let Instruction::CallVoid { arguments, .. } = instruction else {
        return false;
    };
    let [ScalarArgument::Borrow(argument)] = arguments.as_slice() else {
        return false;
    };
    let BorrowSource::AggregateSlot(slot_index) = argument.source else {
        return false;
    };
    moved_slots.contains(&slot_index)
}
