use nocter_checking::{
    BodyAnalysisRecovery, CheckedProgramOutput, DeclarationAnalysisRecovery, NameAnalysisRecovery,
    PreparationRecovery,
};
use nocter_declaration_lowering::DeclarationLoweringRecovery;

/// The exact current-generation source-semantic evidence retained by one analysis attempt.
///
/// This sum belongs to session composition rather than an individual compiler phase. Each variant
/// contains only the immutable evidence contract produced by its owner and cannot enter a later
/// production transition. Editor features do not consume or compare these variants directly;
/// analysis converts them into capability-bearing query contexts.
#[derive(Debug)]
pub enum SemanticAnalysis {
    Declarations(Box<DeclarationAnalysisRecovery>),
    Names(Box<NameAnalysisRecovery>),
    Bodies(Box<BodyAnalysisRecovery>),
    Checked(Box<CheckedProgramOutput>),
}

impl SemanticAnalysis {
    pub(crate) fn from_declaration_lowering(recovery: DeclarationLoweringRecovery) -> Self {
        let (graph, types, source_ownership, source_index) = recovery.into_declaration_parts();
        Self::Declarations(Box::new(DeclarationAnalysisRecovery::from_parts(
            graph,
            types,
            source_ownership,
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
}
