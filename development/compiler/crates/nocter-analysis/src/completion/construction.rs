use nocter_checking::{
    CheckedProgram, ConstructionCompletionCandidate, ConstructionCompletionError,
    ConstructionCompletionTarget, PreparedSemanticProgram,
};
use nocter_declarations::{CallableKind, DeclarationGraph};
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole};

use super::{SemanticCompletion, SemanticCompletionError, SemanticCompletionKind};
use crate::presentation::visible_spelling::VisibleSpellings;
use crate::presentation::{prepared_presentation, presentation};

/// Resolves a selected checked construction member back to its type-owned use-site surface.
pub(super) fn checked_completions(
    program: &CheckedProgram,
    index: &SourceIndex,
    source: SourceId,
    offset: ByteOffset,
    module: nocter_model::ModuleId,
) -> Result<Option<Box<[SemanticCompletion]>>, SemanticCompletionError> {
    let mut selected: Option<(TextRange, ConstructionCompletionTarget)> = None;
    for binding in index.bindings_in(source).filter(|binding| {
        binding.role() == SourceRole::Reference
            && binding.origin().span().range().contains_cursor(offset)
    }) {
        let target = match binding.entity() {
            SemanticEntity::Variant(variant) => ConstructionCompletionTarget::Variant(variant),
            SemanticEntity::Callable(callable) => {
                let declaration = program
                    .graph()
                    .declarations()
                    .callables()
                    .get(callable)
                    .ok_or(ConstructionCompletionError::MissingCallable(callable))?;
                if declaration.kind() != CallableKind::ConstructionFunction {
                    continue;
                }
                ConstructionCompletionTarget::Function(callable)
            }
            _ => continue,
        };
        let range = binding.origin().span().range();
        if selected
            .as_ref()
            .is_none_or(|(current, _)| range.len() < current.len())
        {
            selected = Some((range, target));
        }
    }
    let Some((_, target)) = selected else {
        return Ok(None);
    };
    let owner = program.construction_completion_owner(target)?;
    let candidates = program.construction_completions(owner, source)?;
    let spellings = VisibleSpellings::for_source(program.graph(), module, index, source);
    Ok(Some(render_checked_completions(
        program,
        &spellings,
        &candidates,
    )))
}

fn render_checked_completions(
    program: &CheckedProgram,
    spellings: &VisibleSpellings,
    candidates: &[ConstructionCompletionCandidate],
) -> Box<[SemanticCompletion]> {
    render_completions(program.graph(), candidates, |entity| {
        presentation(program, entity, spellings)
            .map(|presentation| Box::<str>::from(presentation.code()))
    })
}

pub(super) fn render_prepared_completions(
    program: &PreparedSemanticProgram,
    spellings: &VisibleSpellings,
    candidates: &[ConstructionCompletionCandidate],
) -> Box<[SemanticCompletion]> {
    render_completions(program.graph(), candidates, |entity| {
        prepared_presentation(program, entity, spellings)
            .map(|presentation| Box::<str>::from(presentation.code()))
    })
}

fn render_completions(
    graph: &DeclarationGraph,
    candidates: &[ConstructionCompletionCandidate],
    mut detail: impl FnMut(SemanticEntity) -> Option<Box<str>>,
) -> Box<[SemanticCompletion]> {
    candidates
        .iter()
        .filter_map(|candidate| {
            let label = graph.symbols().spelling(candidate.name())?;
            let (kind, entity) = match candidate.target() {
                ConstructionCompletionTarget::Variant(variant) => (
                    SemanticCompletionKind::EnumMember,
                    SemanticEntity::Variant(variant),
                ),
                ConstructionCompletionTarget::Function(callable) => (
                    SemanticCompletionKind::Constructor,
                    SemanticEntity::Callable(callable),
                ),
            };
            Some(SemanticCompletion::new(label, kind, detail(entity)))
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}
