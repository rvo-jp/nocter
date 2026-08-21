use std::io;
use std::path::{Path, PathBuf};

use nocter_installation::CompilerInstallation;
use nocter_language_server::{
    LanguageServerEnvironment, LanguageServerExit, LanguageServerRunError, LanguageServerToolchain,
    run_language_server,
};

/// Validated process facts retained until the binary enters its dedicated protocol loop.
#[derive(Clone, Debug)]
pub struct LanguageServerLaunch {
    current_directory: PathBuf,
    installation: CompilerInstallation,
}

impl LanguageServerLaunch {
    #[must_use]
    pub(crate) fn new(
        current_directory: impl Into<PathBuf>,
        installation: CompilerInstallation,
    ) -> Self {
        Self {
            current_directory: current_directory.into(),
            installation,
        }
    }

    #[must_use]
    pub fn current_directory(&self) -> &Path {
        &self.current_directory
    }

    #[must_use]
    pub const fn installation(&self) -> &CompilerInstallation {
        &self.installation
    }
}

/// Runs one validated language-server launch on process stdio.
///
/// Protocol messages are the only bytes written to stdout. Recoverable notification failures are
/// written to stderr.
///
/// # Errors
///
/// Returns a framing or protocol-output failure.
pub fn run_language_server_stdio(
    launch: &LanguageServerLaunch,
) -> Result<LanguageServerExit, LanguageServerRunError> {
    let input = io::stdin();
    let output = io::stdout();
    let environment = LanguageServerEnvironment::new(
        launch.current_directory(),
        LanguageServerToolchain::new(
            launch.installation().manifest().default_target(),
            launch.installation().root(),
            launch.installation().standard_package(),
        ),
    );
    run_language_server(
        input.lock(),
        output.lock(),
        launch.installation.release(),
        environment,
        |issue| eprintln!("language server: {issue}"),
    )
}
