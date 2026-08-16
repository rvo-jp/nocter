//! Syntax-independent semantic identities and structural types.
//!
//! This crate deliberately has no source or syntax dependency. Source lowering may create these
//! values, but later compiler stages consume them without gaining a reverse path to syntax trees,
//! source ranges, or rendered type spellings.

mod id;
mod origin;
mod symbol;
mod type_store;

pub use id::{
    AssociatedTypeId, BodyId, CallableId, ConformanceId, ConstructionId, DeclarationSiteId, DropId,
    FieldId, GenericParameterId, ImportId, InstanceId, InterfaceId, ModuleId, NominalTypeId,
    OpaqueTypeId, PackageId, PackageTargetId, ParameterId, TestId, TypeAliasId, TypeId, VariantId,
};
pub use origin::{DuplicateOrigin, ParameterOrigin, ResultProvenance};
pub use symbol::{Symbol, SymbolTable};
pub use type_store::{
    BorrowCapability, BuiltinType, CallableCapability, CallableContract, InvalidParameterOrigin,
    TypeKind, TypeStore, UnknownTypeId,
};
