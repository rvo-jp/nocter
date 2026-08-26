use nocter_checking::{
    AggregateConstruction, CheckedOperation, CheckedProgram, PreparedSemanticProgram,
    StructuralFieldCompletionCandidate, StructuralFieldCompletionError,
};
use nocter_declarations::DeclarationGraph;
use nocter_model::{FieldId, ModuleId};
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceIndex};
use nocter_syntax::{NodeKind, SyntaxTree};

use super::{SemanticCompletion, SemanticCompletionKind};
use crate::presentation::visible_spelling::VisibleSpellings;
use crate::presentation::{prepared_presentation, presentation};
use crate::source_selection::select_source_candidates;

/// Resolves the structural construction containing a checked cursor position.
pub(super) fn checked_completions(
    program: &CheckedProgram,
    index: &SourceIndex,
    trees: &[SyntaxTree],
    source: SourceId,
    offset: ByteOffset,
    module: ModuleId,
) -> Result<Option<Box<[SemanticCompletion]>>, StructuralFieldCompletionError> {
    let Some(context_range) = containing_initializer(trees, source, offset) else {
        return Ok(None);
    };
    let selected = select_source_candidates(index.bindings_in(source).filter_map(|binding| {
        let SemanticEntity::BodyNode(body, node) = binding.entity() else {
            return None;
        };
        let range = binding.origin().span().range();
        if !range.contains_range(context_range) {
            return None;
        }
        let checked = program.bodies().get(body)?.nodes().get(node)?;
        let CheckedOperation::Aggregate(AggregateConstruction::Struct { definition, fields }) =
            checked.operation()
        else {
            return None;
        };
        Some((*binding, (*definition, fields.as_ref())))
    }))
    .unique();
    let Some((definition, fields)) = selected else {
        return Ok(None);
    };
    let used_fields = fields.iter().map(|(field, _)| *field).collect::<Vec<_>>();
    let candidates = program.structural_field_completions(definition, source, &used_fields)?;
    let spellings = VisibleSpellings::for_source(program.graph(), module, index, source);
    Ok(Some(render_completions(
        program.graph(),
        &candidates,
        |field| {
            presentation(program, SemanticEntity::Field(field), &spellings)
                .map(|value| Box::<str>::from(value.code()))
        },
    )))
}

pub(super) fn render_prepared_completions(
    program: &PreparedSemanticProgram,
    spellings: &VisibleSpellings,
    candidates: &[StructuralFieldCompletionCandidate],
) -> Box<[SemanticCompletion]> {
    render_completions(program.graph(), candidates, |field| {
        prepared_presentation(program, SemanticEntity::Field(field), spellings)
            .map(|value| Box::<str>::from(value.code()))
    })
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
