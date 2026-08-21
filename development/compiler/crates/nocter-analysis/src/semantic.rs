use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceBinding, SourceRole};

use crate::AnalysisSnapshot;
use crate::presentation::{SemanticPresentation, presentation};

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
        self.source_index()?
            .bindings_at(source, offset)
            .filter(|binding| interactive_binding(binding))
            .min_by_key(|binding| selection_key(binding))
            .map(|binding| SemanticSelection {
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
    #[must_use]
    pub fn semantic_subject(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Option<SemanticSubject> {
        let checked = self.target()?.program().checked();
        let selection = self.semantic_selection(source, offset)?;
        let presentation = presentation(checked, selection.entity())?;
        Some(SemanticSubject {
            entity: selection.entity(),
            role: selection.role(),
            range: selection.range(),
            presentation,
        })
    }
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

fn selection_key(binding: &SourceBinding) -> (u32, u8, u8) {
    let range = binding.origin().span().range();
    (
        range.end().get() - range.start().get(),
        match binding.role() {
            SourceRole::Reference => 0,
            SourceRole::Declaration => 1,
            SourceRole::Implementation => 2,
        },
        entity_rank(binding.entity()),
    )
}

const fn entity_rank(entity: SemanticEntity) -> u8 {
    match entity {
        SemanticEntity::LocalBinding(..) | SemanticEntity::Capture(..) => 0,
        SemanticEntity::Parameter(_) | SemanticEntity::GenericParameter(_) => 1,
        SemanticEntity::Field(_) | SemanticEntity::BuiltinField(_) | SemanticEntity::Variant(_) => {
            2
        }
        SemanticEntity::Callable(_)
        | SemanticEntity::NominalType(_)
        | SemanticEntity::TypeAlias(_)
        | SemanticEntity::Interface(_)
        | SemanticEntity::AssociatedType(_) => 3,
        SemanticEntity::Module(_)
        | SemanticEntity::Package(_)
        | SemanticEntity::PackageTarget(_) => 4,
        SemanticEntity::Import(_)
        | SemanticEntity::DeclarationSite(_)
        | SemanticEntity::Construction(_)
        | SemanticEntity::Instance(_)
        | SemanticEntity::Conformance(_)
        | SemanticEntity::Drop(_)
        | SemanticEntity::Test(_)
        | SemanticEntity::Requirement(_)
        | SemanticEntity::Body(_)
        | SemanticEntity::BodyScope(..)
        | SemanticEntity::BodyNode(..)
        | SemanticEntity::OpaqueType(_) => 5,
    }
}
