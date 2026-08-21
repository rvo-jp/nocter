use std::collections::BTreeMap;
use std::fmt;

use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode, ToolchainInput,
    UseResolutionInput,
};
use nocter_model::CompilationTarget;
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

#[derive(Debug)]
pub(crate) struct DiscoveredPackage {
    pub(crate) identity: PackageIdentity,
    pub(crate) display_name: Box<str>,
    pub(crate) mode: PackageMode,
    pub(crate) declaration: Option<(Box<str>, usize)>,
}

#[derive(Debug)]
pub struct DiscoveredUnit {
    pub(crate) target: CompilationTarget,
    pub(crate) sources: SourceMap,
    pub(crate) syntax: Vec<SyntaxTree>,
    pub(crate) packages: Vec<DiscoveredPackage>,
    pub(crate) modules: Vec<DiscoveredModule>,
    pub(crate) use_resolutions: Vec<UseResolutionInput>,
    pub(crate) toolchain: Option<ToolchainInput>,
}

impl DiscoveredUnit {
    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
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

    #[must_use]
    pub fn has_syntax_errors(&self) -> bool {
        self.syntax.iter().any(SyntaxTree::has_errors)
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
