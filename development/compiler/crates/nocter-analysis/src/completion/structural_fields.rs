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
    let selected = index
        .bindings_in(source)
        .filter_map(|binding| {
            let SemanticEntity::BodyNode(body, node) = binding.entity() else {
                return None;
            };
            let range = binding.origin().span().range();
            if !contains_range(range, context_range) {
                return None;
            }
            let checked = program.bodies().get(body)?.nodes().get(node)?;
            let CheckedOperation::Aggregate(AggregateConstruction::Struct { definition, fields }) =
                checked.operation()
            else {
                return None;
            };
            Some((range, *definition, fields.as_ref()))
        })
        .min_by_key(|(range, _, _)| range_length(*range));
    let Some((_, definition, fields)) = selected else {
        return Ok(None);
    };
    let used_fields = fields.iter().map(|(field, _)| *field).collect::<Vec<_>>();
    let candidates = program.structural_field_completions(definition, module, &used_fields)?;
    let spellings = VisibleSpellings::new(program.graph(), module);
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
    module: ModuleId,
    candidates: &[StructuralFieldCompletionCandidate],
) -> Box<[SemanticCompletion]> {
    let spellings = VisibleSpellings::new(program.graph(), module);
    render_completions(program.graph(), candidates, |field| {
        prepared_presentation(program, SemanticEntity::Field(field), &spellings)
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
            Some(SemanticCompletion {
                label: graph.symbols().spelling(candidate.name())?.into(),
                kind: SemanticCompletionKind::Field,
                detail: detail(field),
            })
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
            (node.kind() == NodeKind::StructInitializer && contains(node.range(), offset))
                .then_some(node.range())
        })
        .min_by_key(|range| range_length(*range))
}

const fn contains(range: TextRange, offset: ByteOffset) -> bool {
    range.start().get() <= offset.get() && offset.get() <= range.end().get()
}

const fn contains_range(outer: TextRange, inner: TextRange) -> bool {
    outer.start().get() <= inner.start().get() && inner.end().get() <= outer.end().get()
}

const fn range_length(range: TextRange) -> u32 {
    range.end().get() - range.start().get()
}
