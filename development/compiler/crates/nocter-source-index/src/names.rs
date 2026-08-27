use std::collections::HashMap;

use nocter_model::Symbol;
use nocter_source::SourceId;

use crate::SemanticEntity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceVisibleName {
    name: Symbol,
    entity: SemanticEntity,
}

impl SourceVisibleName {
    pub(crate) const fn new(name: Symbol, entity: SemanticEntity) -> Self {
        Self { name, entity }
    }

    pub(crate) const fn parts(self) -> (Symbol, SemanticEntity) {
        (self.name, self.entity)
    }
}

/// Editor-only projection of the effective names visible in each physical source.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SourceVisibleNames {
    by_source: HashMap<SourceId, Box<[SourceVisibleName]>>,
}

impl SourceVisibleNames {
    pub(crate) fn in_source(&self, source: SourceId) -> &[SourceVisibleName] {
        self.by_source.get(&source).map_or(&[], AsRef::as_ref)
    }

    pub(crate) fn entities(&self) -> impl Iterator<Item = SemanticEntity> + '_ {
        self.by_source.values().flatten().map(|name| name.parts().1)
    }

    pub(crate) fn sources(&self) -> impl Iterator<Item = SourceId> + '_ {
        self.by_source.keys().copied()
    }

    pub(crate) fn into_builder(self) -> SourceVisibleNamesBuilder {
        SourceVisibleNamesBuilder {
            by_source: self
                .by_source
                .into_iter()
                .map(|(source, names)| (source, names.into_vec()))
                .collect(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct SourceVisibleNamesBuilder {
    by_source: HashMap<SourceId, Vec<SourceVisibleName>>,
}

impl SourceVisibleNamesBuilder {
    pub(crate) fn define(
        &mut self,
        source: SourceId,
        names: impl IntoIterator<Item = (Symbol, SemanticEntity)>,
    ) {
        self.by_source.insert(
            source,
            names
                .into_iter()
                .map(|(name, entity)| SourceVisibleName::new(name, entity))
                .collect(),
        );
    }

    pub(crate) fn finish(self) -> SourceVisibleNames {
        SourceVisibleNames {
            by_source: self
                .by_source
                .into_iter()
                .map(|(source, mut names)| {
                    names.sort_unstable_by_key(|name| name.parts());
                    names.dedup_by_key(|name| name.parts().0);
                    (source, names.into_boxed_slice())
                })
                .collect(),
        }
    }
}
