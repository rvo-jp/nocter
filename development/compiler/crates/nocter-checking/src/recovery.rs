use nocter_declarations::DeclarationGraph;
use nocter_frontend_bindings::SourceOwnershipTable;
use nocter_model::{Arena, TypeProjection, TypeStore};
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::SourceIndex;

use crate::body_evidence::{TypedInterruptionEvidence, TypedInterruptionSnapshot};
use crate::member_completion::{MemberCompletionContext, select_member_completions};
use crate::{
    BodyEvidence, ConstructionCompletionCandidate, ConstructionCompletionError,
    MemberCompletionCandidate, MemberCompletionError, PreparedSemanticProgram,
    TypedBodyInterruption, TypedBodyInterruptionKind,
};

/// The complete declaration graph retained when a program-wide preparation rule rejects source.
///
/// This boundary contains no interface implementation, construction, instance-operation, name, or body result.
/// It may retain the independently completed standard contract capability when a later preparation
/// authority rejects source. Tooling can inspect these exact facts without rerunning lowering or
/// pretending that the rejecting authority completed.
#[derive(Debug)]
pub struct DeclarationAnalysisRecovery {
    graph: DeclarationGraph,
    types: TypeStore,
    source_ownership: SourceOwnershipTable,
    source_index: SourceIndex,
    standard_semantics: Option<crate::StandardSemanticTable>,
}

impl DeclarationAnalysisRecovery {
    pub(crate) fn new(
        graph: DeclarationGraph,
        types: TypeStore,
        source_ownership: SourceOwnershipTable,
        source_index: SourceIndex,
        standard_semantics: Option<crate::StandardSemanticTable>,
    ) -> Self {
        Self {
            graph,
            types,
            source_ownership,
            source_index,
            standard_semantics,
        }
    }

    pub(crate) fn current_branch(&self) -> Self {
        Self {
            graph: self.graph.clone(),
            types: self.types.clone(),
            source_ownership: self.source_ownership.clone(),
            source_index: self.source_index.clone(),
            standard_semantics: self.standard_semantics.clone(),
        }
    }

    /// Creates declaration-only analysis from the exact facts retained by the rejecting phase.
    #[must_use]
    pub fn from_parts(
        graph: DeclarationGraph,
        types: TypeStore,
        source_ownership: SourceOwnershipTable,
        source_index: SourceIndex,
    ) -> Self {
        Self::new(graph, types, source_ownership, source_index, None)
    }

    #[must_use]
    pub const fn graph(&self) -> &DeclarationGraph {
        &self.graph
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        &self.types
    }

    #[must_use]
    pub const fn source_ownership(&self) -> &SourceOwnershipTable {
        &self.source_ownership
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    /// Returns the validated standard capability completed before a later preparation failure.
    #[must_use]
    pub const fn standard_semantics(&self) -> Option<&crate::StandardSemanticTable> {
        self.standard_semantics.as_ref()
    }
}

/// Inconsistency between an interruption kind and its retained recovery capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptionEvidenceError;

impl std::fmt::Display for InterruptionEvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("typed interruption recovery evidence is inconsistent")
    }
}

impl std::error::Error for InterruptionEvidenceError {}

/// Immutable current-generation semantic authority retained after typed-body failure.
///
/// The prepared program remains the syntax-independent authority for program-wide semantics.
/// Lexical body names stay in this editor recovery layer. Every authored body failure may add one
/// typed interruption backed by a private transactional type/copyability snapshot. Independently
/// successful bodies retain typed evidence for local editor queries, but they have not passed
/// whole-program ownership, provenance, or target closure and cannot become a checked program.
/// Every declared body owns exactly one [`BodyEvidence`] entry, so authored rejection is never
/// represented by absence.
#[derive(Debug)]
pub struct BodyAnalysisRecovery {
    prepared: PreparedSemanticProgram,
    body_names: Arena<nocter_model::BodyId, crate::ResolvedBodyNames>,
    source_index: SourceIndex,
    bodies: Arena<nocter_model::BodyId, BodyEvidence>,
}

