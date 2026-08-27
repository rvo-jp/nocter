use std::fmt;

use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceBinding, SourceRole};

use crate::AnalysisSnapshot;
use crate::presentation::{
    PresentationError, SemanticPresentation, body_recovery_presentation, declaration_presentation,
    hover_presentation, name_recovery_presentation,
};
use crate::source_context::SourceContextError;
use crate::source_selection::select_source_binding;

#[path = "evidence.rs"]
pub(crate) mod evidence;

/// One exact interactive source occurrence selected independently of presentation or protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticSelection {
    entity: SemanticEntity,
    role: SourceRole,
    range: TextRange,
}

impl SemanticSelection {
    #[must_use]
    pub const fn entity(self) -> SemanticEntity {
        self.entity
    }

    #[must_use]
    pub const fn role(self) -> SourceRole {
        self.role
    }

    #[must_use]
    pub const fn range(self) -> TextRange {
        self.range
    }
}

/// Protocol-independent source selection for one resolved semantic identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSubject {
    entity: SemanticEntity,
    role: SourceRole,
    range: TextRange,
    presentation: SemanticPresentation,
    documentation: Option<Box<str>>,
}

impl SemanticSubject {
    #[must_use]
    pub const fn entity(&self) -> SemanticEntity {
        self.entity
    }

    #[must_use]
    pub const fn role(&self) -> SourceRole {
        self.role
    }

    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    #[must_use]
    pub const fn presentation(&self) -> &SemanticPresentation {
        &self.presentation
    }

    #[must_use]
    pub fn documentation(&self) -> Option<&str> {
        self.documentation.as_deref()
    }
}

