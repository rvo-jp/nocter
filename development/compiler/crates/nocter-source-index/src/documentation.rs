use std::collections::HashMap;

use crate::{SemanticEntity, SourceOrigin};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EntityDocumentation {
    entity: SemanticEntity,
    markdown: Box<str>,
}

impl EntityDocumentation {
    #[must_use]
    pub(crate) const fn entity(&self) -> SemanticEntity {
        self.entity
    }

    #[must_use]
    pub(crate) fn markdown(&self) -> &str {
        &self.markdown
    }

    pub(crate) fn into_parts(self) -> (SemanticEntity, Box<str>) {
        (self.entity, self.markdown)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OccurrenceDocumentation {
    entity: SemanticEntity,
    origin: SourceOrigin,
    markdown: Box<str>,
}

impl OccurrenceDocumentation {
    #[must_use]
    pub(crate) const fn entity(&self) -> SemanticEntity {
        self.entity
    }

    #[must_use]
    pub(crate) const fn origin(&self) -> SourceOrigin {
        self.origin
    }

    #[must_use]
    pub(crate) fn markdown(&self) -> &str {
        &self.markdown
    }

    pub(crate) fn into_parts(self) -> ((SemanticEntity, SourceOrigin), Box<str>) {
        ((self.entity, self.origin), self.markdown)
    }
}

pub(crate) fn finish_entities(
    documentation: HashMap<SemanticEntity, Box<str>>,
) -> Box<[EntityDocumentation]> {
    let mut entries = documentation
        .into_iter()
        .map(|(entity, markdown)| EntityDocumentation { entity, markdown })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(EntityDocumentation::entity);
    entries.into_boxed_slice()
}

pub(crate) fn finish_occurrences(
    documentation: HashMap<(SemanticEntity, SourceOrigin), Box<str>>,
) -> Box<[OccurrenceDocumentation]> {
    let mut entries = documentation
        .into_iter()
        .map(|((entity, origin), markdown)| OccurrenceDocumentation {
            entity,
            origin,
            markdown,
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| occurrence_sort_key(entry.entity(), entry.origin()));
    entries.into_boxed_slice()
}

pub(crate) fn occurrence_sort_key(
    entity: SemanticEntity,
    origin: SourceOrigin,
) -> (
    SemanticEntity,
    nocter_source::SourceId,
    nocter_source::ByteOffset,
    nocter_source::ByteOffset,
    u8,
    usize,
) {
    (
        entity,
        origin.source(),
        origin.span().range().start(),
        origin.span().range().end(),
        origin.syntax().sort_key().0,
        origin.syntax().sort_key().1,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentationOwner {
    Entity(SemanticEntity),
    Occurrence {
        entity: SemanticEntity,
        origin: SourceOrigin,
    },
}
