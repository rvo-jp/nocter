use nocter_declarations::StandardDeclarationRole;
use nocter_model::CompilationTarget;
use nocter_source::SourceMap;
use nocter_syntax::{NodeId, SyntaxToken, SyntaxTree};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageIdentity(Box<str>);

impl PackageIdentity {
    #[must_use]
    pub fn new(identity: impl Into<Box<str>>) -> Self {
        Self(identity.into())
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PackageMode {
    Declared,
    SingleFile,
}

/// The authored package declaration and its canonical physical identity.
///
/// The path is supplied by package discovery. Lowering treats it as an opaque, already
/// canonicalized key and never probes the filesystem.
#[derive(Clone, Debug)]
pub struct PackageDeclarationInput<'syntax> {
    canonical_path: Box<str>,
    syntax: &'syntax SyntaxTree,
}

impl<'syntax> PackageDeclarationInput<'syntax> {
    #[must_use]
    pub fn new(canonical_path: impl Into<Box<str>>, syntax: &'syntax SyntaxTree) -> Self {
        Self {
            canonical_path: canonical_path.into(),
            syntax,
        }
    }

    #[must_use]
    pub const fn canonical_path(&self) -> &str {
        &self.canonical_path
    }

    #[must_use]
    pub const fn syntax(&self) -> &'syntax SyntaxTree {
        self.syntax
    }
}

#[derive(Clone, Debug)]
pub struct PackageInput<'syntax> {
    identity: PackageIdentity,
    display_name: Box<str>,
    mode: PackageMode,
    declaration: Option<PackageDeclarationInput<'syntax>>,
}

impl<'syntax> PackageInput<'syntax> {
    #[must_use]
    pub fn new(
        identity: PackageIdentity,
        display_name: impl Into<Box<str>>,
        mode: PackageMode,
        declaration: Option<PackageDeclarationInput<'syntax>>,
    ) -> Self {
        Self {
            identity,
            display_name: display_name.into(),
            mode,
            declaration,
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

    #[must_use]
    pub const fn declaration(&self) -> Option<&PackageDeclarationInput<'syntax>> {
        self.declaration.as_ref()
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModuleIdentity {
    package: PackageIdentity,
    path: Box<[Box<str>]>,
}

impl ModuleIdentity {
    #[must_use]
    pub fn new<S>(package: PackageIdentity, path: impl IntoIterator<Item = S>) -> Self
    where
        S: Into<Box<str>>,
    {
        Self {
            package,
            path: path
                .into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    #[must_use]
    pub const fn package(&self) -> &PackageIdentity {
        &self.package
    }

    #[must_use]
    pub const fn path(&self) -> &[Box<str>] {
        &self.path
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ModuleSourceKind {
    Root,
    Implementation,
    SingleFile,
}

/// The exact graph edge selected by package/source discovery for one authored `use`.
///
/// Lowering never reconstructs this distinction from a canonical path. Source targets compose a
/// physical implementation source into the importing module; module targets enter semantic name
/// resolution.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UseTargetInput {
    Source(Box<str>),
    Module(ModuleIdentity),
}

/// One resolved `use` node and its discovery-owned target.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct UseResolutionInput {
    declaration: NodeId,
    target: UseTargetInput,
}

impl UseResolutionInput {
    #[must_use]
    pub const fn new(declaration: NodeId, target: UseTargetInput) -> Self {
        Self {
            declaration,
            target,
        }
    }

    #[must_use]
    pub const fn declaration(&self) -> NodeId {
        self.declaration
    }

    #[must_use]
    pub const fn target(&self) -> &UseTargetInput {
        &self.target
    }
}

/// One package target directive and the exact directory module selected by package discovery.
///
/// The directive remains the authority for target kind, name, and declaration order. Discovery
/// owns only path resolution; lowering never reinterprets the authored `module` string.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PackageTargetResolutionInput {
    declaration: NodeId,
    module: ModuleIdentity,
}

impl PackageTargetResolutionInput {
    #[must_use]
    pub const fn new(declaration: NodeId, module: ModuleIdentity) -> Self {
        Self {
            declaration,
            module,
        }
    }

    #[must_use]
    pub const fn declaration(&self) -> NodeId {
        self.declaration
    }

    #[must_use]
    pub const fn module(&self) -> &ModuleIdentity {
        &self.module
    }
}

/// One exact source declaration selected for a compiler-owned standard semantic role.
///
/// Package discovery supplies the declaration-name token. Checking validates its semantic shape
/// and standard-package ownership. Neither layer searches for
/// a matching spelling, so a project declaration with the same name cannot acquire authority.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StandardRoleInput {
    role: StandardDeclarationRole,
    declaration: SyntaxToken,
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
    packages: Vec<PackageInput<'syntax>>,
    modules: Vec<ModuleInput<'syntax>>,
    use_resolutions: Vec<UseResolutionInput>,
    package_target_resolutions: Vec<PackageTargetResolutionInput>,
    standard_roles: Vec<StandardRoleInput>,
}

impl<'syntax> CompileUnitInput<'syntax> {
    #[must_use]
    pub fn new(
        target: CompilationTarget,
        sources: &'syntax SourceMap,
        packages: Vec<PackageInput<'syntax>>,
        modules: Vec<ModuleInput<'syntax>>,
        use_resolutions: Vec<UseResolutionInput>,
    ) -> Self {
        Self {
            target,
            sources,
            packages,
            modules,
            use_resolutions,
            package_target_resolutions: Vec::new(),
            standard_roles: Vec::new(),
        }
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    /// Reuses one immutable discovery snapshot for another explicitly selected target.
    #[must_use]
    pub fn with_target(mut self, target: CompilationTarget) -> Self {
        self.target = target;
        self
    }

    /// Attaches package-discovery target resolutions to the immutable source snapshot.
    #[must_use]
    pub fn with_package_target_resolutions(
        mut self,
        resolutions: Vec<PackageTargetResolutionInput>,
    ) -> Self {
        self.package_target_resolutions = resolutions;
        self
    }

    /// Attaches discovery-selected standard declarations without changing the common constructor
    /// used by source-only and focused compiler tests.
    #[must_use]
    pub fn with_standard_roles(mut self, roles: Vec<StandardRoleInput>) -> Self {
        self.standard_roles = roles;
        self
    }

    #[must_use]
    pub const fn sources(&self) -> &'syntax SourceMap {
        self.sources
    }

    #[must_use]
    pub fn packages(&self) -> &[PackageInput<'syntax>] {
        &self.packages
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
    pub fn package_target_resolutions(&self) -> &[PackageTargetResolutionInput] {
        &self.package_target_resolutions
    }

    #[must_use]
    pub fn standard_roles(&self) -> &[StandardRoleInput] {
        &self.standard_roles
    }
}
