use std::fmt;

use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceBinding, SourceRole};

use crate::AnalysisSnapshot;
use crate::presentation::{PresentationError, SemanticPresentation, hover_presentation};
use crate::source_context::{SourceContext, SourceContextError};
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
        self.target()?;
        selected_binding(self.source_index()?, source, offset).map(|binding| SemanticSelection {
            entity: binding.entity(),
            role: binding.role(),
            range: binding.origin().span().range(),
        })
    }

    /// Resolves one exact source position through the current successful semantic snapshot.
    ///
    /// Failed generations deliberately answer no semantic query. When projections overlap, the
    /// narrowest displayable source range wins; ties prefer references, then declarations, then
    /// implementation sites. This keeps keywords and declaration bodies outside editor ranges.
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
        let Some(target) = self.target() else {
            return Ok(None);
        };
        let checked = target.program().checked();
        let Some(index) = self.source_index() else {
            return Ok(None);
        };
        let Some(binding) = selected_binding(index, source, offset) else {
            return Ok(None);
        };
        let context = SourceContext::resolve(index, source)?;
        let presentation =
            hover_presentation(checked, binding.entity(), context.module(), index, source)?;
        Ok(Some(SemanticSubject {
            entity: binding.entity(),
            role: binding.role(),
            range: binding.origin().span().range(),
            presentation,
            documentation: index.documentation_for(binding).map(Box::from),
        }))
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
    select_source_binding(index.bindings_at(source, offset), interactive_binding)
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
            | SemanticEntity::NominalType(_)
            | SemanticEntity::TypeAlias(_)
            | SemanticEntity::Interface(_)
            | SemanticEntity::AssociatedType(_)
            | SemanticEntity::Callable(_)
            | SemanticEntity::Field(_)
            | SemanticEntity::BuiltinField(_)
            | SemanticEntity::Variant(_)
            | SemanticEntity::GenericParameter(_)
            | SemanticEntity::Parameter(_)
            | SemanticEntity::Test(_)
            | SemanticEntity::LocalBinding(..)
            | SemanticEntity::Capture(..)
    )
}
