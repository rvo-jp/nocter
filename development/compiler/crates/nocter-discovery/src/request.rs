use std::path::PathBuf;

use nocter_compile_input::{ModuleIdentity, ToolchainInput};
use nocter_filesystem::SourceOverlay;
use nocter_model::CompilationTarget;
use nocter_package::ResolvedPackageGraph;

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

/// Closed package graph and initial directory modules selected for one compile unit.
#[derive(Debug)]
pub struct DiscoveryRequest {
    target: CompilationTarget,
    layout: DiscoveryLayout,
    toolchain: ToolchainInput,
}

impl DiscoveryRequest {
    #[must_use]
    pub fn declared(
        target: CompilationTarget,
        packages: ResolvedPackageGraph,
        roots: Vec<ModuleIdentity>,
        toolchain: ToolchainInput,
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
        toolchain: ToolchainInput,
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
        toolchain: ToolchainInput,
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
    pub const fn toolchain(&self) -> &ToolchainInput {
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

    pub(crate) fn into_parts(self) -> (CompilationTarget, DiscoveryLayout, ToolchainInput) {
        (self.target, self.layout, self.toolchain)
    }
}
