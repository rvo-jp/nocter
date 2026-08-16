use nocter_declarations::DeclarationGraph;
use nocter_source_index::SourceIndex;

use crate::DropTable;

/// Immutable program-wide authorities shared by every body checker.
#[derive(Clone, Copy)]
pub(super) struct BodyProgramFacts<'program> {
    graph: &'program DeclarationGraph,
    drops: &'program DropTable,
    source_index: &'program SourceIndex,
}

impl<'program> BodyProgramFacts<'program> {
    pub(super) const fn new(
        graph: &'program DeclarationGraph,
        drops: &'program DropTable,
        source_index: &'program SourceIndex,
    ) -> Self {
        Self {
            graph,
            drops,
            source_index,
        }
    }

    pub(super) const fn graph(self) -> &'program DeclarationGraph {
        self.graph
    }

    pub(super) const fn drops(self) -> &'program DropTable {
        self.drops
    }

    pub(super) const fn source_index(self) -> &'program SourceIndex {
        self.source_index
    }
}
