use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nocter_compile_input::{BuiltinAttachmentInput, ModuleIdentity, PackageIdentity};
use nocter_declarations::{PrimitiveRole, StandardDeclarationRole};
use nocter_model::CompilationTarget;
use nocter_syntax::NodeKind;

/// One package whose exact identity, physical root, and dependency aliases were resolved before
/// source discovery.
#[derive(Clone, Debug)]
pub struct ResolvedPackage {
    identity: PackageIdentity,
    display_name: Box<str>,
    root: PathBuf,
    dependencies: BTreeMap<Box<str>, PackageIdentity>,
}

/// The physical input shape selected by the command layer before source discovery.
///
/// Declared packages and explicit source files are different authored layouts. They converge on
/// one discovered compile-unit graph, but discovery never guesses one layout from the other.
#[derive(Debug)]
pub enum DiscoveryLayout {
    Declared {
        packages: Vec<ResolvedPackage>,
        roots: Vec<ModuleIdentity>,
    },
    SingleFile {
        source: PathBuf,
        support_packages: Vec<ResolvedPackage>,
    },
}

/// One compiler-owned standard semantic role selected by exact module, declaration kind, and
/// declaration name. Discovery resolves this locator to one syntax token before semantic lowering.
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
    kind: NodeKind,
    name: Box<str>,
}

impl PrimitiveRoleLocator {
    #[must_use]
    pub fn new(
        role: PrimitiveRole,
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
    pub const fn role(&self) -> PrimitiveRole {
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
    builtin_attachments: Vec<BuiltinAttachmentInput>,
    standard_roles: Vec<StandardRoleLocator>,
    primitive_roles: Vec<PrimitiveRoleLocator>,
}

impl ToolchainRequest {
    #[must_use]
    pub fn new(
        standard_package: PackageIdentity,
        prelude: ModuleIdentity,
        builtin_attachments: Vec<BuiltinAttachmentInput>,
        standard_roles: Vec<StandardRoleLocator>,
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
    pub fn standard_roles(&self) -> &[StandardRoleLocator] {
        &self.standard_roles
    }

    #[must_use]
    pub fn primitive_roles(&self) -> &[PrimitiveRoleLocator] {
        &self.primitive_roles
    }

    #[must_use]
    pub fn with_primitive_roles(mut self, roles: Vec<PrimitiveRoleLocator>) -> Self {
        self.primitive_roles = roles;
        self
    }
}

impl ResolvedPackage {
    #[must_use]
    pub fn new(
        identity: PackageIdentity,
        display_name: impl Into<Box<str>>,
        root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            identity,
            display_name: display_name.into(),
            root: root.into(),
            dependencies: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_dependency(mut self, alias: impl Into<Box<str>>, package: PackageIdentity) -> Self {
        self.dependencies.insert(alias.into(), package);
        self
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
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn dependencies(&self) -> &BTreeMap<Box<str>, PackageIdentity> {
        &self.dependencies
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
        packages: Vec<ResolvedPackage>,
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
        support_packages: Vec<ResolvedPackage>,
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

    pub(crate) fn into_parts(self) -> (CompilationTarget, DiscoveryLayout, ToolchainRequest) {
        (self.target, self.layout, self.toolchain)
    }
}
