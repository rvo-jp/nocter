use nocter_checking::{CallTarget, CheckedOperation};
use nocter_model::{BodyId, BodyNodeId};
use nocter_source::{ByteOffset, SourceId};
use nocter_source_index::SemanticEntity;

use crate::AnalysisSnapshot;
use crate::presentation::{
    SemanticPresentation, closure_signature_presentation, static_signature_presentation,
};
use crate::source_context::{SourceContext, SourceContextError};

/// One compiler-selected call signature and active authored argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSignatureHelp {
    presentation: SemanticPresentation,
    parameters: Box<[SemanticParameterLabel]>,
    active_parameter: Option<u32>,
}

/// One byte range within the compiler-rendered signature label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticParameterLabel {
    start: usize,
    end: usize,
}

impl SemanticParameterLabel {
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }
}

impl SemanticSignatureHelp {
    #[must_use]
    pub const fn presentation(&self) -> &SemanticPresentation {
        &self.presentation
    }

    #[must_use]
    pub const fn parameters(&self) -> &[SemanticParameterLabel] {
        &self.parameters
    }

    #[must_use]
    pub const fn active_parameter(&self) -> Option<u32> {
        self.active_parameter
    }
}

impl AnalysisSnapshot {
    /// Selects the innermost checked call containing `offset`.
    ///
    /// # Errors
    ///
    /// Returns an internal context error when a reached source has no unique semantic module.
    pub fn semantic_signature_help(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Result<Option<SemanticSignatureHelp>, SourceContextError> {
        let Some(target) = self.target() else {
            return Ok(None);
        };
        let checked = target.program().checked();
        let Some(index) = self.source_index() else {
            return Ok(None);
        };
        let from = SourceContext::resolve(index, source)?.module();
        let Some((body_id, _node_id, _range, call)) = index
            .bindings_in(source)
            .filter_map(|binding| {
                let SemanticEntity::BodyNode(body_id, node_id) = binding.entity() else {
                    return None;
                };
                let range = binding.origin().span().range();
                if !range.contains_cursor(offset) {
                    return None;
                }
                let node = checked.bodies().get(body_id)?.nodes().get(node_id)?;
                let CheckedOperation::Call(call) = node.operation() else {
                    return None;
                };
                Some((body_id, node_id, range, call))
            })
            .min_by_key(|(_, _, range, _)| range.len())
        else {
            return Ok(None);
        };
        let rendered = match call.target() {
            CallTarget::Static(selection)
            | CallTarget::CallableValue {
                dispatch: selection,
                ..
            } => static_signature_presentation(checked, selection, from),
            CallTarget::ClosureValue { closure, .. } => {
                closure_signature_presentation(checked, *closure, from)
            }
        };
        let Some(rendered) = rendered else {
            return Ok(None);
        };
        let parameters = rendered
            .parameter_ranges
            .into_vec()
            .into_iter()
            .map(|(start, end)| SemanticParameterLabel { start, end })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let active_parameter = active_parameter(
            self,
            source,
            body_id,
            call.arguments(),
            offset,
            parameters.len(),
        );
        Ok(Some(SemanticSignatureHelp {
            presentation: rendered.presentation,
            parameters,
            active_parameter,
        }))
    }
}

fn active_parameter(
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    body: BodyId,
    arguments: &[BodyNodeId],
    offset: ByteOffset,
    parameter_count: usize,
) -> Option<u32> {
    if parameter_count == 0 {
        return None;
    }
    let index = snapshot.source_index()?;
    let completed = arguments
        .iter()
        .filter_map(|argument| {
            index
                .bindings_for(SemanticEntity::BodyNode(body, *argument))
                .iter()
                .filter(|binding| binding.origin().source() == source)
                .map(|binding| binding.origin().span().range())
                .min_by_key(|range| range.len())
        })
        .filter(|range| range.end() < offset)
        .count();
    u32::try_from(completed.min(parameter_count - 1)).ok()
}
