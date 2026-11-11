#![allow(clippy::disallowed_types)]

//! Syntax-independent semantic identities and structural types.
//!
//! This crate deliberately has no source or syntax dependency. Source lowering may create these
//! values, but later compiler stages consume them without gaining a reverse path to syntax trees,
//! source ranges, or rendered type spellings.

mod arena;
mod array_range;
mod attachment_family;
mod closure_sequence;
mod constant;
mod frozen_data;
mod sparse_arena;
pub use frozen_data::{FrozenData, FrozenNode, InvalidFrozenData};
mod construction_identity;
mod field_path;
mod id;
mod numeric;
mod origin;
mod package;
mod symbol;
mod target;
mod type_projection;
mod type_store;
mod type_transaction;
mod value_access;

pub use arena::{Arena, ArenaBuilder, ArenaCheckpoint};
pub use array_range::ArrayRange;
pub use attachment_family::AttachmentFamily;
pub use closure_sequence::{ClosureSequence, ClosureSequenceIter, UnknownClosureSequenceId};
pub use constant::{ConstantValue, FrozenValue, GenericApplication, GenericValue, UsizeTerm};
pub use field_path::{
    FieldPath, IndexBoundsCheck, IndexRelation, PathRelation, ValueField, ValuePath,
    ValueProjection,
};
pub use id::{
    AccessBlockId, AccessCodeId, AccessExpressionId, AccessFieldId, AccessFlowPointId,
    AccessInstanceId, AccessLocalId, AccessLoopId, AccessParameterId, AccessPlaceId,
    AccessStatementId, AssociatedTypeId, BodyId, BodyNodeId, BodyScopeId, CallableId,
    CapabilityEvidenceId, CaptureId, ClosureId, ConstantExpressionId, ConstantId, ConstructionId,
    DeclarationSiteId, DropId, ExecutableItemId, ExecutableStaticId, FieldId, GenericParameterId,
    ImportId, InstanceId, InterfaceId, InterfaceImplementationId, LocalBindingId, LoopId,
    MirBlockId, MirDropFlagId, MirLocalId, MirOperationId, MirPlaceId, MirValueId, ModuleId,
    NominalTypeId, OpaqueTypeId, PackageId, PackageTargetId, ParameterId, PlaceId, RequirementId,
    StaticId, TestId, TypeAliasId, TypeId, VariantId,
};
pub use numeric::{BuiltinNumericType, lossless_builtin_numeric_conversion};
pub use origin::{
    DuplicateOrigin, InputProvenance, InputProvenanceConstraint, InvalidInputProvenance,
    ParameterOrigin, ProvenanceSet, provenance_is_bounded_by,
};
pub use package::PackageIdentity;
pub use sparse_arena::{SparseArena, SparseArenaBuilder, SparseInsertionError};
pub use symbol::{Symbol, SymbolTable};
pub use target::{CompilationTarget, PackageTargetKind};
pub use type_projection::{TypeProjection, TypeProjectionError};
pub use type_store::{
    AllocationGuarantee, ArgumentPack, ArgumentPackType, BorrowCapability, BuiltinType,
    CallableCapability, CallableContract, CallableGuarantees, CallableRepresentation, CallableType,
    CompileTimeGuarantee, InvalidParameterOrigin, NonblockingGuarantee, TrapGuarantee,
    TupleElements, TypeCursor, TypeKind, TypeStore, UnknownTypeId,
};
pub use type_transaction::{StaleTypeTransaction, TypeAuthority, TypeTransaction};
pub use value_access::{
    ValueAccess, ValueAccessMode, ValueApplication, ValueBindingKind, ValueClosure, ValueCode,
    ValueCodeRoot, ValueExpression, ValueFunctionOrigin, ValueInputOperation, ValueInvocation,
    ValueStatic,
};
