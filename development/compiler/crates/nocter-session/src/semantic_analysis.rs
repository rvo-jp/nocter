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
    authority: SemanticAuthorityView<'a>,
}

#[derive(Clone, Copy)]
enum SemanticAuthorityView<'a> {
    Declarations(&'a DeclarationAnalysisRecovery),
    Names(&'a NameAnalysisRecovery),
    Bodies(&'a BodyAnalysisRecovery),
    Checked {
        program: &'a CheckedProgram,
        source_index: &'a SourceIndex,
    },
}

impl<'a> SemanticEvidenceView<'a> {
    pub(crate) const fn from_checked(
        checked: &'a CheckedProgram,
        source_index: &'a SourceIndex,
    ) -> Self {
        Self {
            authority: SemanticAuthorityView::Checked {
                program: checked,
                source_index,
            },
        }
    }

    #[must_use]
    pub fn graph(self) -> &'a DeclarationGraph {
        match self.authority {
            SemanticAuthorityView::Declarations(analysis) => analysis.graph(),
            SemanticAuthorityView::Names(analysis) => analysis.graph(),
            SemanticAuthorityView::Bodies(analysis) => analysis.prepared().graph(),
            SemanticAuthorityView::Checked { program, .. } => program.graph(),
        }
    }

    #[must_use]
    pub fn types(self) -> &'a TypeStore {
        match self.authority {
            SemanticAuthorityView::Declarations(analysis) => analysis.types(),
            SemanticAuthorityView::Names(analysis) => analysis.types(),
            SemanticAuthorityView::Bodies(analysis) => analysis.prepared().types(),
            SemanticAuthorityView::Checked { program, .. } => program.types(),
        }
    }

    #[must_use]
    pub fn source_ownership(self) -> &'a SourceOwnershipTable {
        match self.authority {
            SemanticAuthorityView::Declarations(analysis) => analysis.source_ownership(),
            SemanticAuthorityView::Names(analysis) => analysis.source_ownership(),
            SemanticAuthorityView::Bodies(analysis) => analysis.prepared().source_ownership(),
            SemanticAuthorityView::Checked { program, .. } => program.source_ownership(),
        }
    }

    #[must_use]
    pub fn source_index(self) -> &'a SourceIndex {
        match self.authority {
            SemanticAuthorityView::Declarations(analysis) => analysis.source_index(),
            SemanticAuthorityView::Names(analysis) => analysis.source_index(),
            SemanticAuthorityView::Bodies(analysis) => analysis.source_index(),
            SemanticAuthorityView::Checked { source_index, .. } => source_index,
        }
    }

    #[must_use]
    pub const fn checked(self) -> Option<&'a CheckedProgram> {
        match self.authority {
            SemanticAuthorityView::Checked { program, .. } => Some(program),
            SemanticAuthorityView::Declarations(_)
            | SemanticAuthorityView::Names(_)
            | SemanticAuthorityView::Bodies(_) => None,
        }
    }

    #[must_use]
    pub const fn body_analysis(self) -> Option<&'a BodyAnalysisRecovery> {
        match self.authority {
            SemanticAuthorityView::Bodies(analysis) => Some(analysis),
            SemanticAuthorityView::Declarations(_)
            | SemanticAuthorityView::Names(_)
            | SemanticAuthorityView::Checked { .. } => None,
        }
    }

    #[must_use]
    pub const fn name_analysis(self) -> Option<&'a NameAnalysisRecovery> {
        match self.authority {
            SemanticAuthorityView::Names(analysis) => Some(analysis),
            SemanticAuthorityView::Declarations(_)
            | SemanticAuthorityView::Bodies(_)
            | SemanticAuthorityView::Checked { .. } => None,
        }
    }

    #[must_use]
    pub const fn declaration_analysis(self) -> Option<&'a DeclarationAnalysisRecovery> {
        match self.authority {
            SemanticAuthorityView::Declarations(analysis) => Some(analysis),
            SemanticAuthorityView::Names(_)
            | SemanticAuthorityView::Bodies(_)
            | SemanticAuthorityView::Checked { .. } => None,
        }
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
                authority: SemanticAuthorityView::Bodies(analysis),
            },
            SemanticAuthority::Names(analysis) => SemanticEvidenceView {
                authority: SemanticAuthorityView::Names(analysis),
            },
            SemanticAuthority::Declarations(analysis) => SemanticEvidenceView {
                authority: SemanticAuthorityView::Declarations(analysis),
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
