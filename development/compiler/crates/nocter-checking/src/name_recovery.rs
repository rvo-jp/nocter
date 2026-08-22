use nocter_declarations::DeclarationGraph;
use nocter_model::{Arena, BodyId, TypeStore};
use nocter_source_index::SourceIndex;

use crate::ResolvedBodyNames;

/// Current-generation declaration and partial lexical-scope state retained after a name rule.
///
/// The failing spelling has no invented target. Only scopes, bindings, and projections completed
/// before the resolver stopped are present, so this value cannot be used as checking input.
#[derive(Debug)]
pub struct NameAnalysisRecovery {
    graph: DeclarationGraph,
    types: TypeStore,
    body_names: Arena<BodyId, ResolvedBodyNames>,
    source_index: SourceIndex,
}

impl NameAnalysisRecovery {
    pub(crate) const fn new(
        graph: DeclarationGraph,
        types: TypeStore,
        body_names: Arena<BodyId, ResolvedBodyNames>,
        source_index: SourceIndex,
    ) -> Self {
        Self {
            graph,
            types,
            body_names,
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
    pub const fn body_names(&self) -> &Arena<BodyId, ResolvedBodyNames> {
        &self.body_names
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }
}
