use super::*;

/// Maximum outgoing ABI words required by any direct or nested call-like
/// instruction. Structural recursion and call-family classification live in
/// the IR effects projection rather than being repeated by the backend.
pub(super) fn max_call_argument_count(instructions: &[Instruction]) -> usize {
    let mut maximum = 0;
    crate::ir::visit_instruction_tree(instructions, &mut |instruction| {
        maximum = maximum.max(instruction.effects().call_argument_words());
    });
    maximum
}
