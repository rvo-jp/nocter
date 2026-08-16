use nocter_declarations::{BodyOwner, DeclarationGraph};
use nocter_model::{BuiltinType, TypeId, TypeKind, TypeStore};
use nocter_source_index::SourceIndex;

use super::error::BodyCheckInternalError;
use crate::{BodySource, ConformanceTable, DropTable, InstanceOperationTable};

/// Immutable program-wide authorities shared by every body checker.
#[derive(Clone, Copy)]
pub(super) struct BodyProgramFacts<'program> {
    graph: &'program DeclarationGraph,
    drops: &'program DropTable,
    conformances: &'program ConformanceTable,
    instance_operations: &'program InstanceOperationTable,
    source_index: &'program SourceIndex,
}

pub(super) fn body_result_type(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source: BodySource<'_>,
) -> Result<TypeId, BodyCheckInternalError> {
    match source.owner() {
        BodyOwner::Callable(callable) => graph
            .declarations()
            .callables()
            .get(callable)
            .map(nocter_declarations::CallableDeclaration::result)
            .ok_or(BodyCheckInternalError::BodyIdentityMismatch(source.body())),
        BodyOwner::Drop(_) => Ok(types.builtin(BuiltinType::Void)),
        BodyOwner::Test(_) => types
            .intern(TypeKind::Fallible(types.builtin(BuiltinType::Void)))
            .map_err(|_| BodyCheckInternalError::UnknownType(types.builtin(BuiltinType::Void))),
    }
}

impl<'program> BodyProgramFacts<'program> {
    pub(super) const fn new(
        graph: &'program DeclarationGraph,
        drops: &'program DropTable,
        conformances: &'program ConformanceTable,
        instance_operations: &'program InstanceOperationTable,
        source_index: &'program SourceIndex,
    ) -> Self {
        Self {
            graph,
            drops,
            conformances,
            instance_operations,
            source_index,
        }
    }

    pub(super) const fn graph(self) -> &'program DeclarationGraph {
        self.graph
    }

    pub(super) const fn drops(self) -> &'program DropTable {
        self.drops
    }

    pub(super) const fn conformances(self) -> &'program ConformanceTable {
        self.conformances
    }

    pub(super) const fn instance_operations(self) -> &'program InstanceOperationTable {
        self.instance_operations
    }

    pub(super) const fn source_index(self) -> &'program SourceIndex {
        self.source_index
    }
}