impl AnalysisSnapshot {
    /// Resolves one exact interactive semantic occurrence without rendering it.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when the selected source binding has no semantic domain in the
    /// current evidence result.
    pub fn semantic_selection(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Result<Option<SemanticSelection>, crate::EvidenceIntegrityError> {
        let Some(query) = self.semantic_query() else {
            return Ok(None);
        };
        let selection = semantic_selection_from(query.source_index(), source, offset);
        if let Some(selection) = selection {
            query.validate_interactive_entity(selection.entity())?;
        }
        Ok(selection)
    }

    /// Resolves one exact source position through the deepest current semantic authority.
    ///
    /// Failed generations use only the immutable recovery stage retained by the production
    /// pipeline; this query never reruns lowering or invents missing bindings. When projections
    /// overlap, the narrowest displayable source range wins; ties prefer references, then
    /// declarations, then implementation sites. This keeps keywords and declaration bodies
    /// outside editor ranges.
    ///
    /// # Errors
    ///
    /// Returns an internal query error when source ownership or checked presentation is
    /// inconsistent.
    pub fn semantic_subject(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Result<Option<SemanticSubject>, SemanticQueryError> {
        let Some(authority) = self.semantic_query() else {
            return Ok(None);
        };
        let index = authority.source_index();
        let Some(binding) = selected_binding(index, source, offset) else {
            return Ok(None);
        };
        authority.validate_interactive_entity(binding.entity())?;
        let module = authority.source_ownership().module_for_source(source)?;
        let spellings = self
            .queries
            .source_spellings(authority.graph(), module, index, source);
        let Some(presentation) = authority.presentation(binding.entity(), &spellings, source)?
        else {
            return Ok(None);
        };
        Ok(Some(SemanticSubject {
            entity: binding.entity(),
            role: binding.role(),
            range: binding.origin().span().range(),
            presentation,
            documentation: index.documentation_for(binding).map(Box::from),
        }))
    }

    pub(crate) fn semantic_query(&self) -> Option<SemanticQueryContext<'_>> {
        if let crate::AnalysisState::Current(crate::CurrentAnalysis {
            semantic_evidence: crate::CurrentSemanticEvidence::Target(target),
            ..
        }) = &self.state
        {
            return Some(SemanticQueryContext {
                evidence: SemanticEvidence::Checked {
                    checked: target.program().checked(),
                    source_index: target.source_index(),
                },
            });
        }
        match self.retained_semantic()? {
            nocter_session::SemanticAnalysis::Checked(checked) => Some(SemanticQueryContext {
                evidence: SemanticEvidence::Checked {
                    checked: checked.program(),
                    source_index: checked.source_index(),
                },
            }),
            nocter_session::SemanticAnalysis::Bodies(analysis) => Some(SemanticQueryContext {
                evidence: SemanticEvidence::Bodies(analysis),
            }),
            nocter_session::SemanticAnalysis::Names(analysis) => Some(SemanticQueryContext {
                evidence: SemanticEvidence::Names(analysis),
            }),
            nocter_session::SemanticAnalysis::Declarations(analysis) => {
                Some(SemanticQueryContext {
                    evidence: SemanticEvidence::Declarations(analysis),
                })
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SemanticQueryContext<'a> {
    evidence: SemanticEvidence<'a>,
}

#[derive(Clone, Copy)]
enum SemanticEvidence<'a> {
    Checked {
        checked: &'a nocter_checking::CheckedProgram,
        source_index: &'a nocter_source_index::SourceIndex,
    },
    Bodies(&'a nocter_checking::BodyAnalysisRecovery),
    Names(&'a nocter_checking::NameAnalysisRecovery),
    Declarations(&'a nocter_checking::DeclarationAnalysisRecovery),
}

impl<'a> SemanticQueryContext<'a> {
    pub(crate) fn source_ownership(&self) -> &'a nocter_checking::SourceOwnershipTable {
        match self.evidence {
            SemanticEvidence::Checked { checked, .. } => checked.source_ownership(),
            SemanticEvidence::Bodies(analysis) => analysis.prepared().source_ownership(),
            SemanticEvidence::Names(recovery) => recovery.source_ownership(),
            SemanticEvidence::Declarations(recovery) => recovery.source_ownership(),
        }
    }

    pub(crate) fn source_index(&self) -> &'a nocter_source_index::SourceIndex {
        match self.evidence {
            SemanticEvidence::Checked { source_index, .. } => source_index,
            SemanticEvidence::Bodies(analysis) => analysis.source_index(),
            SemanticEvidence::Names(recovery) => recovery.source_index(),
            SemanticEvidence::Declarations(recovery) => recovery.source_index(),
        }
    }

    pub(crate) fn graph(&self) -> &'a nocter_declarations::DeclarationGraph {
        match self.evidence {
            SemanticEvidence::Checked { checked, .. } => checked.graph(),
            SemanticEvidence::Bodies(analysis) => analysis.prepared().graph(),
            SemanticEvidence::Names(recovery) => recovery.graph(),
            SemanticEvidence::Declarations(recovery) => recovery.graph(),
        }
    }

    pub(crate) fn types(&self) -> &'a nocter_model::TypeStore {
        match self.evidence {
            SemanticEvidence::Checked { checked, .. } => checked.types(),
            SemanticEvidence::Bodies(analysis) => analysis.prepared().types(),
            SemanticEvidence::Names(recovery) => recovery.types(),
            SemanticEvidence::Declarations(recovery) => recovery.types(),
        }
    }

    pub(crate) const fn body_recovery(&self) -> Option<&'a nocter_checking::BodyAnalysisRecovery> {
        match self.evidence {
            SemanticEvidence::Bodies(analysis) => Some(analysis),
            SemanticEvidence::Checked { .. }
            | SemanticEvidence::Names(_)
            | SemanticEvidence::Declarations(_) => None,
        }
    }

    pub(crate) const fn declaration_recovery(
        &self,
    ) -> Option<&'a nocter_checking::DeclarationAnalysisRecovery> {
        match self.evidence {
            SemanticEvidence::Declarations(analysis) => Some(analysis),
            SemanticEvidence::Checked { .. }
            | SemanticEvidence::Bodies(_)
            | SemanticEvidence::Names(_) => None,
        }
    }

