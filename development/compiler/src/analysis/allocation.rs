//! Editor-facing presentation of compiler-owned allocation-effect facts.

use super::CompileUnitAnalysis;
use crate::source::ByteSpan;

pub(crate) fn allocation_effect_markdown(
    analysis: &CompileUnitAnalysis,
    declaration: ByteSpan,
) -> Option<String> {
    let fact = analysis.callable_semantic_facts.get(declaration)?;
    fact.needs_current_allocation_context
        .then(|| "**Allocation effect:** uses the current allocation context.".to_string())
}
