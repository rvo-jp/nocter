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
    #[must_use]
    pub fn semantic_selection(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Option<SemanticSelection> {
        selected_binding(self.semantic_authority()?.source_index(), source, offset).map(|binding| {
            SemanticSelection {
                entity: binding.entity(),
                role: binding.role(),
                range: binding.origin().span().range(),
            }
        })
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
        let Some(authority) = self.semantic_authority() else {
            return Ok(None);
        };
        let index = authority.source_index();
        let Some(binding) = selected_binding(index, source, offset) else {
            return Ok(None);
        };
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

    pub(crate) fn semantic_authority(&self) -> Option<SemanticAuthority<'_>> {
        if let crate::AnalysisState::Current(crate::CurrentAnalysis {
            authority: crate::CurrentSemanticAuthority::Target(target),
            ..
        }) = &self.state
        {
            return Some(SemanticAuthority::Checked {
                checked: target.program().checked(),
                source_index: target.source_index(),
            });
        }
        match self.retained_semantic()? {
            nocter_session::SemanticAnalysis::Checked(checked) => {
                Some(SemanticAuthority::Checked {
                    checked: checked.program(),
                    source_index: checked.source_index(),
                })
            }
            nocter_session::SemanticAnalysis::Bodies(analysis) => {
                Some(SemanticAuthority::Bodies(analysis))
            }
            nocter_session::SemanticAnalysis::Names(analysis) => {
                Some(SemanticAuthority::Names(analysis))
            }
            nocter_session::SemanticAnalysis::Declarations(analysis) => {
                Some(SemanticAuthority::Declarations(analysis))
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum SemanticAuthority<'a> {
    Checked {
        checked: &'a nocter_checking::CheckedProgram,
        source_index: &'a nocter_source_index::SourceIndex,
    },
    Bodies(&'a nocter_checking::BodyAnalysisRecovery),
    Names(&'a nocter_checking::NameAnalysisRecovery),
    Declarations(&'a nocter_checking::DeclarationAnalysisRecovery),
}

impl<'a> SemanticAuthority<'a> {
    pub(crate) fn source_ownership(&self) -> &'a nocter_checking::SourceOwnershipTable {
        match self {
            Self::Checked { checked, .. } => checked.source_ownership(),
            Self::Bodies(analysis) => analysis.prepared().source_ownership(),
            Self::Names(recovery) => recovery.source_ownership(),
            Self::Declarations(recovery) => recovery.source_ownership(),
        }
    }

    pub(crate) fn source_index(&self) -> &'a nocter_source_index::SourceIndex {
        match self {
            Self::Checked { source_index, .. } => source_index,
            Self::Bodies(analysis) => analysis.source_index(),
            Self::Names(recovery) => recovery.source_index(),
            Self::Declarations(recovery) => recovery.source_index(),
        }
    }

    pub(crate) fn graph(&self) -> &'a nocter_declarations::DeclarationGraph {
        match self {
            Self::Checked { checked, .. } => checked.graph(),
            Self::Bodies(analysis) => analysis.prepared().graph(),
            Self::Names(recovery) => recovery.graph(),
            Self::Declarations(recovery) => recovery.graph(),
        }
    }

    pub(crate) fn types(&self) -> &'a nocter_model::TypeStore {
        match self {
            Self::Checked { checked, .. } => checked.types(),
            Self::Bodies(analysis) => analysis.prepared().types(),
            Self::Names(recovery) => recovery.types(),
            Self::Declarations(recovery) => recovery.types(),
        }
    }

    pub(crate) fn checked(&self) -> Option<&'a nocter_checking::CheckedProgram> {
        match self {
            Self::Checked { checked, .. } => Some(checked),
            Self::Bodies(_) | Self::Names(_) | Self::Declarations(_) => None,
        }
    }

    pub(crate) fn body(
        &self,
        body: nocter_model::BodyId,
    ) -> Option<&'a nocter_checking::CheckedBody> {
        match self {
            Self::Checked { checked, .. } => checked.bodies().get(body),
            Self::Bodies(analysis) => analysis.body(body),
            Self::Names(_) | Self::Declarations(_) => None,
        }
    }

    pub(crate) const fn body_analysis(&self) -> Option<&'a nocter_checking::BodyAnalysisRecovery> {
        match self {
            Self::Bodies(analysis) => Some(analysis),
            Self::Checked { .. } | Self::Names(_) | Self::Declarations(_) => None,
        }
    }

    pub(crate) const fn declaration_analysis(
        &self,
    ) -> Option<&'a nocter_checking::DeclarationAnalysisRecovery> {
        match self {
            Self::Declarations(analysis) => Some(analysis),
            Self::Checked { .. } | Self::Bodies(_) | Self::Names(_) => None,
        }
    }

    pub(crate) fn scope(
        &self,
        body: nocter_model::BodyId,
        scope: nocter_model::BodyScopeId,
    ) -> Option<&'a nocter_checking::BodyScope> {
        match self {
            Self::Checked { checked, .. } => checked.bodies().get(body)?.scopes().get(scope),
            Self::Bodies(analysis) => analysis.body_names().get(body)?.scopes().get(scope),
            Self::Names(analysis) => analysis.body_names().get(body)?.scopes().get(scope),
            Self::Declarations(_) => None,
        }
    }

    pub(crate) fn completion_detail(
        &self,
        entity: SemanticEntity,
        spellings: &crate::presentation::visible_spelling::VisibleSpellings,
    ) -> Option<Box<str>> {
        match self {
            Self::Checked { checked, .. } => {
                crate::presentation::presentation(checked, entity, spellings)
            }
            Self::Bodies(analysis) => body_recovery_presentation(analysis, entity, spellings),
            Self::Names(analysis) => name_recovery_presentation(analysis, entity, spellings),
            Self::Declarations(analysis) => declaration_presentation(analysis, entity, spellings),
        }
        .map(|presentation| Box::<str>::from(presentation.code()))
    }

    fn presentation(
        &self,
        entity: SemanticEntity,
        spellings: &crate::presentation::visible_spelling::VisibleSpellings,
        source: SourceId,
    ) -> Result<Option<SemanticPresentation>, PresentationError> {
        match self {
            Self::Checked { checked, .. } => {
                hover_presentation(checked, entity, spellings, source).map(Some)
            }
            Self::Bodies(analysis) => Ok(body_recovery_presentation(analysis, entity, spellings)),
            Self::Names(recovery) => Ok(name_recovery_presentation(recovery, entity, spellings)),
            Self::Declarations(recovery) => {
                Ok(declaration_presentation(recovery, entity, spellings))
            }
        }
    }
}

/// An internal failure while answering a semantic presentation query.
#[derive(Debug)]
pub enum SemanticQueryError {
    SourceContext(SourceContextError),
    Presentation(PresentationError),
}

impl fmt::Display for SemanticQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceContext(error) => error.fmt(formatter),
            Self::Presentation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceContext(error) => Some(error),
            Self::Presentation(error) => Some(error),
        }
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
