//! Syntax-independent semantic identities and structural types.
//!
//! This crate deliberately has no source or syntax dependency. Source lowering may create these
//! values, but later compiler stages consume them without gaining a reverse path to syntax trees,
//! source ranges, or rendered type spellings.

mod arena;
mod id;
mod origin;
mod symbol;
mod type_store;

pub use arena::{Arena, ArenaBuilder};
pub use id::{
    AssociatedTypeId, BodyId, BodyNodeId, BodyScopeId, CallableId, CaptureId, ClosureId,
    ConformanceId, ConstructionId, DeclarationSiteId, DropId, FieldId, GenericParameterId,
    ImportId, InstanceId, InterfaceId, LocalBindingId, LoopId, ModuleId, NominalTypeId,
    OpaqueTypeId, PackageId, PackageTargetId, ParameterId, PlaceId, RequirementId, TestId,
    TypeAliasId, TypeId, VariantId,
};
pub use origin::{DuplicateOrigin, ParameterOrigin, ResultProvenance};
pub use symbol::{Symbol, SymbolTable};
pub use type_store::{
    BorrowCapability, BuiltinType, CallableCapability, CallableContract, InvalidParameterOrigin,
    TypeKind, TypeStore, UnknownTypeId,
};
