use nocter_declarations::DeclarationGraph;
use nocter_model::TypeStore;
use nocter_source_index::SourceIndex;

/// Source-projected declaration facts retained after an authored declaration rule rejects source.
///
/// The snapshot contains no accepted declaration program, frontend checking bindings, builder, or
/// production transition. Consumers may inspect only the completed facts owned by declaration
/// lowering.
#[derive(Debug)]
pub struct DeclarationLoweringRecovery {
    graph: DeclarationGraph,
    types: TypeStore,
    source_index: SourceIndex,
}

impl DeclarationLoweringRecovery {
    pub(crate) const fn new(
        graph: DeclarationGraph,
        types: TypeStore,
        source_index: SourceIndex,
    ) -> Self {
        Self {
            graph,
            types,
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
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(self) -> (DeclarationGraph, TypeStore, SourceIndex) {
        (self.graph, self.types, self.source_index)
    }
}
