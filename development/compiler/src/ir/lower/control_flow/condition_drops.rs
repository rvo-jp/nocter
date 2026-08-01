use super::*;

pub(super) fn condition_explicit_moves_are_single_evaluation_call(condition: &Expr) -> bool {
    match condition {
        Expr::Call(_) => true,
        Expr::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            condition_explicit_moves_are_single_evaluation_call(&unary.operand)
        }
        Expr::Propagate(propagation) => {
            condition_explicit_moves_are_single_evaluation_call(&propagation.expression)
        }
        Expr::Force(force) => {
            condition_explicit_moves_are_single_evaluation_call(&force.expression)
        }
        Expr::Catch(catch) => {
            condition_explicit_moves_are_single_evaluation_call(&catch.expression)
        }
        Expr::Group(group) => {
            condition_explicit_moves_are_single_evaluation_call(&group.expression)
        }
        _ => false,
    }
}

pub(super) fn aggregate_argument_slots_in_instructions(
    instructions: &[Instruction],
) -> HashSet<usize> {
    let mut slots = HashSet::new();
    for instruction in instructions {
        match instruction {
            Instruction::CallI32 { arguments, .. }
            | Instruction::CallFallibleI32 { arguments, .. }
            | Instruction::CallU8 { arguments, .. }
            | Instruction::CallFallibleU8 { arguments, .. }
            | Instruction::CallUsize { arguments, .. }
            | Instruction::CallFallibleUsize { arguments, .. }
            | Instruction::CallBool { arguments, .. }
            | Instruction::CallFallibleBool { arguments, .. }
            | Instruction::CallStr { arguments, .. }
            | Instruction::CallFallibleStr { arguments, .. }
            | Instruction::CallSlice { arguments, .. }
            | Instruction::CallFallibleSlice { arguments, .. }
            | Instruction::CallVoid { arguments, .. }
            | Instruction::CallAggregate { arguments, .. }
            | Instruction::CallFallibleAggregate { arguments, .. }
            | Instruction::CallDirectAggregate { arguments, .. }
            | Instruction::CallFallibleDirectAggregate { arguments, .. }
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
        let AggregateArgumentSource::Slot(slot_index) = source;
        slots.insert(*slot_index);
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
            Instruction::CallFallibleI32 { failure_mode, .. }
            | Instruction::CallFallibleU8 { failure_mode, .. }
            | Instruction::CallFallibleUsize { failure_mode, .. }
            | Instruction::CallFallibleBool { failure_mode, .. }
            | Instruction::CallFallibleStr { failure_mode, .. }
            | Instruction::CallFallibleSlice { failure_mode, .. }
            | Instruction::CallFallibleAggregate { failure_mode, .. }
            | Instruction::CallFallibleDirectAggregate { failure_mode, .. } => {
                remove_condition_moved_aggregate_drops_from_failure_mode(failure_mode, moved_slots);
            }
            _ => {}
        }
    }
    instructions.retain(|instruction| !is_condition_moved_aggregate_drop(instruction, moved_slots));
}

fn remove_condition_moved_aggregate_drops_from_failure_mode(
    failure_mode: &mut FallibleFailureMode,
    moved_slots: &HashSet<usize>,
) {
    match failure_mode {
        FallibleFailureMode::PropagateWithCleanup { instructions, .. }
        | FallibleFailureMode::Recover { instructions }
        | FallibleFailureMode::Handle { instructions }
        | FallibleFailureMode::Catch { instructions, .. } => {
            remove_condition_moved_aggregate_drops(instructions, moved_slots);
        }
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => {}
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
