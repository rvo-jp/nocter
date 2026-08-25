//! One-way lowering from syntax snapshots to the immutable declaration program.
//!
//! Filesystem discovery and package fetching are inputs, not responsibilities of this crate. The
//! lowering boundary canonicalizes an explicit compile unit, creates semantic identities, and
//! projects them back to source without exposing syntax to later semantic stages.

use nocter_compile_input::{
    BuiltinTypeInput, CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput,
    ModuleSourceKind, PackageInput, PackageMode, PackageTargetResolutionInput,
    SourceVisibilityResolutionInput, ToolchainInput, UseResolutionInput,
};
use nocter_diagnostics::{DiagnosticNote, SourceDiagnostic};
use nocter_model::PackageIdentity;

mod contract;
mod contract_diagnostic;
mod definition_diagnostic;
mod definitions;
mod diagnostic;
mod frontend_projection;
mod generic_diagnostic;
mod generics;
mod headers;
mod import_diagnostic;
mod imports;
mod namespace;
mod namespace_diagnostic;
mod package_source;
mod package_targets;
mod pipeline;
mod primitive_bindings;
mod recovery;
mod representation_contract;
mod reservation;
mod surface;
mod surface_diagnostic;
mod surface_origin;
mod topology;
mod topology_diagnostic;
mod topology_violation;
mod type_binding_diagnostic;
mod type_normalization_diagnostic;
mod types;
mod visibility;

#[cfg(test)]
mod test_support;

pub use contract::{DeclarationContractError, DeclarationContracts, analyze_declaration_contracts};
pub use contract_diagnostic::{DeclarationContractDiagnostic, DeclarationContractRule};
pub use definition_diagnostic::DefinitionDiagnostic;
pub use definitions::{
    DeclarationDiagnostics, DefinitionRule, DefinitionViolation, HeaderDefinitionError,
};
pub use generic_diagnostic::GenericDiagnostic;
pub use generics::{
    GenericError, GenericRule, GenericViolation, PreparedGenerics, prepare_generic_binders,
};
pub use headers::{HeaderError, PreparedHeaders, prepare_declaration_headers};
pub use import_diagnostic::ImportDiagnostic;
pub use imports::{
    ImportError, ImportRule, ImportViolation, PreparedImports, PreparedNamespaces, ToolchainError,
    apply_toolchain_profile, prepare_authored_imports,
};
pub use namespace::{NamespaceRule, NamespaceViolation};
pub use namespace_diagnostic::NamespaceDiagnostic;
pub use pipeline::{
    DeclarationLoweringError, DeclarationLoweringFailure, lower_compile_unit_declarations,
    lower_compile_unit_declarations_recovering, lower_incomplete_body_declarations_recovering,
};
pub use primitive_bindings::{PrimitiveResolutionError, resolve_primitive_bindings};
pub use recovery::{
    DeclarationBodyAnalysisInput, DeclarationCheckingTransition, DeclarationLoweringRecovery,
};
pub use reservation::{
    ReservationError, ReservedDeclarations, ReservedEntity, reserve_declaration_identities,
};
pub use surface::{
    DeclarationSurface, SurfaceDeclaration, SurfaceDeclarationId, SurfaceDeclarationKind,
    SurfaceError, SurfaceImport, SurfaceSource, SurfaceSourceId, SurfaceVisibility,
    collect_declaration_surface,
};
pub use surface_diagnostic::{SurfaceDiagnostic, SurfaceRule};
pub use topology::{
    LoweredDeclarations, LoweringError, PackageTargetResolutionError, lower_compile_unit_topology,
};
pub use topology_diagnostic::TopologyDiagnostic;
pub use topology_violation::{TopologyRule, TopologyViolation};
pub use type_binding_diagnostic::TypeBindingDiagnostic;
pub use type_normalization_diagnostic::TypeNormalizationDiagnostic;
pub use types::{
    BoundCallableType, BoundCapability, BoundDeclarationPattern, BoundRequirementKind, BoundTypeId,
    BoundTypeKind, NormalizedDeclarationPattern, NormalizedOpaqueResult, PreparedTypeBindings,
    PreparedTypes, TypeBindingError, TypeBindingRule, TypeBindingViolation, TypeNormalizationError,
    TypeNormalizationRule, TypeNormalizationViolation, bind_header_type_syntax,
    evaluate_header_constants, normalize_header_types,
};
