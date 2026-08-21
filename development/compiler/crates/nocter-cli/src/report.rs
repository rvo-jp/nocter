use std::fmt;
use std::path::{Path, PathBuf};

use nocter_installation::{CompilerInstallation, NocterHomeOrigin};

#[derive(Debug)]
struct InstallationIdentity {
    release: Box<str>,
    host: Box<str>,
    default_target: &'static str,
}

impl InstallationIdentity {
    fn from_installation(installation: &CompilerInstallation) -> Self {
        Self {
            release: installation.release().into(),
            host: installation.manifest().host().into(),
            default_target: installation.manifest().default_target().name(),
        }
    }
}

impl fmt::Display for InstallationIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "release: {}", self.release)?;
        writeln!(formatter, "host: {}", self.host)?;
        writeln!(formatter, "default target: {}", self.default_target)
    }
}

/// Successful `nocter --version` report from one validated compiler installation.
#[derive(Debug)]
pub struct VersionReport {
    identity: InstallationIdentity,
}

impl VersionReport {
    pub(crate) fn from_installation(installation: &CompilerInstallation) -> Self {
        Self {
            identity: InstallationIdentity::from_installation(installation),
        }
    }

    #[must_use]
    pub fn render(&self) -> String {
        format!("Nocter\n{}", self.identity)
    }
}

/// Successful `nocter doctor` report from one validated compiler installation.
#[derive(Debug)]
pub struct DoctorReport {
    identity: InstallationIdentity,
    root: PathBuf,
    origin: NocterHomeOrigin,
}

impl DoctorReport {
    pub(crate) fn from_installation(installation: &CompilerInstallation) -> Self {
        Self {
            identity: InstallationIdentity::from_installation(installation),
            root: installation.root().into(),
            origin: installation.origin(),
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "Nocter home is valid\nroot: {}\nselected by: {}\n{}",
            self.root.display(),
            origin_name(self.origin),
            self.identity,
        )
    }
}

const fn origin_name(origin: NocterHomeOrigin) -> &'static str {
    match origin {
        NocterHomeOrigin::Configured => "NOCTER_HOME",
        NocterHomeOrigin::Executable => "compiler executable",
    }
}
