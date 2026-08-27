use nocter_checking::{
    AggregateConstruction, CheckedOperation, CheckedProgram, StructuralFieldCompletionCandidate,
};
use nocter_declarations::DeclarationGraph;
use nocter_model::FieldId;
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceIndex};
use nocter_syntax::{NodeKind, SyntaxTree};

use super::{SemanticCompletion, SemanticCompletionError, SemanticCompletionKind};
use crate::query::evidence::CompleteSemanticQuery;
use crate::query::presentation::presentation;
use crate::query::presentation::visible_spelling::VisibleSpellings;
use crate::query::source_selection::select_source_candidates;

/// Resolves the structural construction containing a checked cursor position.
pub(super) fn checked_completions(
    query: CompleteSemanticQuery<'_>,
    program: &CheckedProgram,
    index: &SourceIndex,
    trees: &[SyntaxTree],
    source: SourceId,
    offset: ByteOffset,
    spellings: &VisibleSpellings,
) -> Result<Option<Box<[SemanticCompletion]>>, SemanticCompletionError> {
    let Some(context_range) = containing_initializer(trees, source, offset) else {
        return Ok(None);
    };
    let mut candidates = Vec::new();
    for binding in index.bindings_in(source) {
        let SemanticEntity::BodyNode(body, node) = binding.entity() else {
            continue;
        };
        let range = binding.origin().span().range();
        if !range.contains_range(context_range) {
            continue;
        }
        let checked = query.checked_operation(body, node)?;
        let CheckedOperation::Aggregate(AggregateConstruction::Struct { definition, fields }) =
            checked
        else {
            continue;
        };
        candidates.push((*binding, (*definition, fields.as_ref())));
    }
    let selected = select_source_candidates(candidates.into_iter()).unique();
    let Some((definition, fields)) = selected else {
        return Ok(None);
    };
    let used_fields = fields.iter().map(|(field, _)| *field).collect::<Vec<_>>();
    let candidates = program.structural_field_completions(definition, source, &used_fields)?;
    Ok(Some(render_completions(
        program.graph(),
        &candidates,
        |field| {
            presentation(program, SemanticEntity::Field(field), spellings)
                .map(|value| Box::<str>::from(value.code()))
        },
    )))
}

pub(super) fn render_recovery_completions(
    graph: &DeclarationGraph,
    candidates: &[StructuralFieldCompletionCandidate],
    detail: impl FnMut(FieldId) -> Option<Box<str>>,
) -> Box<[SemanticCompletion]> {
    render_completions(graph, candidates, detail)
}

fn render_completions(
    graph: &DeclarationGraph,
    candidates: &[StructuralFieldCompletionCandidate],
    mut detail: impl FnMut(FieldId) -> Option<Box<str>>,
) -> Box<[SemanticCompletion]> {
    candidates
        .iter()
        .filter_map(|candidate| {
            let field = candidate.field();
            Some(SemanticCompletion::new(
                graph.symbols().spelling(candidate.name())?,
                SemanticCompletionKind::Field,
                detail(field),
            ))
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn containing_initializer(
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
            (node.kind() == NodeKind::StructInitializer && node.range().contains_cursor(offset))
                .then_some(node.range())
        })
        .min_by_key(|range| range.len())
}
