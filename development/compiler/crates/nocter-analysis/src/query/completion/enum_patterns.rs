use nocter_checking::{CheckedProgram, EnumPatternCompletionCandidate, EnumPatternCompletionError};
use nocter_declarations::DeclarationGraph;
use nocter_model::{NominalTypeId, VariantId};
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceIndex};
use nocter_syntax::{NodeKind, SyntaxTree};

use super::{SemanticCompletion, SemanticCompletionKind};
use crate::query::presentation::presentation;
use crate::query::presentation::visible_spelling::VisibleSpellings;
use crate::query::source_selection::select_source_candidates;

pub(super) fn checked_completions(
    program: &CheckedProgram,
    index: &SourceIndex,
    trees: &[SyntaxTree],
    source: SourceId,
    offset: ByteOffset,
    spellings: &VisibleSpellings,
) -> Result<Option<Box<[SemanticCompletion]>>, EnumPatternCompletionError> {
    let Some(pattern) = containing_pattern(trees, source, offset) else {
        return Ok(None);
    };
    let variant = select_source_candidates(index.bindings_in(source).filter_map(|binding| {
        let SemanticEntity::Variant(variant) = binding.entity() else {
            return None;
        };
        pattern
            .contains_range(binding.origin().span().range())
            .then_some((*binding, variant))
    }))
    .unique();
    let Some(variant) = variant else {
        return Ok(None);
    };
    let definition = variant_owner(program.graph(), variant)?;
    let candidates = program.enum_pattern_completions(definition, source)?;
    Ok(Some(render_completions(
        program.graph(),
        &candidates,
        |variant| {
            presentation(program, SemanticEntity::Variant(variant), spellings)
                .map(|value| Box::<str>::from(value.code()))
        },
    )))
}

pub(super) fn render_recovery_completions(
    graph: &DeclarationGraph,
    candidates: &[EnumPatternCompletionCandidate],
    detail: impl FnMut(VariantId) -> Option<Box<str>>,
) -> Box<[SemanticCompletion]> {
    render_completions(graph, candidates, detail)
}

fn render_completions(
    graph: &DeclarationGraph,
    candidates: &[EnumPatternCompletionCandidate],
    mut detail: impl FnMut(VariantId) -> Option<Box<str>>,
) -> Box<[SemanticCompletion]> {
    candidates
        .iter()
        .filter_map(|candidate| {
            let variant = candidate.variant();
            Some(SemanticCompletion::new(
                graph.symbols().spelling(candidate.name())?,
                SemanticCompletionKind::EnumMember,
                detail(variant),
            ))
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn variant_owner(
    graph: &DeclarationGraph,
    variant: VariantId,
) -> Result<NominalTypeId, EnumPatternCompletionError> {
    graph
        .declarations()
        .variants()
        .get(variant)
        .map(nocter_declarations::VariantDeclaration::owner)
        .ok_or(EnumPatternCompletionError::MissingVariant(variant))
}

fn containing_pattern(
    trees: &[SyntaxTree],
    source: SourceId,
    offset: ByteOffset,
) -> Option<TextRange> {
    trees
        .iter()
        .find(|tree| tree.source() == source)
        .into_iter()
        .flat_map(SyntaxTree::nodes)
        .filter_map(|(_, node)| {
            (node.kind() == NodeKind::EnumPattern && node.range().contains_cursor(offset))
                .then_some(node.range())
        })
        .min_by_key(|range| range.len())
}
