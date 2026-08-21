use std::fmt;
use std::path::Path;

use nocter_model::CompilationTarget;
use nocter_package::StandardPackage;

use crate::{InstallationManifest, NocterHome, NocterHomeOrigin};

/// One physically validated Nocter home that is compatible with the running compiler.
///
/// The current native-only compiler requires one identity for the compiler host, installation
/// host, and default compilation target. Constructing this type closes that relationship once for
/// every command, including commands that do not compile source.
#[derive(Clone, Debug)]
pub struct CompilerInstallation {
    home: NocterHome,
}

impl CompilerInstallation {
    pub(crate) fn validate(
        home: NocterHome,
        compiler_host: &str,
    ) -> Result<Self, InstallationCompatibilityError> {
        let installation_host = home.manifest().host();
        if installation_host != compiler_host {
            return Err(InstallationCompatibilityError::HostMismatch {
                compiler: compiler_host.into(),
                installation: installation_host.into(),
            });
        }
        let default_target = home.manifest().default_target();
        if default_target.name() != installation_host {
            return Err(
                InstallationCompatibilityError::NativeDefaultTargetMismatch {
                    host: installation_host.into(),
                    target: default_target,
                },
            );
        }
        Ok(Self { home })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        self.home.root()
    }

    #[must_use]
    pub const fn origin(&self) -> NocterHomeOrigin {
        self.home.origin()
    }

    #[must_use]
    pub const fn release(&self) -> &str {
        self.home.release()
    }

    #[must_use]
    pub const fn manifest(&self) -> &InstallationManifest {
        self.home.manifest()
    }

    #[must_use]
    pub fn standard_package(&self) -> StandardPackage {
        self.home.standard_package()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum InstallationCompatibilityError {
    HostMismatch {
        compiler: Box<str>,
        installation: Box<str>,
    },
    NativeDefaultTargetMismatch {
        host: Box<str>,
        target: CompilationTarget,
    },
}

impl fmt::Display for InstallationCompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HostMismatch {
                compiler,
                installation,
            } => write!(
                formatter,
                "Nocter home host `{installation}` does not match compiler host `{compiler}`"
            ),
            Self::NativeDefaultTargetMismatch { host, target } => write!(
                formatter,
                "Nocter home default target `{target}` does not match native compiler host `{host}`"
            ),
        }
    }
}

impl std::error::Error for InstallationCompatibilityError {}
