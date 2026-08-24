use std::fmt;

use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceBinding, SourceRole};

use crate::AnalysisSnapshot;
use crate::presentation::{
    PresentationError, SemanticPresentation, declaration_presentation, hover_presentation,
    name_recovery_presentation, prepared_presentation,
};
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
        let context = SourceContext::resolve(index, source)?;
        let Some(presentation) =
            authority.presentation(binding.entity(), context.module(), index, source)?
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
        if let Some(target) = self.target() {
            return Some(SemanticAuthority::Complete {
                checked: target.program().checked(),
                source_index: target.source_index(),
            });
        }
        if let Some(recovery) = self.body_recovery() {
            return Some(SemanticAuthority::Bodies(recovery.prepared()));
        }
        if let Some(recovery) = self.name_recovery() {
            return Some(SemanticAuthority::Names(recovery));
        }
        self.declaration_recovery()
            .map(SemanticAuthority::Declarations)
    }
}

pub(crate) enum SemanticAuthority<'a> {
    Complete {
        checked: &'a nocter_checking::CheckedProgram,
        source_index: &'a nocter_source_index::SourceIndex,
    },
    Bodies(&'a nocter_checking::PreparedSemanticProgram),
    Names(&'a nocter_checking::NameAnalysisRecovery),
    Declarations(&'a nocter_checking::DeclarationAnalysisRecovery),
}

impl<'a> SemanticAuthority<'a> {
    pub(crate) fn source_index(&self) -> &'a nocter_source_index::SourceIndex {
        match self {
            Self::Complete { source_index, .. } => source_index,
            Self::Bodies(prepared) => prepared.source_index(),
            Self::Names(recovery) => recovery.source_index(),
            Self::Declarations(recovery) => recovery.source_index(),
        }
    }

    pub(crate) fn graph(&self) -> &'a nocter_declarations::DeclarationGraph {
        match self {
            Self::Complete { checked, .. } => checked.graph(),
            Self::Bodies(prepared) => prepared.graph(),
            Self::Names(recovery) => recovery.graph(),
            Self::Declarations(recovery) => recovery.graph(),
        }
    }

    pub(crate) fn checked(&self) -> Option<&'a nocter_checking::CheckedProgram> {
        match self {
            Self::Complete { checked, .. } => Some(checked),
            Self::Bodies(_) | Self::Names(_) | Self::Declarations(_) => None,
        }
    }

    fn presentation(
        &self,
        entity: SemanticEntity,
        from: nocter_model::ModuleId,
        source_index: &nocter_source_index::SourceIndex,
        source: SourceId,
    ) -> Result<Option<SemanticPresentation>, PresentationError> {
        match self {
            Self::Complete { checked, .. } => {
                hover_presentation(checked, entity, from, source_index, source).map(Some)
            }
            Self::Bodies(prepared) => {
                let spellings = crate::presentation::visible_spelling::VisibleSpellings::for_source(
                    prepared.graph(),
                    from,
                    source_index,
                    source,
                );
                Ok(prepared_presentation(prepared, entity, &spellings))
            }
            Self::Names(recovery) => {
                let spellings = crate::presentation::visible_spelling::VisibleSpellings::for_source(
                    recovery.graph(),
                    from,
                    source_index,
                    source,
                );
                Ok(name_recovery_presentation(recovery, entity, &spellings))
            }
            Self::Declarations(recovery) => {
                let spellings = crate::presentation::visible_spelling::VisibleSpellings::for_source(
                    recovery.graph(),
                    from,
                    source_index,
                    source,
                );
                Ok(declaration_presentation(recovery, entity, &spellings))
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
