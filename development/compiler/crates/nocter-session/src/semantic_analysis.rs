use nocter_checking::{
    BodyAnalysisRecovery, CheckedProgramOutput, DeclarationAnalysisRecovery, NameAnalysisRecovery,
    PreparationRecovery,
};
use nocter_declaration_lowering::DeclarationLoweringRecovery;

/// The deepest completed source-semantic stage retained by one analysis attempt.
///
/// This sum belongs to session composition rather than an individual compiler phase. Each variant
/// contains only the immutable contract produced by its owning phase and cannot enter a later
/// production transition.
#[derive(Debug)]
pub enum SemanticAnalysis {
    Declarations(Box<DeclarationAnalysisRecovery>),
    Names(Box<NameAnalysisRecovery>),
    Bodies(Box<BodyAnalysisRecovery>),
    Checked(Box<CheckedProgramOutput>),
}

impl SemanticAnalysis {
    pub(crate) fn from_declaration_lowering(recovery: DeclarationLoweringRecovery) -> Self {
        let (graph, types, source_index) = recovery.into_declaration_parts();
        Self::Declarations(Box::new(DeclarationAnalysisRecovery::from_parts(
            graph,
            types,
            source_index,
        )))
    }

    pub(crate) fn from_preparation(recovery: PreparationRecovery) -> Self {
        match recovery {
            PreparationRecovery::Declarations(recovery) => Self::Declarations(recovery),
            PreparationRecovery::Names(recovery) => Self::Names(recovery),
        }
    }

    pub(crate) fn from_bodies(recovery: BodyAnalysisRecovery) -> Self {
        Self::Bodies(Box::new(recovery))
    }

    pub(crate) fn from_checked(checked: CheckedProgramOutput) -> Self {
        Self::Checked(Box::new(checked))
    }

    #[must_use]
    pub fn declarations(&self) -> Option<&DeclarationAnalysisRecovery> {
        match self {
            Self::Declarations(analysis) => Some(analysis),
            Self::Names(_) | Self::Bodies(_) | Self::Checked(_) => None,
        }
    }

    #[must_use]
    pub fn names(&self) -> Option<&NameAnalysisRecovery> {
        match self {
            Self::Names(analysis) => Some(analysis),
            Self::Declarations(_) | Self::Bodies(_) | Self::Checked(_) => None,
        }
    }

    #[must_use]
    pub fn bodies(&self) -> Option<&BodyAnalysisRecovery> {
        match self {
            Self::Bodies(analysis) => Some(analysis),
            Self::Declarations(_) | Self::Names(_) | Self::Checked(_) => None,
        }
    }

    #[must_use]
    pub fn checked(&self) -> Option<&CheckedProgramOutput> {
        match self {
            Self::Checked(analysis) => Some(analysis),
            Self::Declarations(_) | Self::Names(_) | Self::Bodies(_) => None,
        }
    }
}
