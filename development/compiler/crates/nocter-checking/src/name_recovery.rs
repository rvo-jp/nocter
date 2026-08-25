use nocter_declarations::DeclarationGraph;
use nocter_model::{Arena, BodyId, TypeStore};
use nocter_source_index::SourceIndex;

use crate::ResolvedBodyNames;

/// Sparse body-name authority retained after one or more independent body-name failures.
///
/// Every declared body owns one slot. A missing slot means that body did not establish a valid
/// lexical recovery contract; it does not hide successfully resolved bodies that happen to follow
/// it in declaration order.
#[derive(Debug)]
pub struct PartialBodyNames {
    bodies: Arena<BodyId, Option<ResolvedBodyNames>>,
}

impl PartialBodyNames {
    pub(crate) const fn new(bodies: Arena<BodyId, Option<ResolvedBodyNames>>) -> Self {
        Self { bodies }
    }

    #[must_use]
    pub fn get(&self, body: BodyId) -> Option<&ResolvedBodyNames> {
        self.bodies.get(body)?.as_ref()
    }

    pub fn iter(&self) -> impl Iterator<Item = (BodyId, &ResolvedBodyNames)> {
        self.bodies
            .iter()
            .filter_map(|(body, names)| names.as_ref().map(|names| (body, names)))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}

/// Current-generation declaration and body-local lexical state retained after name rules.
///
/// Failing spellings have no invented targets. Each body owns an independent sparse recovery slot,
/// so a failure cannot hide valid scopes from bodies visited later. The table remains unsuitable as
/// checking input because failed bodies are intentionally incomplete.
#[derive(Debug)]
pub struct NameAnalysisRecovery {
    graph: DeclarationGraph,
    types: TypeStore,
    body_names: PartialBodyNames,
    source_index: SourceIndex,
}

impl NameAnalysisRecovery {
    pub(crate) const fn new(
        graph: DeclarationGraph,
        types: TypeStore,
        body_names: Arena<BodyId, Option<ResolvedBodyNames>>,
        source_index: SourceIndex,
    ) -> Self {
        Self {
            graph,
            types,
            body_names: PartialBodyNames::new(body_names),
            source_index,
        }
    }

    #[must_use]
    pub const fn graph(&self) -> &DeclarationGraph {
        &self.graph
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        &self.types
    }

    #[must_use]
    pub const fn body_names(&self) -> &PartialBodyNames {
        &self.body_names
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }
}
