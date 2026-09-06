use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use nocter_compile_input::{CompileUnitInput, ModuleIdentity, ModuleSourceKind, PackageMode};
use nocter_filesystem::SourceOverlay;
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_source::SourceMap;
use nocter_syntax::SyntaxTree;

#[derive(Clone, Debug)]
pub struct DiscoveredSource {
    canonical_path: Box<str>,
    kind: ModuleSourceKind,
    syntax: usize,
}

impl DiscoveredSource {
    pub(crate) const fn new(
        canonical_path: Box<str>,
        kind: ModuleSourceKind,
        syntax: usize,
    ) -> Self {
        Self {
            canonical_path,
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

    pub(crate) const fn syntax_index(&self) -> usize {
        self.syntax
    }
}

#[derive(Clone, Debug)]
pub struct DiscoveredModule {
    identity: ModuleIdentity,
    sources: Vec<DiscoveredSource>,
}

impl DiscoveredModule {
    pub(crate) const fn new(identity: ModuleIdentity, sources: Vec<DiscoveredSource>) -> Self {
        Self { identity, sources }
    }

    #[must_use]
    pub const fn identity(&self) -> &ModuleIdentity {
        &self.identity
    }

    #[must_use]
    pub fn sources(&self) -> &[DiscoveredSource] {
        &self.sources
    }
}

/// One exact cross-module dependency selected from an authored top-level or block `use`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiscoveredModuleDependency {
    source: ModuleIdentity,
    target: ModuleIdentity,
}

impl DiscoveredModuleDependency {
    pub(crate) const fn new(source: ModuleIdentity, target: ModuleIdentity) -> Self {
        Self { source, target }
    }

    #[must_use]
    pub const fn source(&self) -> &ModuleIdentity {
        &self.source
    }

    #[must_use]
    pub const fn target(&self) -> &ModuleIdentity {
        &self.target
    }
}

#[derive(Debug)]
pub(crate) struct DiscoveredPackage {
    pub(crate) identity: PackageIdentity,
    pub(crate) display_name: Box<str>,
    pub(crate) mode: PackageMode,
    pub(crate) dependencies: BTreeMap<Box<str>, PackageIdentity>,
}

#[derive(Debug)]
pub struct DiscoveredUnit {
    pub(crate) source_overlay: SourceOverlay,
    pub(crate) sources: Arc<SourceMap>,
    pub(crate) syntax: Arc<[SyntaxTree]>,
    pub(crate) packages: Vec<DiscoveredPackage>,
    pub(crate) modules: Vec<DiscoveredModule>,
    pub(crate) module_dependencies: Vec<DiscoveredModuleDependency>,
    pub(crate) compile_input: CompileUnitInput<'static>,
}

impl DiscoveredUnit {
    #[must_use]
    pub fn target(&self) -> CompilationTarget {
        self.compile_input.target()
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        &self.source_overlay
    }

    #[must_use]
    pub fn sources(&self) -> &SourceMap {
        &self.sources
    }

    #[must_use]
    pub fn syntax_trees(&self) -> &[SyntaxTree] {
        &self.syntax
    }

    #[must_use]
    pub fn modules(&self) -> &[DiscoveredModule] {
        &self.modules
    }

    /// Returns the exact packages selected before dependency traversal.
    #[must_use]
    pub fn root_packages(&self) -> &[PackageIdentity] {
        self.compile_input.root_packages()
    }

    /// Returns the exact authored dependency aliases of one resolved package.
    ///
    /// Alias spelling remains discovery-owned because it is a property of the importing package,
    /// not of the dependency's semantic identity.
    #[must_use]
    pub fn package_dependencies(
        &self,
        package: &PackageIdentity,
    ) -> Option<&BTreeMap<Box<str>, PackageIdentity>> {
        self.packages
            .iter()
            .find(|candidate| &candidate.identity == package)
            .map(|candidate| &candidate.dependencies)
    }

    /// Returns exact cross-module edges selected from top-level and block `use` declarations.
    #[must_use]
    pub fn module_dependencies(&self) -> &[DiscoveredModuleDependency] {
        &self.module_dependencies
    }

    /// Reports whether one discovered source is authored by a selected root package.
    ///
    /// The package root declaration is the root module's physical root source. Consumers do not
    /// reconstruct ownership from filesystem ancestry, which would misclassify dependencies or
    /// nested package roots.
    #[must_use]
    pub fn is_root_package_source(&self, canonical_path: &str) -> bool {
        self.modules.iter().any(|module| {
            self.root_packages().contains(module.identity().package())
                && module
                    .sources()
                    .iter()
                    .any(|source| source.canonical_path() == canonical_path)
        })
    }

    #[must_use]
    pub fn has_syntax_errors(&self) -> bool {
        self.syntax.iter().any(SyntaxTree::has_errors)
    }

    /// Projects lexer and parser failures into the common source-diagnostic envelope.
    ///
    /// Diagnostics are ordered by source identity and normalized source position. This projection
    /// is performed only after a syntax-invalid snapshot is rejected; later phases never inspect
    /// lexer or parser error variants.
    #[must_use]
    pub fn syntax_diagnostics(&self) -> Box<[nocter_diagnostics::SourceDiagnostic]> {
        nocter_diagnostics::syntax_diagnostics(&self.syntax)
    }

    /// Consumes the discovery snapshot and retains its immutable normalized sources for
    /// presentation after a failed compiler session.
    #[must_use]
    pub fn into_sources(self) -> SourceMap {
        Arc::unwrap_or_clone(self.sources)
    }

    /// Borrows this immutable discovery snapshot as the sole declaration-lowering input.
    ///
    /// # Errors
    ///
    /// Returns an error while any loaded source has lexical or parse diagnostics, or when an
    /// incomplete snapshot lacks the toolchain profile selected by discovery. Callers retain this
    /// snapshot and can project syntax diagnostics through its source map and syntax trees.
    pub fn compile_input(&self) -> Result<&CompileUnitInput<'static>, CompileInputError> {
        if self.has_syntax_errors() {
            return Err(CompileInputError::SyntaxErrorsPresent);
        }
        self.analysis_input()
    }

    /// Borrows the discovered graph for an editor-only recovery attempt even when syntax
    /// diagnostics exist.
    ///
    /// The returned input must never be treated as a compilable program. Later phases may reject
    /// its explicit missing/error nodes; tooling may retain only phase-owned facts completed before
    /// that rejection while the original syntax diagnostics remain authoritative.
    ///
    /// # Errors
    ///
    /// Returns an error when the incomplete snapshot lacks the toolchain profile selected by
    /// discovery.
    pub fn analysis_input(&self) -> Result<&CompileUnitInput<'static>, CompileInputError> {
        if self.compile_input.toolchain().is_none() {
            return Err(CompileInputError::MissingToolchainProfile);
        }
        Ok(&self.compile_input)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileInputError {
    SyntaxErrorsPresent,
    MissingToolchainProfile,
}

impl fmt::Display for CompileInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SyntaxErrorsPresent => {
                formatter.write_str("discovered sources contain syntax errors")
            }
            Self::MissingToolchainProfile => {
                formatter.write_str("discovery snapshot has no resolved toolchain profile")
            }
        }
    }
}

impl std::error::Error for CompileInputError {}
