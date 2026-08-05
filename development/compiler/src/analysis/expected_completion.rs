//! Expected-argument compatibility for completion ranking.

use super::signature_help::signature_help_for_file_analysis;
use super::visible_locals::visible_local_bindings_at_offset;
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::type_expr_display_lossy;
use crate::source::{ByteSpan, SourceMap};
use crate::typecheck::type_expr_is_assignable;
use std::collections::HashSet;

pub(super) fn compatible_local_spans_at_offset(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> HashSet<ByteSpan> {
    let spread_expected = file
        .typecheck_facts
        .sequence_spread_plans()
        .map(|(_, plan)| plan)
        .filter(|plan| plan.spread_span.start <= offset && offset <= plan.spread_span.end)
        .min_by_key(|plan| (plan.spread_span.len(), plan.spread_span.start))
        .map(|plan| plan.source_type.clone());
    let signature_expected = || {
        let signature = signature_help_for_file_analysis(sources, analysis, file, offset)?;
        signature
            .parameters
            .get(signature.active_parameter)
            .map(|parameter| parameter.ty.clone())
    };
    let Some(expected) = spread_expected.or_else(signature_expected) else {
        return HashSet::new();
    };

    visible_local_bindings_at_offset(&file.ast, offset)
        .into_iter()
        .filter_map(|binding| {
            let actual = file.typecheck_facts.binding_type_expr(binding.name_span)?;
            (type_expr_is_assignable(&expected, actual, &file.resolved)
                || completion_type_key(&expected) == completion_type_key(actual))
            .then_some(binding.name_span)
        })
        .collect()
}

fn completion_type_key(ty: &crate::ast::TypeExpr) -> String {
    let label = type_expr_display_lossy(ty);
    let mut key = String::with_capacity(label.len());
    let mut token = String::new();
    let flush = |key: &mut String, token: &mut String| {
        let short = token.rsplit(['/', '.']).next().unwrap_or(token.as_str());
        key.push_str(short);
        token.clear();
    };
    for character in label.chars() {
        if character == '/' || character == '.' || character == '_' || character.is_alphanumeric() {
            token.push(character);
        } else {
            flush(&mut key, &mut token);
            key.push(character);
        }
    }
    flush(&mut key, &mut token);
    key
}
