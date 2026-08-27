use nocter_checking::{
    BodyAnalysisRecovery, CheckedProgramOutput, DeclarationAnalysisRecovery, NameAnalysisRecovery,
    PreparationRecovery,
};
use nocter_declaration_lowering::DeclarationLoweringRecovery;

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

    #[must_use]
    pub fn checked(&self) -> Option<&CheckedProgramOutput> {
        match &self.authority {
            SemanticAuthority::Checked(checked) => Some(checked),
            SemanticAuthority::Declarations(_)
            | SemanticAuthority::Names(_)
            | SemanticAuthority::Bodies(_) => None,
        }
    }

    #[must_use]
    pub fn body_analysis(&self) -> Option<&BodyAnalysisRecovery> {
        match &self.authority {
            SemanticAuthority::Bodies(analysis) => Some(analysis),
            SemanticAuthority::Declarations(_)
            | SemanticAuthority::Names(_)
            | SemanticAuthority::Checked(_) => None,
        }
    }

    #[must_use]
    pub fn name_analysis(&self) -> Option<&NameAnalysisRecovery> {
        match &self.authority {
            SemanticAuthority::Names(analysis) => Some(analysis),
            SemanticAuthority::Declarations(_)
            | SemanticAuthority::Bodies(_)
            | SemanticAuthority::Checked(_) => None,
        }
    }

    #[must_use]
    pub fn declaration_analysis(&self) -> Option<&DeclarationAnalysisRecovery> {
        match &self.authority {
            SemanticAuthority::Declarations(analysis) => Some(analysis),
            SemanticAuthority::Names(_)
            | SemanticAuthority::Bodies(_)
            | SemanticAuthority::Checked(_) => None,
        }
    }
}
