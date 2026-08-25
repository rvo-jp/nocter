use nocter_declarations::DeclarationGraph;
use nocter_model::{Arena, TypeProjection, TypeProjectionError, TypeStore};
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::SourceIndex;

use crate::member_completion::select_member_completions;
use crate::{
    CheckedBody, ConstructionCompletionCandidate, ConstructionCompletionError, CopyabilityTable,
    MemberCompletionCandidate, MemberCompletionContext, MemberCompletionError,
    PreparedSemanticProgram, TypedBodyInterruption, TypedBodyInterruptionKind,
};

/// The deepest semantic stage retained when checking preparation rejects source.
#[derive(Debug)]
pub enum PreparationRecovery {
    Declarations(Box<DeclarationAnalysisRecovery>),
    Names(Box<crate::NameAnalysisRecovery>),
}

/// The complete declaration graph retained when a program-wide preparation rule rejects source.
///
/// This boundary contains no conformance, construction, instance-operation, name, or body result.
/// It exists so tooling can inspect the exact declaration identities involved in the failure
/// without rerunning lowering or pretending that a later semantic authority was completed.
#[derive(Debug)]
pub struct DeclarationAnalysisRecovery {
    graph: DeclarationGraph,
    types: TypeStore,
    source_index: SourceIndex,
}

impl DeclarationAnalysisRecovery {
    pub(crate) fn new(
        graph: DeclarationGraph,
        types: TypeStore,
        source_index: SourceIndex,
    ) -> Self {
        Self {
            graph,
            types,
            source_index,
        }
    }

    /// Creates declaration-only analysis from the exact facts retained by the rejecting phase.
    #[must_use]
    pub fn from_parts(
        graph: DeclarationGraph,
        types: TypeStore,
        source_index: SourceIndex,
    ) -> Self {
        Self::new(graph, types, source_index)
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
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }
}

#[derive(Debug)]
struct TypedInterruptionSnapshot {
    interruption: TypedBodyInterruption,
    types: TypeStore,
    copyabilities: CopyabilityTable,
}

/// Immutable current-generation semantic authority retained after typed-body failure.
///
/// The prepared program remains the authority for declarations, names, and scopes. Every authored
/// body failure may add one typed interruption backed by a private transactional type/copyability
/// snapshot. Independently successful bodies are retained sparsely for local editor queries, but
/// they have not passed whole-program ownership, provenance, or target closure and therefore
/// cannot be promoted to a checked program.
#[derive(Debug)]
pub struct BodyAnalysisRecovery {
    prepared: PreparedSemanticProgram,
    typed: Box<[TypedInterruptionSnapshot]>,
    bodies: Arena<nocter_model::BodyId, Option<CheckedBody>>,
}

impl BodyAnalysisRecovery {
    pub(crate) fn new(
        prepared: PreparedSemanticProgram,
        typed: Vec<(TypedBodyInterruption, TypeStore, CopyabilityTable)>,
        bodies: Arena<nocter_model::BodyId, Option<CheckedBody>>,
    ) -> Self {
        Self {
            prepared,
            typed: typed
                .into_iter()
                .map(
                    |(interruption, types, copyabilities)| TypedInterruptionSnapshot {
                        interruption,
                        types,
                        copyabilities,
                    },
                )
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            bodies,
        }
    }

    #[must_use]
    pub const fn prepared(&self) -> &PreparedSemanticProgram {
        &self.prepared
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
        self.typed
            .iter()
            .filter(|typed| {
                let origin = typed.interruption.origin();
                origin.source() == source && origin.span().range().contains_cursor(offset)
            })
            .min_by_key(|typed| typed.interruption.origin().span().range().len())
            .map(|typed| &typed.interruption)
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
        let body = typed.interruption.body();
        let owner = match self.prepared.graph().declarations().bodies().get(body) {
            Some(body) => body.owner(),
            None => return Some(Err(MemberCompletionError::MissingBody(body))),
        };
        Some(select_member_completions(
            crate::member_completion::MemberCompletionAuthorities {
                graph: self.prepared.graph(),
                types: &typed.types,
                conformances: self.prepared.conformances(),
                instance_operations: self.prepared.instance_operations(),
                declaration_patterns: self.prepared.declaration_patterns(),
                copyabilities: &typed.copyabilities,
                source_access: self.prepared.source_access(),
            },
            MemberCompletionContext::new(owner, source, *receiver, *available, *owned),
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
    ) -> Option<Result<TypeProjection, TypeProjectionError>> {
        let typed = self.snapshot(interruption)?;
        let TypedBodyInterruptionKind::OutcomeContract {
            proposed_result, ..
        } = typed.interruption.kind()
        else {
            return None;
        };
        Some(typed.types.project(*proposed_result))
    }
}
