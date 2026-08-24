//! Discovery-owned, syntax-borrowing input for one Nocter compile unit.
//!
//! This crate defines the complete handoff from source discovery to semantic lowering. It owns no
//! filesystem policy and performs no semantic work; producers resolve physical topology once and
//! consumers treat every identity and edge as immutable input.

use nocter_declarations::{BuiltinAttachment, StandardDeclarationRole};
use nocter_model::{CompilationTarget, PackageIdentity, PackageTargetKind};
use nocter_runtime_contract::PrimitiveRole;
use nocter_source::SourceMap;
use nocter_syntax::{NodeId, SyntaxToken, SyntaxTree};

mod dependency;
mod identity;

pub use dependency::{SourceVisibilityResolutionInput, UseResolutionInput};
pub use identity::{ModuleIdentity, is_valid_module_segment};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PackageMode {
    Declared,
    SingleFile,
}

/// Package identity selected independently of its root-module source.
///
/// Declared package syntax is not repeated here: the root module's `Root` source is the single
/// package declaration authority. This keeps discovery from handing lowering two representations
/// of the same physical `index.nct`.
#[derive(Clone, Debug)]
pub struct PackageInput {
    identity: PackageIdentity,
    display_name: Box<str>,
    mode: PackageMode,
}

impl PackageInput {
    #[must_use]
    pub fn new(
        identity: PackageIdentity,
        display_name: impl Into<Box<str>>,
        mode: PackageMode,
    ) -> Self {
        Self {
            identity,
            display_name: display_name.into(),
            mode,
        }
    }

    #[must_use]
    pub const fn identity(&self) -> &PackageIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub const fn mode(&self) -> PackageMode {
        self.mode
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ModuleSourceKind {
    Root,
    Implementation,
    SingleFile,
}

/// One package target directive paired with the directory module selected by discovery.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PackageTargetResolutionInput {
    declaration: NodeId,
    name: Box<str>,
    name_literal: NodeId,
    kind: PackageTargetKind,
    declaration_order: u32,
    module: ModuleIdentity,
}

impl PackageTargetResolutionInput {
    #[must_use]
    pub fn new(
        declaration: NodeId,
        name: impl Into<Box<str>>,
        name_literal: NodeId,
        kind: PackageTargetKind,
        declaration_order: u32,
        module: ModuleIdentity,
    ) -> Self {
        Self {
            declaration,
            name: name.into(),
            name_literal,
            kind,
            declaration_order,
            module,
        }
    }

    #[must_use]
    pub const fn declaration(&self) -> NodeId {
        self.declaration
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn name_literal(&self) -> NodeId {
        self.name_literal
    }

    #[must_use]
    pub const fn kind(&self) -> PackageTargetKind {
        self.kind
    }

    #[must_use]
    pub const fn declaration_order(&self) -> u32 {
        self.declaration_order
    }

    #[must_use]
    pub const fn module(&self) -> &ModuleIdentity {
        &self.module
    }
}

/// One exact source declaration selected for a compiler-owned standard semantic role.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StandardRoleInput {
    role: StandardDeclarationRole,
    declaration: SyntaxToken,
}

/// One exact source callable selected for a compiler-owned primitive role.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PrimitiveRoleInput {
    role: PrimitiveRole,
    declaration: SyntaxToken,
}

impl PrimitiveRoleInput {
    #[must_use]
    pub const fn new(role: PrimitiveRole, declaration: SyntaxToken) -> Self {
        Self { role, declaration }
    }

    #[must_use]
    pub const fn role(self) -> PrimitiveRole {
        self.role
    }

    #[must_use]
    pub const fn declaration(self) -> SyntaxToken {
        self.declaration
    }
}

/// One compiler-owned built-in surface paired with its exact authored module.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BuiltinAttachmentInput {
    attachment: BuiltinAttachment,
    module: ModuleIdentity,
}

impl BuiltinAttachmentInput {
    #[must_use]
    pub const fn new(attachment: BuiltinAttachment, module: ModuleIdentity) -> Self {
        Self { attachment, module }
    }

    #[must_use]
    pub const fn attachment(&self) -> BuiltinAttachment {
        self.attachment
    }

    #[must_use]
    pub const fn module(&self) -> &ModuleIdentity {
        &self.module
    }
}

/// Exact standard-library authority selected once by toolchain discovery.
///
/// The semantic pipeline may resolve these identities into dense program IDs, but it must never
/// reconstruct them from package names, module spellings, or declaration names.
#[derive(Clone, Debug)]
pub struct ToolchainInput {
    standard_package: PackageIdentity,
    prelude: ModuleIdentity,
    builtin_attachments: Vec<BuiltinAttachmentInput>,
    standard_roles: Vec<StandardRoleInput>,
    primitive_roles: Vec<PrimitiveRoleInput>,
}

impl ToolchainInput {
    #[must_use]
    pub fn new(
        standard_package: PackageIdentity,
        prelude: ModuleIdentity,
        builtin_attachments: Vec<BuiltinAttachmentInput>,
        standard_roles: Vec<StandardRoleInput>,
    ) -> Self {
        Self {
            standard_package,
            prelude,
            builtin_attachments,
            standard_roles,
            primitive_roles: Vec::new(),
        }
    }

    #[must_use]
    pub const fn standard_package(&self) -> &PackageIdentity {
        &self.standard_package
    }

    #[must_use]
    pub const fn prelude(&self) -> &ModuleIdentity {
        &self.prelude
    }

    #[must_use]
    pub fn builtin_attachments(&self) -> &[BuiltinAttachmentInput] {
        &self.builtin_attachments
    }

    #[must_use]
    pub fn standard_roles(&self) -> &[StandardRoleInput] {
        &self.standard_roles
    }

