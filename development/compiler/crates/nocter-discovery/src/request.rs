use std::path::PathBuf;

use nocter_compile_input::{ModuleIdentity, StructuralAttachmentInput};
use nocter_declarations::StandardDeclarationRole;
use nocter_filesystem::SourceOverlay;
use nocter_model::{BuiltinType, CompilationTarget, PackageIdentity};
use nocter_package::ResolvedPackageGraph;
use nocter_runtime_contract::PrimitiveRole;
use nocter_syntax::NodeKind;

/// The physical input shape selected by the command layer before source discovery.
///
/// Declared packages and explicit source files are different authored layouts. They converge on
/// one discovered compile-unit graph, but discovery never guesses one layout from the other.
#[derive(Debug)]
pub enum DiscoveryLayout {
    Declared {
        packages: ResolvedPackageGraph,
        roots: Vec<ModuleIdentity>,
    },
    ToolchainStandard {
        package: ResolvedPackageGraph,
    },
    SingleFile {
        source: PathBuf,
        support_packages: ResolvedPackageGraph,
    },
}

/// One compiler-owned standard semantic role selected from the visible contract with an exact
/// module, declaration kind, and declaration name.
///
/// Discovery deliberately ignores matching private implementation fragments. It resolves this
/// locator to the one authored contract token before declaration joining gives contract and body
/// one semantic identity.
#[derive(Clone, Debug)]
pub struct StandardRoleLocator {
    role: StandardDeclarationRole,
    module: ModuleIdentity,
    kind: NodeKind,
    name: Box<str>,
}

/// One primitive role and the exact declaration shape selected by the compiler toolchain.
#[derive(Clone, Debug)]
pub struct PrimitiveRoleLocator {
    role: PrimitiveRole,
    module: ModuleIdentity,
    name: Box<str>,
}

/// Exact authored declaration selected for one named compiler-represented type.
#[derive(Clone, Debug)]
pub struct BuiltinTypeLocator {
    builtin: BuiltinType,
    module: ModuleIdentity,
    name: Box<str>,
}

impl BuiltinTypeLocator {
    #[must_use]
    pub fn new(builtin: BuiltinType, module: ModuleIdentity, name: impl Into<Box<str>>) -> Self {
        Self {
            builtin,
            module,
            name: name.into(),
        }
    }

    #[must_use]
    pub const fn builtin(&self) -> BuiltinType {
        self.builtin
    }

    #[must_use]
    pub const fn module(&self) -> &ModuleIdentity {
        &self.module
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }
}

impl PrimitiveRoleLocator {
    #[must_use]
    pub fn new(role: PrimitiveRole, module: ModuleIdentity, name: impl Into<Box<str>>) -> Self {
        Self {
            role,
            module,
            name: name.into(),
        }
    }

    #[must_use]
    pub const fn role(&self) -> PrimitiveRole {
        self.role
    }

    #[must_use]
    pub const fn module(&self) -> &ModuleIdentity {
        &self.module
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }
}

impl StandardRoleLocator {
    #[must_use]
    pub fn new(
        role: StandardDeclarationRole,
        module: ModuleIdentity,
        kind: NodeKind,
        name: impl Into<Box<str>>,
    ) -> Self {
        Self {
            role,
            module,
            kind,
            name: name.into(),
        }
    }

    #[must_use]
    pub const fn role(&self) -> StandardDeclarationRole {
        self.role
    }

    #[must_use]
    pub const fn module(&self) -> &ModuleIdentity {
        &self.module
    }

    #[must_use]
    pub const fn kind(&self) -> NodeKind {
        self.kind
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }
}

/// Exact standard-library authority supplied by the selected compiler toolchain.
#[derive(Clone, Debug)]
pub struct ToolchainRequest {
    standard_package: PackageIdentity,
    prelude: ModuleIdentity,
    structural_attachments: Vec<StructuralAttachmentInput>,
    standard_roles: Vec<StandardRoleLocator>,
    primitive_roles: Vec<PrimitiveRoleLocator>,
    builtin_types: Vec<BuiltinTypeLocator>,
}

impl ToolchainRequest {
    #[must_use]
    pub fn new(
        standard_package: PackageIdentity,
        prelude: ModuleIdentity,
        structural_attachments: Vec<StructuralAttachmentInput>,
        standard_roles: Vec<StandardRoleLocator>,
    ) -> Self {
        Self {
            standard_package,
            prelude,
            structural_attachments,
            standard_roles,
            primitive_roles: Vec::new(),
            builtin_types: Vec::new(),
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
    pub fn structural_attachments(&self) -> &[StructuralAttachmentInput] {
        &self.structural_attachments
    }

    #[must_use]
    pub fn standard_roles(&self) -> &[StandardRoleLocator] {
        &self.standard_roles
    }

    #[must_use]
    pub fn primitive_roles(&self) -> &[PrimitiveRoleLocator] {
        &self.primitive_roles
    }

    #[must_use]
    pub fn builtin_types(&self) -> &[BuiltinTypeLocator] {
        &self.builtin_types
    }

    #[must_use]
    pub fn with_primitive_roles(mut self, roles: Vec<PrimitiveRoleLocator>) -> Self {
        self.primitive_roles = roles;
        self
    }

    #[must_use]
    pub fn with_builtin_types(mut self, builtins: Vec<BuiltinTypeLocator>) -> Self {
        self.builtin_types = builtins;
        self
    }
}

/// Closed package graph and initial directory modules selected for one compile unit.
#[derive(Debug)]
pub struct DiscoveryRequest {
    target: CompilationTarget,
    layout: DiscoveryLayout,
    toolchain: ToolchainRequest,
}

impl DiscoveryRequest {
    #[must_use]
    pub fn declared(
        target: CompilationTarget,
        packages: ResolvedPackageGraph,
        roots: Vec<ModuleIdentity>,
        toolchain: ToolchainRequest,
    ) -> Self {
        Self {
            target,
            layout: DiscoveryLayout::Declared { packages, roots },
            toolchain,
        }
    }

    #[must_use]
    pub fn single_file(
        target: CompilationTarget,
        source: impl Into<PathBuf>,
        support_packages: ResolvedPackageGraph,
        toolchain: ToolchainRequest,
    ) -> Self {
        Self {
            target,
            layout: DiscoveryLayout::SingleFile {
                source: source.into(),
                support_packages,
            },
            toolchain,
        }
    }

    /// Selects every authored module in the exact standard package for editor analysis.
    ///
    /// Unlike an ordinary declared root, the toolchain standard is already present under its
    /// compiler-selected identity. This layout must not synthesize a second path-package identity
    /// for the same physical root.
    #[must_use]
    pub fn toolchain_standard(
        target: CompilationTarget,
        package: ResolvedPackageGraph,
        toolchain: ToolchainRequest,
    ) -> Self {
        Self {
            target,
            layout: DiscoveryLayout::ToolchainStandard { package },
            toolchain,
        }
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub const fn layout(&self) -> &DiscoveryLayout {
        &self.layout
    }

    #[must_use]
    pub const fn toolchain(&self) -> &ToolchainRequest {
        &self.toolchain
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        match &self.layout {
            DiscoveryLayout::Declared { packages, .. }
            | DiscoveryLayout::ToolchainStandard { package: packages }
            | DiscoveryLayout::SingleFile {
                support_packages: packages,
                ..
            } => packages.source_overlay(),
        }
    }

    pub(crate) fn into_parts(self) -> (CompilationTarget, DiscoveryLayout, ToolchainRequest) {
        (self.target, self.layout, self.toolchain)
    }
}