impl BodyAnalysisRecovery {
    pub(crate) fn new(
        prepared: PreparedSemanticProgram,
        body_names: Arena<nocter_model::BodyId, crate::ResolvedBodyNames>,
        source_index: SourceIndex,
        bodies: Arena<nocter_model::BodyId, BodyEvidence>,
    ) -> Self {
        Self {
            prepared,
            body_names,
            source_index,
            bodies,
        }
    }

    #[must_use]
    pub const fn prepared(&self) -> &PreparedSemanticProgram {
        &self.prepared
    }

    /// Returns the lexical body result retained by the editor recovery layer.
    #[must_use]
    pub const fn body_names(&self) -> &Arena<nocter_model::BodyId, crate::ResolvedBodyNames> {
        &self.body_names
    }

    /// Returns the independent source projection retained alongside prepared semantics.
    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    /// Returns the explicit evidence state for one declared body.
    #[must_use]
    pub fn body_evidence(&self, body: nocter_model::BodyId) -> Option<&BodyEvidence> {
        self.bodies.get(body)
    }

    /// Iterates the explicit evidence state of every declared body in canonical identity order.
    #[must_use]
    pub fn body_evidence_iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (nocter_model::BodyId, &BodyEvidence)> {
        self.bodies.iter()
    }

    /// Iterates every source diagnostic that explains a rejected body in canonical body order.
    pub fn rejection_diagnostics(
        &self,
    ) -> impl Iterator<Item = &nocter_diagnostics::SourceDiagnostic> {
        self.bodies
            .iter()
            .filter_map(|(_, evidence)| evidence.rejection()?.diagnostic())
    }

    pub fn interruptions(&self) -> impl Iterator<Item = &TypedBodyInterruption> {
        self.bodies
            .iter()
            .filter_map(|(_, evidence)| evidence.rejection()?.interruption())
    }

