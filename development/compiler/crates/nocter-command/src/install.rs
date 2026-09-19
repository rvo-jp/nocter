use std::path::{Path, PathBuf};

use nocter_installation::{
    ArtifactInstallError, ArtifactInstallRequest, ArtifactInstallResult, CompilerInstallation,
    install_artifact,
};

use crate::ParsedInstallCommand;

/// Resolves command-authored paths and executes one verified local artifact installation.
///
/// # Errors
///
/// Returns the exact trust, archive, candidate, destination, recovery, or transaction failure.
pub fn execute_install(
    command: ParsedInstallCommand,
    current_directory: &Path,
    installation: &CompilerInstallation,
) -> Result<ArtifactInstallResult, ArtifactInstallError> {
    let (archive, digest, home) = command.into_parts();
    let archive = absolute_from(current_directory, archive);
    let destination = home.map_or_else(
        || installation.root().to_owned(),
        |home| absolute_from(current_directory, home),
    );
    let request = ArtifactInstallRequest::new(archive, &digest, destination, installation)?;
    install_artifact(&request)
}

fn absolute_from(current_directory: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        current_directory.join(path)
    }
}
