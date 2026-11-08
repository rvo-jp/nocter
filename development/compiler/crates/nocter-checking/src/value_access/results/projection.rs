//! Project result identities and freeze pending value supply before initialization.
use super::Evidence;
use crate::value_access::{Failure, Query, ir::ExpressionKind};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;

pub(super) fn project(
    context: &DeclarationContext<'_>,
    query: &mut Query,
    evidence: &Evidence,
) -> Result<(), Failure> {
    let program = query.program.clone();
    crate::value_access::types::project(
        query,
        |data| evidence.data(&program, data),
        |query, id, expression| {
            if let ExpressionKind::Call {
                target, arguments, ..
            } = &mut expression.kind
            {
                *target = evidence.origin(*target)?;
                for argument in arguments {
                    if let Some(binding) = &mut argument.binding {
                        binding.witness = evidence.witness(binding.witness)?;
                    }
                }
            }
            crate::value_access::supply::resolve(context, query, id, expression)
        },
    )?;
    query.equalities.clear();
    Ok(())
}
