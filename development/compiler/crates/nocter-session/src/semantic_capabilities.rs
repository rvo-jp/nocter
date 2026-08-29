use nocter_checking::{BodyAnalysisRecovery, CheckedProgram, DeclarationAnalysisRecovery};
use nocter_declarations::DeclarationGraph;
use nocter_model::TypeStore;
use nocter_source::SourceId;
use nocter_source_index::SourceIndex;

/// Complete semantics required by queries that cannot operate on recovery evidence.
#[derive(Clone, Copy)]
pub struct CompleteSemanticEvidenceView<'a> {
    pub(super) program: &'a CheckedProgram,
    pub(super) source_index: &'a SourceIndex,
}

impl<'a> CompleteSemanticEvidenceView<'a> {
    #[must_use]
    pub const fn program(self) -> &'a CheckedProgram {
        self.program
    }

    #[must_use]
    pub const fn source_index(self) -> &'a SourceIndex {
        self.source_index
    }
}

/// Exact typed-body capability retained for one declared body.
#[derive(Clone, Copy, Debug)]
pub enum SemanticTypedBodyView<'a> {
    Available(&'a nocter_checking::CheckedBody),
    BodyRejected,
    NamesRejected,
    TypingNotReached,
}

/// Exact lexical-name capability retained for one declared body.
#[derive(Clone, Copy, Debug)]
pub enum SemanticBodyNamesView<'a> {
    Available(&'a nocter_checking::ResolvedBodyNames),
    NamesRejected,
    NameResolutionNotReached,
}

/// A typed interruption selected from the current semantic evidence.
#[derive(Clone, Copy)]
pub struct SemanticInterruptionView<'a> {
    pub(super) recovery: &'a BodyAnalysisRecovery,
    pub(super) index: usize,
    pub(super) interruption: &'a nocter_checking::TypedBodyInterruption,
}

impl<'a> SemanticInterruptionView<'a> {
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    #[must_use]
    pub const fn body(self) -> nocter_model::BodyId {
        self.interruption.body()
    }

    #[must_use]
    pub const fn kind(self) -> &'a nocter_checking::TypedBodyInterruptionKind {
        self.interruption.kind()
    }

    #[must_use]
    pub const fn origin(self) -> nocter_source_index::SourceOrigin {
        self.interruption.origin()
    }

    #[must_use]
    pub fn graph(self) -> &'a DeclarationGraph {
        self.recovery.prepared().graph()
    }

    #[must_use]
    pub fn types(self) -> &'a TypeStore {
        self.recovery.prepared().types()
    }

    #[must_use]
    pub const fn source_index(self) -> &'a SourceIndex {
        self.recovery.source_index()
    }

    #[must_use]
    pub fn member_completions(
        self,
        session: &nocter_checking::MemberCompletionQuerySession,
    ) -> Option<
        Result<
            Box<[nocter_checking::MemberCompletionCandidate]>,
            nocter_checking::MemberCompletionError,
        >,
    > {
        self.recovery
            .interrupted_member_completions(session, self.interruption)
    }

    #[must_use]
    pub fn construction_completions(
        self,
        source: SourceId,
    ) -> Option<
        Result<
            Box<[nocter_checking::ConstructionCompletionCandidate]>,
            nocter_checking::ConstructionCompletionError,
        >,
    > {
        self.recovery
            .interrupted_construction_completions(self.interruption, source)
    }

    #[must_use]
    pub fn structural_field_completions(
        self,
        source: SourceId,
    ) -> Option<
        Result<
            Box<[nocter_checking::StructuralFieldCompletionCandidate]>,
            nocter_checking::StructuralFieldCompletionError,
        >,
    > {
        self.recovery
            .interrupted_structural_field_completions(self.interruption, source)
    }

    #[must_use]
    pub fn enum_pattern_completions(
        self,
        source: SourceId,
    ) -> Option<
        Result<
            Box<[nocter_checking::EnumPatternCompletionCandidate]>,
            nocter_checking::EnumPatternCompletionError,
        >,
    > {
        self.recovery
            .interrupted_enum_pattern_completions(self.interruption, source)
    }

    #[must_use]
    pub fn associated_type_completions(
        self,
    ) -> Option<
        Result<
            Box<[nocter_checking::AssociatedTypeCompletionCandidate]>,
            nocter_checking::AssociatedTypeCompletionError,
        >,
    > {
        self.recovery
            .interrupted_associated_type_completions(self.interruption)
    }

    #[must_use]
    pub fn outcome_type(
        self,
    ) -> Option<Result<&'a nocter_model::TypeProjection, nocter_checking::InterruptionEvidenceError>>
    {
        self.recovery.interrupted_outcome_type(self.interruption)
    }
}

/// Declaration facts that authorize one interface-implementation repair.
#[derive(Clone, Copy)]
pub struct InterfaceImplementationRepairView<'a> {
    pub(super) analysis: &'a DeclarationAnalysisRecovery,
    pub(super) missing: &'a nocter_checking::MissingInterfaceImplementationMethods,
}

impl<'a> InterfaceImplementationRepairView<'a> {
    #[must_use]
    pub const fn missing(self) -> &'a nocter_checking::MissingInterfaceImplementationMethods {
        self.missing
    }

    #[must_use]
    pub const fn graph(self) -> &'a DeclarationGraph {
        self.analysis.graph()
    }

    #[must_use]
    pub const fn types(self) -> &'a TypeStore {
        self.analysis.types()
    }

    #[must_use]
    pub const fn source_index(self) -> &'a SourceIndex {
        self.analysis.source_index()
    }

    #[must_use]
    pub fn process_abort(self) -> Option<nocter_model::CallableId> {
        use nocter_toolchain_contract::StandardDeclarationRole;
        self.analysis
            .standard_semantics()
            .and_then(|standard| standard.callable(StandardDeclarationRole::ProcessAbort))
    }
}
