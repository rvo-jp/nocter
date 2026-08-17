use nocter_model::{CompilationTarget, PackageId};

use crate::capabilities::capabilities_for;
use crate::{
    ExecutableWriterIdentity, PrimitiveRegistry, TargetAbiIdentity, TargetBackendIdentity,
    TargetUnavailable,
};

/// All compiler-owned capabilities selected for one checked target and standard package.
///
/// The fields are private and there is no generic component constructor: a snapshot cannot pair a
/// recognized target with an arbitrary backend, ABI, or executable writer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolchainSnapshot {
    target: CompilationTarget,
    backend: TargetBackendIdentity,
    abi: TargetAbiIdentity,
    executable_writer: ExecutableWriterIdentity,
    standard_package: PackageId,
    primitives: PrimitiveRegistry,
}

impl ToolchainSnapshot {
    /// Selects the complete implementation for a recognized target.
    ///
    /// # Errors
    ///
    /// Returns [`TargetUnavailable`] unless every backend component is implemented by this
    /// compiler release.
    pub fn select(
        target: CompilationTarget,
        standard_package: PackageId,
        primitives: PrimitiveRegistry,
    ) -> Result<Self, TargetUnavailable> {
        let (backend, abi, executable_writer) = capabilities_for(target)?;
        Ok(Self {
            target,
            backend,
            abi,
            executable_writer,
            standard_package,
            primitives,
        })
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub const fn backend(&self) -> TargetBackendIdentity {
        self.backend
    }

    #[must_use]
    pub const fn abi(&self) -> TargetAbiIdentity {
        self.abi
    }

    #[must_use]
    pub const fn executable_writer(&self) -> ExecutableWriterIdentity {
        self.executable_writer
    }

    #[must_use]
    pub const fn standard_package(&self) -> PackageId {
        self.standard_package
    }

    #[must_use]
    pub const fn primitives(&self) -> &PrimitiveRegistry {
        &self.primitives
    }
}

#[cfg(test)]
mod tests {
    use nocter_declarations::{DeclarationArenaBuilder, DeclarationProgramBuilder};
    use nocter_model::{CompilationTarget, SymbolTable};

    use crate::{
        ExecutableWriterIdentity, PrimitiveBinding, PrimitiveRegistry, PrimitiveRole,
        TargetAbiIdentity, TargetBackendIdentity, ToolchainSnapshot,
    };

    fn complete_registry() -> PrimitiveRegistry {
        let mut declarations = DeclarationArenaBuilder::new();
        PrimitiveRegistry::new(
            PrimitiveRole::ALL
                .iter()
                .copied()
                .map(|role| PrimitiveBinding::new(role, declarations.reserve_callable())),
        )
        .unwrap()
    }

    fn standard_package() -> nocter_model::PackageId {
        let symbols = SymbolTable::from_spellings(["std"]);
        let name = symbols.get("std").unwrap();
        DeclarationProgramBuilder::new(CompilationTarget::Arm64Darwin, symbols)
            .add_package(name)
            .unwrap()
    }

    #[test]
    fn arm64_darwin_selects_one_coherent_capability_set() {
        let snapshot = ToolchainSnapshot::select(
            CompilationTarget::Arm64Darwin,
            standard_package(),
            complete_registry(),
        )
        .unwrap();
        assert_eq!(snapshot.backend(), TargetBackendIdentity::Arm64V1);
        assert_eq!(snapshot.abi(), TargetAbiIdentity::Arm64DarwinV1);
        assert_eq!(
            snapshot.executable_writer(),
            ExecutableWriterIdentity::Arm64MachOV1
        );
    }

    #[test]
    fn recognized_reserved_targets_cannot_acquire_a_snapshot() {
        for target in [
            CompilationTarget::X64Linux,
            CompilationTarget::Arm64Linux,
            CompilationTarget::X64Windows,
            CompilationTarget::Arm64Windows,
        ] {
            let error = ToolchainSnapshot::select(target, standard_package(), complete_registry())
                .unwrap_err();
            assert_eq!(error.target(), target);
        }
    }
}
