use std::fmt;

use nocter_model::CompilationTarget;
use nocter_runtime_contract::RuntimeAbiIdentity;

/// The backend implementation selected by an immutable toolchain snapshot.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TargetBackendIdentity {
    Arm64V1,
}

/// The executable writer selected by an immutable toolchain snapshot.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExecutableWriterIdentity {
    Arm64MachOV1,
}

/// A recognized target for which this compiler has no complete target-program capability.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct TargetUnavailable {
    target: CompilationTarget,
}

impl TargetUnavailable {
    pub(crate) const fn new(target: CompilationTarget) -> Self {
        Self { target }
    }

    #[must_use]
    pub const fn target(self) -> CompilationTarget {
        self.target
    }
}

impl fmt::Debug for TargetUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TargetUnavailable")
            .field("target", &self.target)
            .finish()
    }
}

impl fmt::Display for TargetUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "target {} is recognized but not implemented",
            self.target
        )
    }
}

impl std::error::Error for TargetUnavailable {}

pub(crate) const fn capabilities_for(
    target: CompilationTarget,
) -> Result<
    (
        TargetBackendIdentity,
        RuntimeAbiIdentity,
        ExecutableWriterIdentity,
    ),
    TargetUnavailable,
> {
    match RuntimeAbiIdentity::for_target(target) {
        Some(abi @ RuntimeAbiIdentity::Arm64DarwinV1) => Ok((
            TargetBackendIdentity::Arm64V1,
            abi,
            ExecutableWriterIdentity::Arm64MachOV1,
        )),
        None => Err(TargetUnavailable::new(target)),
    }
}
