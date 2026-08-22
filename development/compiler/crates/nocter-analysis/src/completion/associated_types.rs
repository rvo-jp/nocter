use nocter_checking::{
    AssociatedTypeCompletionCandidate, AssociatedTypeCompletionError, CheckedProgram,
    PreparedSemanticProgram,
};
use nocter_declarations::DeclarationGraph;
use nocter_model::{AssociatedTypeId, ModuleId};
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::SemanticEntity;

use super::{SemanticCompletion, SemanticCompletionKind};
use crate::presentation::visible_spelling::VisibleSpellings;
use crate::presentation::{prepared_presentation, presentation};

pub(super) fn checked_completions(
    program: &CheckedProgram,
    source: SourceId,
    offset: ByteOffset,
    module: ModuleId,
) -> Result<Option<Box<[SemanticCompletion]>>, AssociatedTypeCompletionError> {
    let selected = program
        .associated_type_completion_contexts()
        .iter()
        .filter(|context| {
            context.origin().source() == source && contains(context.origin().span().range(), offset)
        })
        .min_by_key(|context| range_length(context.origin().span().range()));
    let Some(context) = selected else {
        return Ok(None);
    };
    let candidates = program.associated_type_completions(context.candidates())?;
    let spellings = VisibleSpellings::new(program.graph(), module);
    Ok(Some(render_completions(
        program.graph(),
        &candidates,
        |associated| {
            presentation(
                program,
                SemanticEntity::AssociatedType(associated),
                &spellings,
            )
            .map(|value| Box::<str>::from(value.code()))
        },
    )?))
}

pub(super) fn render_prepared_completions(
    program: &PreparedSemanticProgram,
    module: ModuleId,
    candidates: &[AssociatedTypeCompletionCandidate],
) -> Result<Box<[SemanticCompletion]>, AssociatedTypeCompletionError> {
    let spellings = VisibleSpellings::new(program.graph(), module);
    render_completions(program.graph(), candidates, |associated| {
        prepared_presentation(
            program,
            SemanticEntity::AssociatedType(associated),
            &spellings,
        )
        .map(|value| Box::<str>::from(value.code()))
    })
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

const fn contains(range: TextRange, offset: ByteOffset) -> bool {
    range.start().get() <= offset.get() && offset.get() <= range.end().get()
}

const fn range_length(range: TextRange) -> u32 {
    range.end().get() - range.start().get()
}
