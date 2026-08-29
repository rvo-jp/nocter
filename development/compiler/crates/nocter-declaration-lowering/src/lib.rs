//! One-way lowering from syntax snapshots to the immutable declaration program.
//!
//! Filesystem discovery and package fetching are inputs, not responsibilities of this crate. The
//! lowering boundary canonicalizes an explicit compile unit, creates semantic identities, and
//! projects them back to source without exposing syntax to later semantic stages.

#[cfg(test)]
use nocter_compile_input::ToolchainInput;
use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageInput, PackageMode, PackageTargetResolutionInput, SourceVisibilityResolutionInput,
    UseResolutionInput,
};
use nocter_diagnostics::{DiagnosticNote, SourceDiagnostic};
use nocter_model::PackageIdentity;

mod authority_projection;
mod contract;
mod contract_diagnostic;
mod current_projection;
mod current_symbols;
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
mod projection_recipe;
mod recovery;
mod representation_contract;
mod reservation;
mod surface;
mod surface_diagnostic;
mod surface_origin;
mod toolchain;
mod topology;
mod topology_diagnostic;
mod topology_violation;
mod type_binding_diagnostic;
mod type_normalization_diagnostic;
#[allow(clippy::disallowed_types)]
mod types;
mod visibility;

#[cfg(test)]
mod test_support;

pub use authority_projection::DeclarationAuthorityProjection;
pub use contract::{DeclarationContractError, DeclarationContracts, analyze_declaration_contracts};
pub use contract_diagnostic::{DeclarationContractDiagnostic, DeclarationContractRule};
pub use current_projection::{CurrentDeclarationProjection, CurrentProjectionError};
pub use current_symbols::{CurrentCheckingSymbols, CurrentSymbolError};
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
pub(crate) use imports::apply_toolchain_profile;
pub use imports::{
    ImportError, ImportRule, ImportViolation, PreparedImports, PreparedNamespaces,
    prepare_authored_imports,
};
pub use namespace::{NamespaceRule, NamespaceViolation};
pub use namespace_diagnostic::NamespaceDiagnostic;
pub use pipeline::{
    DeclarationLoweringError, DeclarationLoweringFailure, lower_compile_unit_declarations,
    lower_compile_unit_declarations_recovering, lower_incomplete_body_declarations_recovering,
    lower_reusable_declarations,
};
pub use projection_recipe::{
    FrontendProjectionRecipe, ProjectionRecipeError, ReusableBodyIdentity,
};
pub use recovery::{
    DeclarationBodyAnalysisInput, DeclarationCheckingTransition, DeclarationLoweringRecovery,
};
#[cfg(test)]
pub(crate) use reservation::reserve_declaration_identities;
pub use reservation::{ReservationError, ReservedDeclarations, ReservedEntity};
pub use surface::{
    DeclarationSurface, SurfaceBlockImport, SurfaceDeclaration, SurfaceDeclarationId,
    SurfaceDeclarationKind, SurfaceError, SurfaceImport, SurfaceSource, SurfaceSourceId,
    SurfaceVisibility, collect_declaration_surface,
};
pub use surface_diagnostic::{SurfaceDiagnostic, SurfaceRule};
pub use toolchain::ToolchainError;
pub use topology::{
    LoweredDeclarations, LoweredTopology, LoweringError, PackageTargetResolutionError,
    ReusableDeclarations, lower_compile_unit_topology,
};
pub use topology_diagnostic::TopologyDiagnostic;
pub use topology_violation::{TopologyRule, TopologyViolation};
pub use type_binding_diagnostic::TypeBindingDiagnostic;
pub use type_normalization_diagnostic::TypeNormalizationDiagnostic;
pub use types::{
    BoundCallableType, BoundDeclarationPattern, BoundInterfaceApplication, BoundRequirementKind,
    BoundTypeId, BoundTypeKind, NormalizedDeclarationPattern, NormalizedOpaqueResult,
    PreparedTypeBindings, PreparedTypes, TypeBindingError, TypeBindingRule, TypeBindingViolation,
    TypeNormalizationError, TypeNormalizationRule, TypeNormalizationViolation,
    bind_header_type_syntax, evaluate_header_constants, normalize_header_types,
};
