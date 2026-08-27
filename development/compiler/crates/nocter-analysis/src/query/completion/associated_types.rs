use nocter_checking::{
    AssociatedTypeCompletionCandidate, AssociatedTypeCompletionError, CheckedProgram,
};
use nocter_declarations::DeclarationGraph;
use nocter_model::AssociatedTypeId;
use nocter_source::{ByteOffset, SourceId};
use nocter_source_index::SemanticEntity;

use super::{SemanticCompletion, SemanticCompletionKind};
use crate::query::presentation::presentation;
use crate::query::presentation::visible_spelling::VisibleSpellings;

pub(super) fn checked_completions(
    program: &CheckedProgram,
    source: SourceId,
    offset: ByteOffset,
    spellings: &VisibleSpellings,
) -> Result<Option<Box<[SemanticCompletion]>>, AssociatedTypeCompletionError> {
    let selected = program
        .associated_type_completion_contexts()
        .iter()
        .filter(|context| {
            context.origin().source() == source
                && context.origin().span().range().contains_cursor(offset)
        })
        .min_by_key(|context| context.origin().span().range().len());
    let Some(context) = selected else {
        return Ok(None);
    };
    let candidates = program.associated_type_completions(context.candidates())?;
    Ok(Some(render_completions(
        program.graph(),
        &candidates,
        |associated| {
            presentation(
                program,
                SemanticEntity::AssociatedType(associated),
                spellings,
            )
            .map(|value| Box::<str>::from(value.code()))
        },
    )?))
}

pub(super) fn render_recovery_completions(
    graph: &DeclarationGraph,
    candidates: &[AssociatedTypeCompletionCandidate],
    detail: impl FnMut(AssociatedTypeId) -> Option<Box<str>>,
) -> Result<Box<[SemanticCompletion]>, AssociatedTypeCompletionError> {
    render_completions(graph, candidates, detail)
}

fn render_completions(
    graph: &DeclarationGraph,
    candidates: &[AssociatedTypeCompletionCandidate],
    mut detail: impl FnMut(AssociatedTypeId) -> Option<Box<str>>,
) -> Result<Box<[SemanticCompletion]>, AssociatedTypeCompletionError> {
    candidates
        .iter()
        .map(|candidate| {
            let associated = candidate.associated();
            let label = graph.symbols().spelling(candidate.name()).ok_or(
                AssociatedTypeCompletionError::MissingAssociatedType(associated),
            )?;
            Ok(SemanticCompletion::new(
                label,
                SemanticCompletionKind::Type,
                detail(associated),
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}