    /// Selects the narrowest typed interruption containing one editor position.
    #[must_use]
    pub fn interruption_at(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Option<&TypedBodyInterruption> {
        self.interruption_position_at(source, offset)
            .map(|(_, interruption)| interruption)
    }

    /// Selects the narrowest typed interruption and its stable recovery position.
    #[must_use]
    pub fn interruption_position_at(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Option<(usize, &TypedBodyInterruption)> {
        self.interruptions()
            .enumerate()
            .filter(|(_, interruption)| {
                let origin = interruption.origin();
                origin.source() == source && origin.span().range().contains_cursor(offset)
            })
            .min_by_key(|(_, interruption)| interruption.origin().span().range().len())
    }

    /// Selects the narrowest typed interruption associated with one diagnostic range.
    #[must_use]
    pub fn interruption_position_overlapping(
        &self,
        source: SourceId,
        range: TextRange,
    ) -> Option<(usize, &TypedBodyInterruption)> {
        self.interruptions()
            .enumerate()
            .filter(|(_, interruption)| {
                let origin = interruption.origin();
                let interruption = origin.span().range();
                origin.source() == source
                    && (interruption.overlaps(range)
                        || interruption.contains_range(range)
                        || range.contains_range(interruption))
            })
            .min_by_key(|(_, interruption)| interruption.origin().span().range().len())
    }

    fn snapshot(&self, interruption: &TypedBodyInterruption) -> Option<&TypedInterruptionSnapshot> {
        self.bodies
            .iter()
            .filter_map(|(_, evidence)| evidence.rejection()?.snapshot())
            .find(|typed| typed.interruption == *interruption)
    }

    /// Applies the normal member selector to an exact failed member-selection context.
    #[must_use]
    pub fn interrupted_member_completions(
        &self,
        session: &crate::MemberCompletionQuerySession,
        interruption: &TypedBodyInterruption,
    ) -> Option<Result<Box<[MemberCompletionCandidate]>, MemberCompletionError>> {
        let typed = self.snapshot(interruption)?;
        let TypedBodyInterruptionKind::MemberSelection {
            receiver,
            available,
            owned,
        } = typed.interruption.kind()
        else {
            return None;
        };
        let TypedInterruptionEvidence::MemberSelection(evidence) = &typed.evidence else {
            return Some(Err(MemberCompletionError::InvalidRecoveryEvidence));
        };
        let semantics = &evidence.semantics;
        let body = typed.interruption.body();
        let source = typed.interruption.origin().source();
        if self
            .prepared
            .graph()
            .declarations()
            .bodies()
            .get(body)
            .is_none()
        {
            return Some(Err(MemberCompletionError::MissingBody(body)));
        }
        Some(select_member_completions(
            crate::member_completion::MemberCompletionAuthorities {
                environment: self.prepared.environment(),
                source_access: self.prepared.source_access(),
                semantics,
                session,
            },
            MemberCompletionContext::new(body, source, *receiver, *available, *owned),
        ))
    }

    /// Applies the use-site construction selector to an exact failed construction selection.
    #[must_use]
    pub fn interrupted_construction_completions(
        &self,
        interruption: &TypedBodyInterruption,
        source: SourceId,
    ) -> Option<Result<Box<[ConstructionCompletionCandidate]>, ConstructionCompletionError>> {
        let typed = self.snapshot(interruption)?;
        let TypedBodyInterruptionKind::ConstructionSelection { owner } = typed.interruption.kind()
        else {
            return None;
        };
        Some(self.prepared.construction_completions(*owner, source))
    }

    /// Applies the structural construction selector to fields fixed before a body failure.
    #[must_use]
    pub fn interrupted_structural_field_completions(
        &self,
        interruption: &TypedBodyInterruption,
        source: SourceId,
    ) -> Option<
        Result<
            Box<[crate::StructuralFieldCompletionCandidate]>,
            crate::StructuralFieldCompletionError,
        >,
    > {
        let typed = self.snapshot(interruption)?;
        let TypedBodyInterruptionKind::StructuralConstruction {
            definition,
            initialized,
        } = typed.interruption.kind()
        else {
            return None;
        };
        Some(
            self.prepared
                .structural_field_completions(*definition, source, initialized),
        )
    }

    /// Applies the enum-pattern selector to the target family fixed before pattern failure.
    #[must_use]
    pub fn interrupted_enum_pattern_completions(
        &self,
        interruption: &TypedBodyInterruption,
        source: SourceId,
    ) -> Option<
        Result<Box<[crate::EnumPatternCompletionCandidate]>, crate::EnumPatternCompletionError>,
    > {
        let typed = self.snapshot(interruption)?;
        let TypedBodyInterruptionKind::EnumPattern { definition } = typed.interruption.kind()
        else {
            return None;
        };
        Some(self.prepared.enum_pattern_completions(*definition, source))
    }

    /// Validates associated-type identities fixed before a type-position failure.
    #[must_use]
    pub fn interrupted_associated_type_completions(
        &self,
        interruption: &TypedBodyInterruption,
    ) -> Option<
        Result<
            Box<[crate::AssociatedTypeCompletionCandidate]>,
            crate::AssociatedTypeCompletionError,
        >,
    > {
        let typed = self.snapshot(interruption)?;
        let TypedBodyInterruptionKind::AssociatedTypeProjection { candidates } =
            typed.interruption.kind()
        else {
            return None;
        };
        Some(self.prepared.associated_type_completions(candidates))
    }

    /// Projects an outcome repair type without exposing the checker store that produced it.
    #[must_use]
    pub fn interrupted_outcome_type(
        &self,
        interruption: &TypedBodyInterruption,
    ) -> Option<Result<&TypeProjection, InterruptionEvidenceError>> {
        let typed = self.snapshot(interruption)?;
        let TypedBodyInterruptionKind::OutcomeContract {
            proposed_result: _, ..
        } = typed.interruption.kind()
        else {
            return None;
        };
        let TypedInterruptionEvidence::Outcome(projection) = &typed.evidence else {
            return Some(Err(InterruptionEvidenceError));
        };
        Some(Ok(projection.as_ref()))
    }
}
