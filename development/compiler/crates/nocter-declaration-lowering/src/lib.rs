//! One-way lowering from syntax snapshots to the immutable declaration program.
//!
//! Filesystem discovery and package fetching are inputs, not responsibilities of this crate. The
//! lowering boundary canonicalizes an explicit compile unit, creates semantic identities, and
//! projects them back to source without exposing syntax to later semantic stages.

mod contract;
mod generics;
mod headers;
mod imports;
mod input;
mod reservation;
mod surface;
mod topology;
mod types;
mod visibility;

#[cfg(test)]
mod test_support;

pub use contract::{CallableContractError, CallableContracts, analyze_callable_contracts};
pub use generics::{GenericError, PreparedGenerics, prepare_generic_binders};
pub use headers::{HeaderError, PreparedHeaders, prepare_declaration_headers};
pub use imports::{
    ImportError, PreludeError, PreparedImports, PreparedNamespaces, apply_standard_prelude,
    prepare_authored_imports,
};
pub use input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode, UseResolutionInput,
    UseTargetInput,
};
pub use reservation::{
    ReservationError, ReservedDeclarations, ReservedEntity, reserve_declaration_identities,
};
pub use surface::{
    DeclarationSurface, SurfaceDeclaration, SurfaceDeclarationId, SurfaceDeclarationKind,
    SurfaceError, SurfaceImport, SurfaceImportTarget, SurfaceSource, SurfaceSourceId,
    collect_declaration_surface,
};
pub use topology::{LoweredDeclarations, LoweringError, lower_compile_unit_topology};
pub use types::{
    BoundCallableType, BoundCapability, BoundDeclarationPattern, BoundRequirementKind, BoundTypeId,
    BoundTypeKind, PreparedTypeBindings, TypeBindingError, bind_header_type_syntax,
};
