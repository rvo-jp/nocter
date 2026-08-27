use std::collections::{BTreeMap, HashMap};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VisibleNameIssue {
    DuplicateSource(SourceId),
    ConflictingName {
        source: SourceId,
        name: Symbol,
        existing: SemanticEntity,
        duplicate: SemanticEntity,
    },
}

impl SourceVisibleNamesBuilder {
    pub(crate) fn define(
        &mut self,
        source: SourceId,
        names: impl IntoIterator<Item = (Symbol, SemanticEntity)>,
    ) -> Vec<VisibleNameIssue> {
        if self.by_source.contains_key(&source) {
            return vec![VisibleNameIssue::DuplicateSource(source)];
        }
        let mut selected = BTreeMap::new();
        let mut issues = Vec::new();
        for (name, entity) in names {
            match selected.entry(name) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(entity);
                }
                std::collections::btree_map::Entry::Occupied(entry) if *entry.get() != entity => {
                    issues.push(VisibleNameIssue::ConflictingName {
                        source,
                        name,
                        existing: *entry.get(),
                        duplicate: entity,
                    });
                }
                std::collections::btree_map::Entry::Occupied(_) => {}
            }
        }
        self.by_source.insert(
            source,
            selected
                .into_iter()
                .map(|(name, entity)| SourceVisibleName::new(name, entity))
                .collect(),
        );
        issues
    }

    pub(crate) fn finish(self) -> SourceVisibleNames {
        SourceVisibleNames {
            by_source: self
                .by_source
                .into_iter()
                .map(|(source, names)| (source, names.into_boxed_slice()))
                .collect(),
        }
    }
}
