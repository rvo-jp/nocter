use std::fmt;

use nocter_model::{
    ExecutableItemId, MirBlockId, MirDropFlagId, MirLocalId, MirOperationId, MirPlaceId,
    MirValueId, TypeId,
};

#[derive(Debug, Eq, PartialEq)]
pub enum MirValidationError {
    UnknownType(TypeId),
    UnknownItem(ExecutableItemId),
    UnknownLocal(MirLocalId),
    UnknownDropFlag(MirDropFlagId),
    UnknownPlace(MirPlaceId),
    UnknownValue(MirValueId),
    UnknownOperation(MirOperationId),
    UnknownBlock(MirBlockId),
    NonStorableLocal(MirLocalId),
    NonStorablePlace(MirPlaceId),
    DuplicateParameter(MirLocalId),
    InvalidParameterKind {
        parameter: MirLocalId,
        position: usize,
    },
    OrphanParameter(MirLocalId),
    InvalidPackInput(ExecutableItemId),
    InvalidDestruction(TypeId),
    InvalidPlaceRoot {
        place: MirPlaceId,
    },
    InvalidProjection {
        place: MirPlaceId,
    },
    PlaceTypeMismatch {
        place: MirPlaceId,
    },
    DynamicDropFlag {
        flag: MirDropFlagId,
    },
    InvalidValueDefinition(MirValueId),
    DuplicateOperation(MirOperationId),
    OrphanOperation(MirOperationId),
    InvalidOperationResult(MirOperationId),
    OperationType(MirOperationId),
    EntryHasParameters,
    EdgeArity {
        block: MirBlockId,
        expected: usize,
        actual: usize,
    },
    EdgeType {
        block: MirBlockId,
        position: usize,
    },
    UnreachableBlock(MirBlockId),
    ValueDoesNotDominate {
        value: MirValueId,
        block: MirBlockId,
    },
    NonBooleanBranch(MirBlockId),
    DuplicateSwitchCase(MirBlockId),
    InvalidSwitchSubject(MirBlockId),
    InvalidReturn(MirBlockId),
    InvalidRootSignature,
    InvalidRootTerminator(MirBlockId),
    InvalidPackExit(MirBlockId),
    InvalidRegionFlow {
        block: MirBlockId,
        region: Option<MirLocalId>,
    },
}

impl fmt::Display for MirValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid MIR: {self:?}")
    }
}

impl std::error::Error for MirValidationError {}
