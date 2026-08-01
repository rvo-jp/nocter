use super::*;

pub(super) fn lower_nonterminal_loop_control_statement(
    instruction: Instruction,
    context: &mut LoweringContext,
    loop_scope_mark: Option<usize>,
    continue_instructions: &[Instruction],
    diagnostic_code: &'static str,
    subject: &str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(loop_scope_mark) = loop_scope_mark else {
        return Err(unsupported_nonterminal_if_diagnostic(
            diagnostic_code,
            subject,
        ));
    };

    let mut instructions = lower_scope_end_drops_for_locals_since(context, loop_scope_mark)?;
    if matches!(instruction, Instruction::Continue) {
        instructions.extend(continue_instructions.iter().cloned());
    }
    instructions.push(instruction);
    Ok(instructions)
}
