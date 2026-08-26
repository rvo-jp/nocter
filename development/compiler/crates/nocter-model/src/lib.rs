//! Syntax-independent semantic identities and structural types.
//!
//! This crate deliberately has no source or syntax dependency. Source lowering may create these
//! values, but later compiler stages consume them without gaining a reverse path to syntax trees,
//! source ranges, or rendered type spellings.

mod arena;
mod attachment_family;
mod constant;
mod construction_identity;
mod id;
mod origin;
mod package;
mod symbol;
mod target;
mod type_projection;
mod type_store;

pub use arena::{Arena, ArenaBuilder, ArenaCheckpoint};
pub use attachment_family::AttachmentFamily;
pub use constant::ConstantValue;
pub use id::{
    AssociatedTypeId, BodyId, BodyNodeId, BodyScopeId, CallableId, CaptureId, ClosureId,
    ConstantId, ConstructionId, DeclarationSiteId, DropId, ExecutableItemId, FieldId,
    GenericParameterId, ImportId, InstanceId, InterfaceId, InterfaceImplementationId,
    LocalBindingId, LoopId, MirBlockId, MirDropFlagId, MirLocalId, MirOperationId, MirPlaceId,
    MirValueId, ModuleId, NominalTypeId, OpaqueTypeId, PackageId, PackageTargetId, ParameterId,
    PlaceId, RequirementId, TestId, TypeAliasId, TypeId, VariantId,
};
pub use origin::{DuplicateOrigin, ParameterOrigin, ResultProvenance};
pub use package::PackageIdentity;
pub use symbol::{Symbol, SymbolTable};
pub use target::{CompilationTarget, PackageTargetKind};
pub use type_projection::{TypeProjection, TypeProjectionError};
pub use type_store::{
    BorrowCapability, BuiltinType, CallableCapability, CallableContract, InvalidParameterOrigin,
    TypeKind, TypeStore, TypeStoreCheckpoint, UnknownTypeId,
};