    pub(crate) fn completion_detail(
        &self,
        entity: SemanticEntity,
        spellings: &crate::presentation::visible_spelling::VisibleSpellings,
    ) -> Result<Option<Box<str>>, PresentationError> {
        let presentation = match self.evidence {
            SemanticEvidence::Checked { checked, .. } => Ok(crate::presentation::presentation(
                checked, entity, spellings,
            )),
            SemanticEvidence::Bodies(analysis) => {
                body_recovery_presentation(analysis, entity, spellings)
            }
            SemanticEvidence::Names(analysis) => {
                Ok(name_recovery_presentation(analysis, entity, spellings))
            }
            SemanticEvidence::Declarations(analysis) => {
                Ok(declaration_presentation(analysis, entity, spellings))
            }
        }?;
        Ok(presentation.map(|presentation| Box::<str>::from(presentation.code())))
    }

    fn presentation(
        &self,
        entity: SemanticEntity,
        spellings: &crate::presentation::visible_spelling::VisibleSpellings,
        source: SourceId,
    ) -> Result<Option<SemanticPresentation>, PresentationError> {
        match self.evidence {
            SemanticEvidence::Checked { checked, .. } => {
                hover_presentation(checked, entity, spellings, source).map(Some)
            }
            SemanticEvidence::Bodies(analysis) => {
                body_recovery_presentation(analysis, entity, spellings)
            }
            SemanticEvidence::Names(recovery) => {
                Ok(name_recovery_presentation(recovery, entity, spellings))
            }
            SemanticEvidence::Declarations(recovery) => {
                Ok(declaration_presentation(recovery, entity, spellings))
            }
        }
    }
}

/// An internal failure while answering a semantic presentation query.
#[derive(Debug)]
pub enum SemanticQueryError {
    Evidence(crate::EvidenceIntegrityError),
    SourceContext(SourceContextError),
    Presentation(PresentationError),
}

impl fmt::Display for SemanticQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evidence(error) => error.fmt(formatter),
            Self::SourceContext(error) => error.fmt(formatter),
            Self::Presentation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Evidence(error) => Some(error),
            Self::SourceContext(error) => Some(error),
            Self::Presentation(error) => Some(error),
        }
    }
}

impl From<crate::EvidenceIntegrityError> for SemanticQueryError {
    fn from(error: crate::EvidenceIntegrityError) -> Self {
        Self::Evidence(error)
    }
}

impl From<SourceContextError> for SemanticQueryError {
    fn from(error: SourceContextError) -> Self {
        Self::SourceContext(error)
    }
}

impl From<PresentationError> for SemanticQueryError {
    fn from(error: PresentationError) -> Self {
        Self::Presentation(error)
    }
}

pub(crate) fn semantic_selection_from(
    index: &nocter_source_index::SourceIndex,
    source: SourceId,
    offset: ByteOffset,
) -> Option<SemanticSelection> {
    selected_binding(index, source, offset).map(|binding| SemanticSelection {
        entity: binding.entity(),
        role: binding.role(),
        range: binding.origin().span().range(),
    })
}

fn selected_binding(
    index: &nocter_source_index::SourceIndex,
    source: SourceId,
    offset: ByteOffset,
) -> Option<SourceBinding> {
    select_source_binding(index.bindings_at(source, offset), interactive_binding).unique()
}

fn interactive_binding(binding: &SourceBinding) -> bool {
    interactive_entity(binding.entity())
        && (!matches!(binding.entity(), SemanticEntity::Module(_))
            || binding.role() == SourceRole::Reference)
}

const fn interactive_entity(entity: SemanticEntity) -> bool {
    matches!(
        entity,
        SemanticEntity::Module(_)
            | SemanticEntity::BuiltinType(_)
            | SemanticEntity::NominalType(_)
            | SemanticEntity::TypeAlias(_)
            | SemanticEntity::Interface(_)
            | SemanticEntity::AssociatedType(_)
            | SemanticEntity::Callable(_)
            | SemanticEntity::Constant(_)
            | SemanticEntity::Field(_)
            | SemanticEntity::Variant(_)
            | SemanticEntity::GenericParameter(_)
            | SemanticEntity::Parameter(_)
            | SemanticEntity::Test(_)
            | SemanticEntity::LocalBinding(..)
            | SemanticEntity::Capture(..)
    )
}
