use nocter_declarations::{DeclarationGraph, DeclarationProgram};
use nocter_model::TypeStore;
use nocter_source_index::SourceIndex;

use crate::member_completion::select_member_completions;
use crate::{
    ConstructionCompletionCandidate, ConstructionCompletionError, CopyabilityTable,
    MemberCompletionCandidate, MemberCompletionContext, MemberCompletionError,
    PreparedSemanticProgram, TypedBodyInterruption, TypedBodyInterruptionKind,
};

/// The exact semantic stage retained by one failed editor analysis generation.
#[derive(Debug)]
pub enum SemanticAnalysisRecovery {
    Declarations(Box<DeclarationAnalysisRecovery>),
    Names(Box<crate::NameAnalysisRecovery>),
    Bodies(Box<BodyAnalysisRecovery>),
}

impl SemanticAnalysisRecovery {
    #[must_use]
    pub fn names(&self) -> Option<&crate::NameAnalysisRecovery> {
        match self {
            Self::Names(recovery) => Some(recovery.as_ref()),
            Self::Declarations(_) | Self::Bodies(_) => None,
        }
    }

    #[must_use]
    pub fn declarations(&self) -> Option<&DeclarationAnalysisRecovery> {
        match self {
            Self::Declarations(recovery) => Some(recovery.as_ref()),
            Self::Names(_) | Self::Bodies(_) => None,
        }
    }

    #[must_use]
    pub fn bodies(&self) -> Option<&BodyAnalysisRecovery> {
        match self {
            Self::Bodies(recovery) => Some(recovery.as_ref()),
            Self::Declarations(_) | Self::Names(_) => None,
        }
    }
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

    /// Converts a structurally valid declaration program rejected by an authored language rule
    /// into the declaration-only editor authority.
    #[must_use]
    pub fn from_program(program: DeclarationProgram, source_index: SourceIndex) -> Self {
        let (graph, types) = program.into_parts();
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

/// The deepest immutable current-generation semantic state retained after typed-body failure.
///
/// The prepared program remains the authority for declarations, names, and scopes. A typed
/// interruption additionally owns the monotonic type/copyability stores used at the exact failed
/// operation; it never masquerades as a checked body or supplies dispatch for invalid source.
#[derive(Debug)]
pub struct BodyAnalysisRecovery {
    prepared: PreparedSemanticProgram,
    typed: Option<TypedInterruptionSnapshot>,
}

impl BodyAnalysisRecovery {
    pub(crate) fn new(
        prepared: PreparedSemanticProgram,
        typed: Option<(TypedBodyInterruption, TypeStore, CopyabilityTable)>,
    ) -> Self {
        Self {
            prepared,
            typed: typed.map(
                |(interruption, types, copyabilities)| TypedInterruptionSnapshot {
                    interruption,
                    types,
                    copyabilities,
                },
            ),
        }
    }

    #[must_use]
    pub const fn prepared(&self) -> &PreparedSemanticProgram {
        &self.prepared
    }

    #[must_use]
    pub fn interruption(&self) -> Option<&TypedBodyInterruption> {
        self.typed.as_ref().map(|typed| &typed.interruption)
    }

    /// Returns the exact monotonic type store reached by this failed generation.
    #[must_use]
    pub fn types(&self) -> &TypeStore {
        self.typed
            .as_ref()
            .map_or_else(|| self.prepared.types(), |typed| &typed.types)
    }

    /// Applies the normal member selector to an exact failed member-selection context.
    #[must_use]
    pub fn interrupted_member_completions(
        &self,
        source: nocter_source::SourceId,
    ) -> Option<Result<Box<[MemberCompletionCandidate]>, MemberCompletionError>> {
        let typed = self.typed.as_ref()?;
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
            self.prepared.graph(),
            &typed.types,
            self.prepared.conformances(),
            self.prepared.instance_operations(),
            &typed.copyabilities,
            self.prepared.source_access(),
            MemberCompletionContext::new(owner, source, *receiver, *available, *owned),
        ))
    }

    /// Applies the use-site construction selector to an exact failed construction selection.
    #[must_use]
    pub fn interrupted_construction_completions(
        &self,
        source: nocter_source::SourceId,
    ) -> Option<Result<Box<[ConstructionCompletionCandidate]>, ConstructionCompletionError>> {
        let typed = self.typed.as_ref()?;
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
        source: nocter_source::SourceId,
    ) -> Option<
        Result<
            Box<[crate::StructuralFieldCompletionCandidate]>,
            crate::StructuralFieldCompletionError,
        >,
    > {
        let typed = self.typed.as_ref()?;
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
        source: nocter_source::SourceId,
    ) -> Option<
        Result<Box<[crate::EnumPatternCompletionCandidate]>, crate::EnumPatternCompletionError>,
    > {
        let typed = self.typed.as_ref()?;
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
    ) -> Option<
        Result<
            Box<[crate::AssociatedTypeCompletionCandidate]>,
            crate::AssociatedTypeCompletionError,
        >,
    > {
        let typed = self.typed.as_ref()?;
        let TypedBodyInterruptionKind::AssociatedTypeProjection { candidates } =
            typed.interruption.kind()
        else {
            return None;
        };
        Some(self.prepared.associated_type_completions(candidates))
    }
}
