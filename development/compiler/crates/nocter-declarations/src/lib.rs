//! Immutable, syntax-independent declaration graph.
//!
//! This crate depends only on [`nocter_model`]. It cannot contain source files, byte ranges,
//! syntax nodes, or rendered type spellings. Syntax lowering constructs a [`DeclarationProgram`]
//! and a separate source index; semantic stages consume this program without importing the
//! lowering or syntax crates.

mod arenas;
mod callable;
mod constant;
mod declaration;
mod import;
mod namespace;
mod path;
mod program;
mod requirement;
mod standard;
mod target;
mod validate;
mod visibility;

pub use arenas::{
    DeclarationArenaBuilder, DeclarationArenas, DefinitionError, IncompleteDefinition,
};
pub use callable::{
    Body, BodyOwner, CallableDeclaration, CallableKind, CallableOwner, CallableProvenance,
    CallableProvenanceContract, DuplicateCallableOrigin, LiteralShape, Parameter, ParameterOwner,
    ParameterRole, ProvenanceAnnotation, ProvenanceOrigin,
};
pub use constant::ConstantDeclaration;
pub use declaration::{
    AssociatedTypeBinding, AssociatedTypeDeclaration, ConformanceDeclaration,
    ConstructionDeclaration, DropDeclaration, FieldDeclaration, GenericOwner, GenericParameter,
    InstanceDeclaration, InterfaceApplication, InterfaceDeclaration, NominalShape,
    NominalTypeDeclaration, OpaqueTypeDeclaration, TestDeclaration, TypeAliasDeclaration,
    VariantDeclaration,
};
pub use import::{ExportedEntity, ImportDeclaration, ImportTarget, ImportedName};
pub use namespace::{DuplicateNamespaceName, FallbackEntry, ModuleNamespace, NamespaceEntry};
pub use path::ModulePath;
pub use program::{
    DeclarationGraph, DeclarationProgram, DeclarationProgramBuilder, DeclarationSite, Module,
    Package, ProgramBuildError, ProgramBuildFailure,
};
pub use requirement::{
    ExpansionCapability, Requirement, RequirementKind, RequirementOwner, RequirementSubject,
    StructuralCapability,
};
pub use standard::{BuiltinAttachment, StandardDeclarationRole, StandardLibrary};
pub use target::PackageTarget;
pub use validate::{
    DeclarationDomain, DeclarationRule, DeclarationViolation, ProgramIntegrityError,
    ProgramValidationError,
};
pub use visibility::Visibility;
