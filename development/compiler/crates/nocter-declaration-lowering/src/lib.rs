//! One-way lowering from syntax snapshots to the immutable declaration program.
//!
//! Filesystem discovery and package fetching are inputs, not responsibilities of this crate. The
//! lowering boundary canonicalizes an explicit compile unit, creates semantic identities, and
//! projects them back to source without exposing syntax to later semantic stages.

mod contract;
mod input;
mod reservation;
mod surface;
mod topology;

pub use contract::{CallableContractError, CallableContracts, analyze_callable_contracts};
pub use input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
};
pub use reservation::{
    ReservationError, ReservedDeclarations, ReservedEntity, reserve_declaration_identities,
};
pub use surface::{
    DeclarationSurface, SurfaceDeclaration, SurfaceDeclarationId, SurfaceDeclarationKind,
    SurfaceError, SurfaceImport, SurfaceSource, SurfaceSourceId, collect_declaration_surface,
};
pub use topology::{LoweredDeclarations, LoweringError, lower_compile_unit_topology};