    #[must_use]
    pub fn primitive_roles(&self) -> &[PrimitiveRoleInput] {
        &self.primitive_roles
    }

    #[must_use]
    pub fn with_standard_roles(mut self, roles: Vec<StandardRoleInput>) -> Self {
        self.standard_roles = roles;
        self
    }

    #[must_use]
    pub fn with_primitive_roles(mut self, roles: Vec<PrimitiveRoleInput>) -> Self {
        self.primitive_roles = roles;
        self
    }
}

impl StandardRoleInput {
    #[must_use]
    pub const fn new(role: StandardDeclarationRole, declaration: SyntaxToken) -> Self {
        Self { role, declaration }
    }

    #[must_use]
    pub const fn role(self) -> StandardDeclarationRole {
        self.role
    }

    #[must_use]
    pub const fn declaration(self) -> SyntaxToken {
        self.declaration
    }
}

#[derive(Clone, Debug)]
pub struct ModuleSourceInput<'syntax> {
    canonical_path: Box<str>,
    kind: ModuleSourceKind,
    syntax: &'syntax SyntaxTree,
}

impl<'syntax> ModuleSourceInput<'syntax> {
    #[must_use]
    pub fn new(
        canonical_path: impl Into<Box<str>>,
        kind: ModuleSourceKind,
        syntax: &'syntax SyntaxTree,
    ) -> Self {
        Self {
            canonical_path: canonical_path.into(),
            kind,
            syntax,
        }
    }

    #[must_use]
    pub const fn canonical_path(&self) -> &str {
        &self.canonical_path
    }

    #[must_use]
    pub const fn kind(&self) -> ModuleSourceKind {
        self.kind
    }

    #[must_use]
    pub const fn syntax(&self) -> &'syntax SyntaxTree {
        self.syntax
    }
}

#[derive(Clone, Debug)]
pub struct ModuleInput<'syntax> {
    identity: ModuleIdentity,
    sources: Vec<ModuleSourceInput<'syntax>>,
}

impl<'syntax> ModuleInput<'syntax> {
    #[must_use]
    pub fn new(identity: ModuleIdentity, sources: Vec<ModuleSourceInput<'syntax>>) -> Self {
        Self { identity, sources }
    }

    #[must_use]
    pub const fn identity(&self) -> &ModuleIdentity {
        &self.identity
    }

    #[must_use]
    pub fn sources(&self) -> &[ModuleSourceInput<'syntax>] {
        &self.sources
    }
}

#[derive(Debug)]
pub struct CompileUnitInput<'syntax> {
    target: CompilationTarget,
    sources: &'syntax SourceMap,
    packages: Vec<PackageInput>,
    root_packages: Vec<PackageIdentity>,
    modules: Vec<ModuleInput<'syntax>>,
    source_visibility_resolutions: Vec<SourceVisibilityResolutionInput>,
    use_resolutions: Vec<UseResolutionInput>,
    package_target_resolutions: Vec<PackageTargetResolutionInput>,
    toolchain: Option<ToolchainInput>,
}

impl<'syntax> CompileUnitInput<'syntax> {
    #[must_use]
    pub fn new(
        target: CompilationTarget,
        sources: &'syntax SourceMap,
        packages: Vec<PackageInput>,
        modules: Vec<ModuleInput<'syntax>>,
        use_resolutions: Vec<UseResolutionInput>,
    ) -> Self {
        Self {
            target,
            sources,
            packages,
            root_packages: Vec::new(),
            modules,
            source_visibility_resolutions: Vec::new(),
            use_resolutions,
            package_target_resolutions: Vec::new(),
            toolchain: None,
        }
    }

    /// Adds exact physical-source edges selected from authored `see` declarations.
    #[must_use]
    pub fn with_source_visibility_resolutions(
        mut self,
        resolutions: Vec<SourceVisibilityResolutionInput>,
    ) -> Self {
        self.source_visibility_resolutions = resolutions;
        self
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub fn with_target(mut self, target: CompilationTarget) -> Self {
        self.target = target;
        self
    }

    #[must_use]
    pub fn with_package_target_resolutions(
        mut self,
        resolutions: Vec<PackageTargetResolutionInput>,
    ) -> Self {
        self.package_target_resolutions = resolutions;
        self
    }

    /// Freezes the package roots selected by the command before dependency discovery.
    #[must_use]
    pub fn with_root_packages(mut self, packages: Vec<PackageIdentity>) -> Self {
        self.root_packages = packages;
        self
    }

    #[must_use]
    pub fn with_toolchain(mut self, toolchain: ToolchainInput) -> Self {
        self.toolchain = Some(toolchain);
        self
    }

    #[must_use]
    pub const fn sources(&self) -> &'syntax SourceMap {
        self.sources
    }

    #[must_use]
    pub fn packages(&self) -> &[PackageInput] {
        &self.packages
    }

    #[must_use]
    pub fn root_packages(&self) -> &[PackageIdentity] {
        &self.root_packages
    }

    #[must_use]
    pub fn modules(&self) -> &[ModuleInput<'syntax>] {
        &self.modules
    }

    #[must_use]
    pub fn use_resolutions(&self) -> &[UseResolutionInput] {
        &self.use_resolutions
    }

    #[must_use]
    pub fn source_visibility_resolutions(&self) -> &[SourceVisibilityResolutionInput] {
        &self.source_visibility_resolutions
    }

    #[must_use]
    pub fn package_target_resolutions(&self) -> &[PackageTargetResolutionInput] {
        &self.package_target_resolutions
    }

    #[must_use]
    pub const fn toolchain(&self) -> Option<&ToolchainInput> {
        self.toolchain.as_ref()
    }
}
