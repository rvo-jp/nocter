use std::fmt;

use nocter_checking::{ArgumentPackSegment, CallTarget, CheckedOperation, StaticDispatch};
use nocter_model::{BodyId, BodyNodeId};
use nocter_source::{ByteOffset, SourceId};
use nocter_source_index::SemanticEntity;

use crate::AnalysisSnapshot;
use crate::query::evidence::{EvidenceIntegrityError, SemanticQueryContext};
use crate::query::presentation::{
    SemanticPresentation, StaticSignatureSource, closure_signature_presentation,
    static_signature_presentation,
};
use crate::query::source_context::SourceContextError;
use crate::query::source_selection::{select_source_binding, select_source_candidates};

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
    ) -> Result<Option<SemanticSignatureHelp>, SemanticSignatureError> {
        let Some(authority) = self.semantic_query()? else {
            return Ok(None);
        };
        let index = authority.source_index();
        let from = authority.module_for_source(source)?;
        let spellings = self
            .queries
            .source_spellings(authority.graph(), from, index, source);
        let mut candidates = Vec::new();
        for binding in index.bindings_in(source) {
            let SemanticEntity::BodyNode(body_id, node_id) = binding.entity() else {
                continue;
            };
            let range = binding.origin().span().range();
            if !range.contains_cursor(offset) {
                continue;
            }
            let Ok(operation) = authority.checked_operation(body_id, node_id)?.into_result() else {
                continue;
            };
            let CheckedOperation::Call(call) = operation else {
                continue;
            };
            candidates.push((*binding, (body_id, call)));
        }
        let Some((body_id, call)) = select_source_candidates(candidates.into_iter()).unique()
        else {
            return Ok(None);
        };
        let rendered = match call.target() {
            CallTarget::Static(selection)
            | CallTarget::CallableValue {
                dispatch: selection,
                ..
            } => {
                let source = static_signature_source(&authority, selection.dispatch())?;
                static_signature_presentation(
                    authority.graph(),
                    authority.types(),
                    selection.generic_arguments(),
                    source,
                    &spellings,
                )
            }
            CallTarget::ClosureValue { closure, .. } => {
                authority.complete().and_then(|authority| {
                    closure_signature_presentation(authority.checked(), *closure, &spellings)
                })
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
        let arguments = call
            .arguments()
            .iter()
            .copied()
            .chain(call.pack().into_iter().flat_map(|pack| {
                pack.segments().iter().map(|segment| match segment {
                    ArgumentPackSegment::Value(value) => *value,
                    ArgumentPackSegment::Spread { iteration, .. } => iteration.iterator(),
                })
            }))
            .collect::<Vec<_>>();
        let active_parameter =
            active_parameter(index, source, body_id, &arguments, offset, parameters.len());
        Ok(Some(SemanticSignatureHelp {
            presentation: rendered.presentation,
            parameters,
            active_parameter,
        }))
    }
}

fn static_signature_source<'a>(
    authority: &SemanticQueryContext<'a>,
    dispatch: StaticDispatch,
) -> Result<StaticSignatureSource<'a>, EvidenceIntegrityError> {
    Ok(match dispatch {
        StaticDispatch::Direct(callable)
        | StaticDispatch::InterfaceMethod {
            method: callable, ..
        }
        | StaticDispatch::InterfaceSelfMethod {
            method: callable, ..
        }
        | StaticDispatch::InterfaceDefault {
            method: callable, ..
        }
        | StaticDispatch::OpaqueMethod {
            method: callable, ..
        } => StaticSignatureSource::Callable(callable),
        StaticDispatch::StructuralRequirement { evidence } => {
            let capability = authority
                .capability_evidence(evidence)
                .ok_or(EvidenceIntegrityError::MissingCapabilityEvidence(evidence))?;
            let nocter_checking::CheckedPredicate::Callable { contract, .. } =
                capability.predicate()
            else {
                return Err(EvidenceIntegrityError::InvalidCapabilityEvidencePredicate(
                    evidence,
                ));
            };
            StaticSignatureSource::Contract(contract)
        }
    })
}

/// An internal inconsistency while answering a signature-help query.
#[derive(Debug)]
pub enum SemanticSignatureError {
    SourceContext(SourceContextError),
    Evidence(EvidenceIntegrityError),
}

impl fmt::Display for SemanticSignatureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceContext(error) => error.fmt(formatter),
            Self::Evidence(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticSignatureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceContext(error) => Some(error),
            Self::Evidence(error) => Some(error),
        }
    }
}

impl From<SourceContextError> for SemanticSignatureError {
    fn from(error: SourceContextError) -> Self {
        Self::SourceContext(error)
    }
}

impl From<EvidenceIntegrityError> for SemanticSignatureError {
    fn from(error: EvidenceIntegrityError) -> Self {
        Self::Evidence(error)
    }
}

fn active_parameter(
    index: &nocter_source_index::SourceIndex,
    source: SourceId,
    body: BodyId,
    arguments: &[BodyNodeId],
    offset: ByteOffset,
    parameter_count: usize,
) -> Option<u32> {
    if parameter_count == 0 {
        return None;
    }
    let completed = arguments
        .iter()
        .filter_map(|argument| {
            select_source_binding(
                index
                    .bindings_for(SemanticEntity::BodyNode(body, *argument))
                    .iter(),
                |binding| binding.origin().source() == source,
            )
            .unique()
            .map(|binding| binding.origin().span().range())
        })
        .filter(|range| range.end() < offset)
        .count();
    u32::try_from(completed.min(parameter_count - 1)).ok()
}

#[cfg(test)]
mod tests {
    use nocter_checking::StaticDispatch;
    use nocter_model::{ArenaBuilder, CapabilityEvidenceId};

    use super::static_signature_source;
    use crate::GenerationId;
    use crate::query::evidence::EvidenceIntegrityError;
    use crate::tests::{TempTree, bundled_snapshot};

    #[test]
    fn structural_signature_requires_exact_capability_evidence() {
        let tree = TempTree::new();
        let (_, snapshot) =
            bundled_snapshot(&tree, "func subject(): i32 { 1 }\n", GenerationId::new(58));
        let authority = snapshot
            .semantic_query()
            .expect("valid semantic index")
            .expect("semantic query");
        let mut identities = ArenaBuilder::<CapabilityEvidenceId, ()>::new();
        let missing = loop {
            let candidate = identities.insert(());
            if authority.capability_evidence(candidate).is_none() {
                break candidate;
            }
        };

        assert!(matches!(
            static_signature_source(
                &authority,
                StaticDispatch::StructuralRequirement { evidence: missing }
            ),
            Err(EvidenceIntegrityError::MissingCapabilityEvidence(evidence)) if evidence == missing
        ));
    }
}
