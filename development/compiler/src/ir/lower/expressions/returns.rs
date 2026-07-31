use super::*;

pub(super) fn replace_success_returns(instructions: Vec<Instruction>) -> Vec<Instruction> {
    instructions
        .into_iter()
        .map(|instruction| match instruction {
            Instruction::Return => Instruction::ReturnFallibleSuccess,
            Instruction::If {
                condition,
                then_instructions,
                else_instructions,
            } => Instruction::If {
                condition,
                then_instructions: replace_success_returns(then_instructions),
                else_instructions: replace_success_returns(else_instructions),
            },
            instruction => instruction,
        })
        .collect()
}

pub(super) fn never_tail_call_argument_requires_current_frame(argument: &ScalarArgument) -> bool {
    matches!(argument, ScalarArgument::Borrow(_)) || is_tail_call_stack_pointer_argument(argument)
}
