use super::*;

pub(in crate::ir::lower) fn split_terminal_branch_block<'a>(
    block: &'a Block,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
) -> Result<(TerminalBranch<'a>, &'a [Stmt]), Vec<Diagnostic>> {
    if let Some(result) = &block.result {
        return Ok((TerminalBranch::Result(result), block.statements.as_slice()));
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return Err(unsupported_terminal_if_diagnostic(
            diagnostic_code,
            subject,
            return_label,
        ));
    };
    Ok((TerminalBranch::Statement(terminal), leading))
}

pub(in crate::ir::lower) fn lower_terminal_branch_leading_statements(
    statements: &[Stmt],
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    return_label: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();
    for statement in statements {
        match statement {
            Stmt::Import(_) | Stmt::FromImport(_) => {}
            Stmt::Binding(statement) => instructions.extend(
                lower_local_binding(statement, context).map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Assignment(statement) => instructions.extend(
                lower_assignment(statement, context).map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Drop(statement) => instructions.extend(
                lower_drop_statement(statement, context).map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.span)
                })?,
            ),
            Stmt::Expression(statement) => {
                let Some(void_instructions) = lower_void_expression_statement(
                    &statement.expression,
                    context,
                )
                .map_err(|diagnostics| {
                    attach_primary_span_if_absent(diagnostics, sources, statement.expression.span())
                })?
                else {
                    return Err(attach_primary_span_if_absent(
                        unsupported_terminal_if_diagnostic(diagnostic_code, subject, return_label),
                        sources,
                        statement.span,
                    ));
                };
                instructions.extend(void_instructions);
            }
            _ => {
                return Err(attach_primary_span_if_absent(
                    unsupported_terminal_if_diagnostic(diagnostic_code, subject, return_label),
                    sources,
                    statement.span(),
                ));
            }
        }
        mark_lowered_statement_aggregate_uses(statement, context);
    }
    Ok(instructions)
}
