use nocter_declarations::DeclarationGraph;
use nocter_frontend_bindings::SourceOwnershipTable;
use nocter_model::{Arena, TypeProjection, TypeStore};
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::SourceIndex;

use crate::member_completion::{MemberCompletionContext, select_member_completions};
use crate::{
    CheckedBody, ConstructionCompletionCandidate, ConstructionCompletionError,
    MemberCompletionCandidate, MemberCompletionError, PreparedSemanticProgram,
    TypedBodyInterruption, TypedBodyInterruptionKind,
};

/// The deepest semantic stage retained when checking preparation rejects source.
#[derive(Debug)]
pub enum PreparationRecovery {
    Declarations(Box<DeclarationAnalysisRecovery>),
    Names(Box<crate::NameAnalysisRecovery>),
}

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

#[derive(Debug)]
struct TypedInterruptionSnapshot {
    interruption: TypedBodyInterruption,
    evidence: TypedInterruptionEvidence,
}

/// Exact semantic capability retained for one typed interruption.
///
/// Most completion kinds are fully described by the interruption itself. Member selection needs
/// its provisional type/copy authority, while outcome repair needs only one closed type
/// projection. Keeping these cases distinct prevents unrelated editor failures from retaining an
/// entire mutable checker store.
#[derive(Debug)]
pub(crate) enum TypedInterruptionEvidence {
    None,
    MemberSelection(Box<MemberInterruptionEvidence>),
    Outcome(Box<TypeProjection>),
}

#[derive(Debug)]
pub(crate) struct MemberInterruptionEvidence {
    pub(crate) semantics: crate::semantic_authority::SemanticAuthority,
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
/// successful bodies are retained sparsely for local editor queries, but they have not passed
/// whole-program ownership, provenance, or target closure and cannot become a checked program.
#[derive(Debug)]
pub struct BodyAnalysisRecovery {
    prepared: PreparedSemanticProgram,
    body_names: Arena<nocter_model::BodyId, crate::ResolvedBodyNames>,
    source_index: SourceIndex,
    typed: Box<[TypedInterruptionSnapshot]>,
    bodies: Arena<nocter_model::BodyId, Option<CheckedBody>>,
}

impl BodyAnalysisRecovery {
    pub(crate) fn new(
        prepared: PreparedSemanticProgram,
        body_names: Arena<nocter_model::BodyId, crate::ResolvedBodyNames>,
        source_index: SourceIndex,
        typed: Vec<(TypedBodyInterruption, TypedInterruptionEvidence)>,
        bodies: Arena<nocter_model::BodyId, Option<CheckedBody>>,
    ) -> Self {
        Self {
            prepared,
            body_names,
            source_index,
            typed: typed
                .into_iter()
                .map(|(interruption, evidence)| TypedInterruptionSnapshot {
                    interruption,
                    evidence,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
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

    /// Returns typed facts for one independently successful body. The sparse body is analysis
    /// evidence only: it has not passed whole-program ownership, provenance, or target closure and
    /// cannot be converted into a [`crate::CheckedProgram`].
    #[must_use]
    pub fn body(&self, body: nocter_model::BodyId) -> Option<&CheckedBody> {
        self.bodies.get(body)?.as_ref()
    }

    #[must_use]
    pub fn interruptions(&self) -> impl ExactSizeIterator<Item = &TypedBodyInterruption> {
        self.typed.iter().map(|typed| &typed.interruption)
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
        self.typed
            .iter()
            .enumerate()
            .filter(|typed| {
                let origin = typed.1.interruption.origin();
                origin.source() == source && origin.span().range().contains_cursor(offset)
            })
            .min_by_key(|typed| typed.1.interruption.origin().span().range().len())
            .map(|(position, typed)| (position, &typed.interruption))
    }

    /// Selects the narrowest typed interruption associated with one diagnostic range.
    #[must_use]
    pub fn interruption_overlapping(
        &self,
        source: SourceId,
        range: TextRange,
    ) -> Option<&TypedBodyInterruption> {
        self.typed
            .iter()
            .filter(|typed| {
                let origin = typed.interruption.origin();
                let interruption = origin.span().range();
                origin.source() == source
                    && (interruption.overlaps(range)
                        || interruption.contains_range(range)
                        || range.contains_range(interruption))
            })
            .min_by_key(|typed| typed.interruption.origin().span().range().len())
            .map(|typed| &typed.interruption)
    }

    fn snapshot(&self, interruption: &TypedBodyInterruption) -> Option<&TypedInterruptionSnapshot> {
        self.typed
            .iter()
            .find(|typed| typed.interruption == *interruption)
    }

    /// Applies the normal member selector to an exact failed member-selection context.
    #[must_use]
    pub fn interrupted_member_completions(
        &self,
        session: &crate::MemberCompletionQuerySession,
        interruption: &TypedBodyInterruption,
        source: SourceId,
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
                graph: self.prepared.graph(),
                semantics,
                interface_implementations: self.prepared.interface_implementations(),
                instance_operations: self.prepared.instance_operations(),
                body_assumptions: self.prepared.body_assumptions(),
                source_access: self.prepared.source_access(),
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
