use std::collections::BTreeMap;
use std::fmt;

use nocter_compile_input::{
    CompileUnitInput, IncludeResolutionInput, ModuleIdentity, ModuleInput, ModuleSourceInput,
    ModuleSourceKind, PackageDeclarationInput, PackageInput, PackageMode,
    PackageTargetResolutionInput, ToolchainInput, UseResolutionInput,
};
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
    pub(crate) declaration: Option<(Box<str>, usize)>,
}

#[derive(Debug)]
pub struct DiscoveredUnit {
    pub(crate) target: CompilationTarget,
    pub(crate) source_overlay: SourceOverlay,
    pub(crate) sources: SourceMap,
    pub(crate) syntax: Vec<SyntaxTree>,
    pub(crate) packages: Vec<DiscoveredPackage>,
    pub(crate) root_packages: Vec<PackageIdentity>,
    pub(crate) modules: Vec<DiscoveredModule>,
    pub(crate) module_dependencies: Vec<DiscoveredModuleDependency>,
    pub(crate) include_resolutions: Vec<IncludeResolutionInput>,
    pub(crate) use_resolutions: Vec<UseResolutionInput>,
    pub(crate) package_target_resolutions: Vec<PackageTargetResolutionInput>,
    pub(crate) toolchain: Option<ToolchainInput>,
}

impl DiscoveredUnit {
    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        &self.source_overlay
    }

    #[must_use]
    pub const fn sources(&self) -> &SourceMap {
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
        &self.root_packages
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
    /// Package declarations and module sources share this ownership boundary. Consumers do not
    /// need to reconstruct it from filesystem ancestry, which would misclassify dependencies or
    /// nested package roots.
    #[must_use]
    pub fn is_root_package_source(&self, canonical_path: &str) -> bool {
        self.packages.iter().any(|package| {
            self.root_packages.contains(&package.identity)
                && package
                    .declaration
                    .as_ref()
                    .is_some_and(|(path, _)| path.as_ref() == canonical_path)
        }) || self.modules.iter().any(|module| {
            self.root_packages.contains(module.identity().package())
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
        self.sources
    }

    /// Borrows this immutable discovery snapshot as the sole declaration-lowering input.
    ///
    /// # Errors
    ///
    /// Returns an error while any loaded source has lexical or parse diagnostics, or when an
    /// incomplete snapshot lacks the toolchain profile selected by discovery. Callers retain this
    /// snapshot and can project syntax diagnostics through its source map and syntax trees.
    pub fn compile_input(&self) -> Result<CompileUnitInput<'_>, CompileInputError> {
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
    pub fn analysis_input(&self) -> Result<CompileUnitInput<'_>, CompileInputError> {
        let packages = self
            .packages
            .iter()
            .map(|package| {
                PackageInput::new(
                    package.identity.clone(),
                    package.display_name.clone(),
                    package.mode,
                    package.declaration.as_ref().map(|(path, syntax)| {
                        PackageDeclarationInput::new(path.clone(), &self.syntax[*syntax])
                    }),
                )
            })
            .collect();
        let modules = self
            .modules
            .iter()
            .map(|module| {
                ModuleInput::new(
                    module.identity.clone(),
                    module
                        .sources
                        .iter()
                        .map(|source| {
                            ModuleSourceInput::new(
                                source.canonical_path.clone(),
                                source.kind,
                                &self.syntax[source.syntax],
                            )
                        })
                        .collect(),
                )
            })
            .collect();
        let toolchain = self
            .toolchain
            .clone()
            .ok_or(CompileInputError::MissingToolchainProfile)?;
        Ok(CompileUnitInput::new(
            self.target,
            &self.sources,
            packages,
            modules,
            self.use_resolutions.clone(),
        )
        .with_include_resolutions(self.include_resolutions.clone())
        .with_root_packages(self.root_packages.clone())
        .with_package_target_resolutions(self.package_target_resolutions.clone())
        .with_toolchain(toolchain))
    }

    #[must_use]
    pub fn module_map(&self) -> BTreeMap<&ModuleIdentity, &DiscoveredModule> {
        self.modules
            .iter()
            .map(|module| (&module.identity, module))
            .collect()
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
