use std::fmt;

use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceBinding, SourceRole};

use self::evidence::{SemanticFact, SemanticQueryContext};
use self::source_selection::{select_source_binding, select_source_candidates};
use crate::AnalysisSnapshot;

mod callable_source;
mod code_actions;
mod completion;
mod evidence;
mod highlights;
mod inlay_hints;
mod navigation;
mod presentation;
mod rename;
mod session;
mod signature;
mod source_context;
mod source_selection;

pub use code_actions::{
    InterfaceImplementationActionError, OutcomeActionError, SemanticCodeAction,
    SemanticCodeActionError,
};
pub use completion::{
    SemanticCompletion, SemanticCompletionEdit, SemanticCompletionError, SemanticCompletionKind,
};
pub use evidence::{
    EvidenceIntegrityError, SemanticBodyGap, SemanticCoverage, SemanticQuerySet,
    SemanticSetUnavailability, TypedBodyUnavailability,
};
pub use highlights::{SemanticHighlight, SemanticHighlightKind};
pub use inlay_hints::{SemanticInlayHint, SemanticInlayHintError, SemanticInlayHintKind};
pub use navigation::SemanticLocation;
pub use presentation::{PresentationError, SemanticPresentation};
pub use rename::{SemanticRenameEdit, SemanticRenameError, SemanticRenamePlan};
pub(crate) use session::AnalysisQuerySession;
pub use signature::{SemanticParameterLabel, SemanticSignatureError, SemanticSignatureHelp};
pub use source_context::SourceContextError;

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
    /// Seals a checked semantic generation for editor mutation.
    ///
    /// Target construction may still have failed for an independent toolchain or ABI reason, but
    /// all semantic identities, source ownership, syntax origins, and projection issues must be
    /// valid before this capability is issued.
    ///
    /// # Errors
    ///
    /// Returns the exact current-generation evidence inconsistency instead of treating a corrupt
    /// editor projection as an ordinary unavailable feature.
    pub(crate) fn seals_semantic_mutation(&self) -> Result<bool, crate::EvidenceIntegrityError> {
        let Some(query) = self.semantic_query()? else {
            return Ok(false);
        };
        Ok(query.complete().is_some())
    }

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
        let Some(query) = self.semantic_query()? else {
            return Ok(None);
        };
        Ok(semantic_selection_from(
            query.source_index(),
            source,
            offset,
        ))
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
        let Some(authority) = self.semantic_query()? else {
            return Ok(None);
        };
        let index = authority.source_index();
        let module = authority.module_for_source(source)?;
        let spellings = self
            .queries
            .source_spellings(authority.graph(), module, index, source);
        let Some(binding) = selected_binding(index, source, offset) else {
            let mut literals = Vec::new();
            for binding in index.bindings_at(source, offset) {
                let SemanticEntity::BodyNode(body, node) = binding.entity() else {
                    continue;
                };
                if let SemanticFact::Available(Some(ty)) =
                    authority.scalar_literal_type(body, node)?
                {
                    literals.push((*binding, (*binding, ty)));
                }
            }
            let Some((binding, ty)) = select_source_candidates(literals.into_iter()).unique()
            else {
                return Ok(None);
            };
            let Some(presentation) = presentation::type_presentation_with_spellings(
                authority.graph(),
                authority.types(),
                ty,
                &spellings,
            ) else {
                return Ok(None);
            };
            return Ok(Some(SemanticSubject {
                entity: binding.entity(),
                role: binding.role(),
                range: binding.origin().span().range(),
                presentation,
                documentation: None,
            }));
        };
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

    fn semantic_query(
        &self,
    ) -> Result<Option<SemanticQueryContext<'_>>, crate::EvidenceIntegrityError> {
        let Some(query) = self.unvalidated_semantic_query() else {
            return Ok(None);
        };
        self.queries.validate_semantics(|| {
            query.validate_generation(self.sources(), self.syntax_trees())
        })?;
        Ok(Some(query))
    }

    fn unvalidated_semantic_query(&self) -> Option<SemanticQueryContext<'_>> {
        Some(SemanticQueryContext::new(self.semantic_evidence()?))
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

pub(in crate::query) fn semantic_selection_from(
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
    match entity {
        SemanticEntity::Module(_)
        | SemanticEntity::BuiltinType(_)
        | SemanticEntity::NominalType(_)
        | SemanticEntity::TypeAlias(_)
        | SemanticEntity::Interface(_)
        | SemanticEntity::AssociatedType(_)
        | SemanticEntity::Callable(_)
        | SemanticEntity::Constant(_)
        | SemanticEntity::Static(_)
        | SemanticEntity::Field(_)
        | SemanticEntity::Variant(_)
        | SemanticEntity::GenericParameter(_)
        | SemanticEntity::Parameter(_)
        | SemanticEntity::Test(_)
        | SemanticEntity::PlaceProjection(..)
        | SemanticEntity::LocalBinding(..)
        | SemanticEntity::Capture(..) => true,
        SemanticEntity::Package(_)
        | SemanticEntity::PackageTarget(_)
        | SemanticEntity::Import(_)
        | SemanticEntity::DeclarationSite(_)
        | SemanticEntity::Construction(_)
        | SemanticEntity::Instance(_)
        | SemanticEntity::InterfaceImplementation(_)
        | SemanticEntity::Drop(_)
        | SemanticEntity::Requirement(_)
        | SemanticEntity::CapabilityEvidence(_)
        | SemanticEntity::Body(_)
        | SemanticEntity::BodyScope(..)
        | SemanticEntity::BodyNode(..)
        | SemanticEntity::OpaqueType(_) => false,
    }
}
