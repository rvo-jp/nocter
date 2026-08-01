//! Expected-argument compatibility for completion ranking.

use super::signature_help::signature_help_for_file_analysis;
use super::visible_locals::visible_local_bindings_at_offset;
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::source::{ByteSpan, SourceMap};
use crate::typecheck::type_expr_is_assignable;
use std::collections::HashSet;

pub(super) fn compatible_local_spans_at_offset(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> HashSet<ByteSpan> {
    let Some(signature) = signature_help_for_file_analysis(sources, analysis, file, offset) else {
        return HashSet::new();
    };
    let Some(parameter) = signature.parameters.get(signature.active_parameter) else {
        return HashSet::new();
    };

    visible_local_bindings_at_offset(&file.ast, offset)
        .into_iter()
        .filter_map(|binding| {
            let actual = file.typecheck_facts.binding_type_expr(binding.name_span)?;
            type_expr_is_assignable(&parameter.ty, actual, &file.resolved)
                .then_some(binding.name_span)
        })
        .collect()
}
