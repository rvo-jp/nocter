use nocter_checking::{
    BodyAnalysisRecovery, CheckedProgram, CheckedProgramOutput, DeclarationAnalysisRecovery,
    NameAnalysisRecovery, PreparationRecovery, SourceOwnershipTable,
};
use nocter_declaration_lowering::DeclarationLoweringRecovery;
use nocter_declarations::DeclarationGraph;
use nocter_model::TypeStore;
use nocter_source_index::SourceIndex;

/// The exact current-generation source-semantic evidence retained by one analysis attempt.
///
/// The phase that happened to produce the evidence is private. Consumers ask for an exact
/// capability and cannot branch on compiler phase order.
#[derive(Debug)]
pub struct SemanticEvidenceBundle {
    authority: SemanticAuthority,
}

#[derive(Debug)]
enum SemanticAuthority {
    Declarations(Box<DeclarationAnalysisRecovery>),
    Names(Box<NameAnalysisRecovery>),
    Bodies(Box<BodyAnalysisRecovery>),
    Checked(Box<CheckedProgramOutput>),
}

/// A borrowed capability contract over one current-generation semantic result.
///
/// Storage variants and compiler phase order remain owned by the session. Analysis consumers use
/// this common view for both a checked target and retained recovery evidence, so they cannot
/// reconstruct the mapping independently.
#[derive(Clone, Copy)]
pub struct SemanticEvidenceView<'a> {
    graph: &'a DeclarationGraph,
    types: &'a TypeStore,
    source_ownership: &'a SourceOwnershipTable,
    source_index: &'a SourceIndex,
    checked: Option<&'a CheckedProgram>,
    bodies: Option<&'a BodyAnalysisRecovery>,
    names: Option<&'a NameAnalysisRecovery>,
    declarations: Option<&'a DeclarationAnalysisRecovery>,
}

impl<'a> SemanticEvidenceView<'a> {
    pub(crate) const fn from_checked(
        checked: &'a CheckedProgram,
        source_index: &'a SourceIndex,
    ) -> Self {
        Self {
            graph: checked.graph(),
            types: checked.types(),
            source_ownership: checked.source_ownership(),
            source_index,
            checked: Some(checked),
            bodies: None,
            names: None,
            declarations: None,
        }
    }

    #[must_use]
    pub const fn graph(self) -> &'a DeclarationGraph {
        self.graph
    }

    #[must_use]
    pub const fn types(self) -> &'a TypeStore {
        self.types
    }

    #[must_use]
    pub const fn source_ownership(self) -> &'a SourceOwnershipTable {
        self.source_ownership
    }

    #[must_use]
    pub const fn source_index(self) -> &'a SourceIndex {
        self.source_index
    }

    #[must_use]
    pub const fn checked(self) -> Option<&'a CheckedProgram> {
        self.checked
    }

    #[must_use]
    pub const fn body_analysis(self) -> Option<&'a BodyAnalysisRecovery> {
        self.bodies
    }

    #[must_use]
    pub const fn name_analysis(self) -> Option<&'a NameAnalysisRecovery> {
        self.names
    }

    #[must_use]
    pub const fn declaration_analysis(self) -> Option<&'a DeclarationAnalysisRecovery> {
        self.declarations
    }
}

impl SemanticEvidenceBundle {
    pub(crate) fn from_declaration_lowering(recovery: DeclarationLoweringRecovery) -> Self {
        let (graph, types, source_ownership, source_index) = recovery.into_declaration_parts();
        Self {
            authority: SemanticAuthority::Declarations(Box::new(
                DeclarationAnalysisRecovery::from_parts(
                    graph,
                    types,
                    source_ownership,
                    source_index,
                ),
            )),
        }
    }

    pub(crate) fn from_preparation(recovery: PreparationRecovery) -> Self {
        match recovery {
            PreparationRecovery::Declarations(recovery) => Self {
                authority: SemanticAuthority::Declarations(recovery),
            },
            PreparationRecovery::Names(recovery) => Self {
                authority: SemanticAuthority::Names(recovery),
            },
        }
    }

    pub(crate) fn from_bodies(recovery: BodyAnalysisRecovery) -> Self {
        Self {
            authority: SemanticAuthority::Bodies(Box::new(recovery)),
        }
    }

    pub(crate) fn from_checked(checked: CheckedProgramOutput) -> Self {
        Self {
            authority: SemanticAuthority::Checked(Box::new(checked)),
        }
    }

    /// Borrows the common analysis contract without exposing the stored phase authority.
    #[must_use]
    pub fn view(&self) -> SemanticEvidenceView<'_> {
        match &self.authority {
            SemanticAuthority::Checked(checked) => {
                SemanticEvidenceView::from_checked(checked.program(), checked.source_index())
            }
            SemanticAuthority::Bodies(analysis) => SemanticEvidenceView {
                graph: analysis.prepared().graph(),
                types: analysis.prepared().types(),
                source_ownership: analysis.prepared().source_ownership(),
                source_index: analysis.source_index(),
                checked: None,
                bodies: Some(analysis),
                names: None,
                declarations: None,
            },
            SemanticAuthority::Names(analysis) => SemanticEvidenceView {
                graph: analysis.graph(),
                types: analysis.types(),
                source_ownership: analysis.source_ownership(),
                source_index: analysis.source_index(),
                checked: None,
                bodies: None,
                names: Some(analysis),
                declarations: None,
            },
            SemanticAuthority::Declarations(analysis) => SemanticEvidenceView {
                graph: analysis.graph(),
                types: analysis.types(),
                source_ownership: analysis.source_ownership(),
                source_index: analysis.source_index(),
                checked: None,
                bodies: None,
                names: None,
                declarations: Some(analysis),
            },
        }
    }

    #[must_use]
    pub fn body_analysis(&self) -> Option<&BodyAnalysisRecovery> {
        self.view().body_analysis()
    }

    #[must_use]
    pub fn name_analysis(&self) -> Option<&NameAnalysisRecovery> {
        self.view().name_analysis()
    }

    #[must_use]
    pub fn declaration_analysis(&self) -> Option<&DeclarationAnalysisRecovery> {
        self.view().declaration_analysis()
    }
}
