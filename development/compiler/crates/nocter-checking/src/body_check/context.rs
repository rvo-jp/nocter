use nocter_declarations::{BodyOwner, DeclarationGraph};
use nocter_frontend_bindings::SourceNamespaceTable;
use nocter_model::{BuiltinType, GenericParameterId, TypeId, TypeKind, TypeStore};
use nocter_source_index::SourceIndex;

use super::error::BodyCheckInternalError;
use crate::{
    BodySource, ConformanceTable, ConstructionSurfaceTable, DropTable, InstanceOperationTable,
    StandardSemanticTable,
};

/// Immutable program-wide authorities shared by every body checker.
#[derive(Clone, Copy)]
pub(super) struct BodyProgramFacts<'program> {
    graph: &'program DeclarationGraph,
    drops: &'program DropTable,
    conformances: &'program ConformanceTable,
    construction_surfaces: &'program ConstructionSurfaceTable,
    instance_operations: &'program InstanceOperationTable,
    standard_semantics: &'program StandardSemanticTable,
    source_namespaces: &'program SourceNamespaceTable,
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

pub(super) fn body_generic_domain(
    graph: &DeclarationGraph,
    source: BodySource<'_>,
) -> Result<Box<[GenericParameterId]>, BodyCheckInternalError> {
    graph
        .declarations()
        .body_generic_domain(source.body())
        .ok_or(BodyCheckInternalError::BodyIdentityMismatch(source.body()))
}

impl<'program> BodyProgramFacts<'program> {
    pub(super) const fn from_prepared(
        prepared: &'program crate::preparation::PreparedCheckingParts<'_>,
    ) -> Self {
        Self {
            graph: &prepared.graph,
            drops: &prepared.drops,
            conformances: &prepared.conformances,
            construction_surfaces: &prepared.construction_surfaces,
            instance_operations: &prepared.instance_operations,
            standard_semantics: &prepared.standard_semantics,
            source_namespaces: &prepared.source_namespaces,
            source_index: &prepared.source_index,
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

    pub(super) const fn construction_surfaces(self) -> &'program ConstructionSurfaceTable {
        self.construction_surfaces
    }

    pub(super) const fn instance_operations(self) -> &'program InstanceOperationTable {
        self.instance_operations
    }

    pub(super) const fn standard_semantics(self) -> &'program StandardSemanticTable {
        self.standard_semantics
    }

    pub(super) const fn source_namespaces(self) -> &'program SourceNamespaceTable {
        self.source_namespaces
    }

    pub(super) const fn source_index(self) -> &'program SourceIndex {
        self.source_index
    }
}
