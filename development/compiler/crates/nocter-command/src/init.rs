use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::ParsedInitCommand;

/// The source template selected for a newly initialized package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializedPackageKind {
    Executable,
    Library,
}

/// One package created without replacing any existing source file.
#[derive(Debug)]
pub struct InitCommandResult {
    root: PathBuf,
    name: Box<str>,
    kind: InitializedPackageKind,
}

impl InitCommandResult {
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn kind(&self) -> InitializedPackageKind {
        self.kind
    }

    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "Initialized {} package `{}` at {}\n",
            match self.kind {
                InitializedPackageKind::Executable => "executable",
                InitializedPackageKind::Library => "library",
            },
            self.name,
            self.root.display(),
        )
    }
}

/// Creates the two source files owned by `nocter init` as one rollback-capable transaction.
///
/// Existing files are checked before the first mutation and every created path is removed if a
/// later write fails. Unrelated content in an existing target directory is never removed.
///
/// # Errors
///
/// Returns a typed path, name, filesystem, or rollback failure without overwriting an existing
/// package declaration, root source, or generated test source.
pub fn execute_init(
    command: ParsedInitCommand,
    current_directory: impl AsRef<Path>,
) -> Result<InitCommandResult, InitCommandError> {
    let (directory, explicit_name, library) = command.into_parts();
    let root = resolve_target(current_directory.as_ref(), directory.as_deref())?;
    let name = match explicit_name {
        Some(name) => name,
        None => root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(Box::<str>::from)
            .ok_or_else(|| InitCommandError::MissingPackageName(root.clone()))?,
    };
    let protected = [root.join("index.nct"), root.join("tests/unit/index.nct")];
    for path in &protected {
        match fs::symlink_metadata(path) {
            Ok(_) => return Err(InitCommandError::ExistingSource(path.clone())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(InitCommandError::Filesystem {
                    operation: "inspect initialization target",
                    path: path.clone(),
                    error,
                });
            }
        }
    }

    let kind = if library {
        InitializedPackageKind::Library
    } else {
        InitializedPackageKind::Executable
    };
    let escaped_name = escape_string(&name);
    let root_source = root_template(&escaped_name, kind);
    let test_source = test_template(kind);

    let mut transaction = InitTransaction::new();
    if let Err(error) = transaction.create_directory_chain(&root) {
        return transaction.abort(error);
    }
    let tests = root.join("tests");
    if let Err(error) = transaction.create_directory(&tests) {
        return transaction.abort(error);
    }
    let unit = tests.join("unit");
    if let Err(error) = transaction.create_directory(&unit) {
        return transaction.abort(error);
    }
    for (path, contents) in [
        (&protected[0], root_source.as_bytes()),
        (&protected[1], test_source.as_bytes()),
    ] {
        if let Err(error) = transaction.write_new(path, contents) {
            return transaction.abort(error);
        }
    }
    transaction.commit();
    Ok(InitCommandResult { root, name, kind })
}

fn resolve_target(current: &Path, requested: Option<&Path>) -> Result<PathBuf, InitCommandError> {
    let current = fs::canonicalize(current).map_err(|error| InitCommandError::Filesystem {
        operation: "canonicalize current directory",
        path: current.into(),
        error,
    })?;
    let requested = requested.unwrap_or_else(|| Path::new("."));
    let target = if requested.is_absolute() {
        requested.into()
    } else {
        current.join(requested)
    };
    canonicalize_existing_prefix(&target)
}

fn canonicalize_existing_prefix(target: &Path) -> Result<PathBuf, InitCommandError> {
    let mut missing = Vec::new();
    let mut existing = target;
    loop {
        match fs::canonicalize(existing) {
            Ok(mut canonical) => {
                let metadata =
                    fs::metadata(&canonical).map_err(|error| InitCommandError::Filesystem {
                        operation: "inspect initialization directory",
                        path: canonical.clone(),
                        error,
                    })?;
                if !metadata.is_dir() {
                    return Err(InitCommandError::NotDirectory(canonical));
                }
                for component in missing.iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let name = existing
                    .file_name()
                    .ok_or_else(|| InitCommandError::NotDirectory(target.into()))?;
                missing.push(name.to_owned());
                existing = existing
                    .parent()
                    .ok_or_else(|| InitCommandError::NotDirectory(target.into()))?;
            }
            Err(error) => {
                return Err(InitCommandError::Filesystem {
                    operation: "canonicalize initialization directory",
                    path: existing.into(),
                    error,
                });
            }
        }
    }
}

fn package_directives(name: &str, kind: InitializedPackageKind) -> String {
    let executable = match kind {
        InitializedPackageKind::Executable => {
            format!("#executable: {{\n    name: \"{name}\",\n}}\n")
        }
        InitializedPackageKind::Library => String::new(),
    };
    format!(
        "#package: {{ name: \"{name}\", version: \"0.1.0\", }}\n{executable}#test: {{\n    name: \"unit\",\n    module: \"./tests/unit\",\n}}\n"
    )
}

fn root_template(name: &str, kind: InitializedPackageKind) -> String {
    let directives = package_directives(name, kind);
    let body = match kind {
        InitializedPackageKind::Executable => format!(
            "use std/io.print\n\nfunc main(): i32! {{\n    print(\"Hello from {name}\\n\")?\n    return 0\n}}\n"
        ),
        InitializedPackageKind::Library => {
            format!("pub func greeting(): &str {{\n    return \"Hello from {name}\"\n}}\n")
        }
    };
    format!("{directives}\n{body}")
}

fn test_template(kind: InitializedPackageKind) -> &'static str {
    match kind {
        InitializedPackageKind::Executable => "test package_initializes {\n    return\n}\n",
        InitializedPackageKind::Library => {
            "use /.greeting\n\ntest greeting_is_available {\n    let text = greeting()\n    return\n}\n"
        }
    }
}

fn escape_string(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\0' => escaped.push_str("\\0"),
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '$' => escaped.push_str("\\$"),
            character if character.is_control() => {
                let mut bytes = [0; 4];
                for byte in character.encode_utf8(&mut bytes).bytes() {
                    use std::fmt::Write as _;
                    write!(escaped, "\\x{byte:02X}").expect("writing to String cannot fail");
                }
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[derive(Default)]
struct InitTransaction {
    files: Vec<PathBuf>,
    directories: Vec<PathBuf>,
    committed: bool,
}

impl InitTransaction {
    fn new() -> Self {
        Self::default()
    }

    fn create_directory_chain(&mut self, directory: &Path) -> Result<(), InitCommandError> {
        let mut missing = Vec::new();
        let mut current = directory;
        while !current.exists() {
            missing.push(current.to_owned());
            current = current
                .parent()
                .ok_or_else(|| InitCommandError::NotDirectory(directory.into()))?;
        }
        if !current.is_dir() {
            return Err(InitCommandError::NotDirectory(current.into()));
        }
        for directory in missing.iter().rev() {
            self.create_directory(directory)?;
        }
        Ok(())
    }

    fn create_directory(&mut self, directory: &Path) -> Result<(), InitCommandError> {
        match fs::metadata(directory) {
            Ok(metadata) if metadata.is_dir() => return Ok(()),
            Ok(_) => return Err(InitCommandError::NotDirectory(directory.into())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(InitCommandError::Filesystem {
                    operation: "inspect initialization directory",
                    path: directory.into(),
                    error,
                });
            }
        }
        fs::create_dir(directory).map_err(|error| InitCommandError::Filesystem {
            operation: "create initialization directory",
            path: directory.into(),
            error,
        })?;
        self.directories.push(directory.into());
        Ok(())
    }

    fn write_new(&mut self, path: &Path, contents: &[u8]) -> Result<(), InitCommandError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| InitCommandError::Filesystem {
                operation: "create initialized source",
                path: path.into(),
                error,
            })?;
        self.files.push(path.into());
        write_and_sync(&mut file, path, contents)
    }

    fn abort(mut self, error: InitCommandError) -> Result<InitCommandResult, InitCommandError> {
        match self.rollback() {
            Ok(()) => Err(error),
            Err(cleanup) => Err(InitCommandError::Rollback {
                original: Box::new(error),
                cleanup,
            }),
        }
    }

    fn commit(mut self) {
        self.committed = true;
    }

    fn rollback(&mut self) -> Result<(), RollbackFailure> {
        let mut first = None;
        for path in self.files.iter().rev() {
            if let Err(error) = fs::remove_file(path)
                && error.kind() != io::ErrorKind::NotFound
                && first.is_none()
            {
                first = Some(RollbackFailure {
                    operation: "remove initialized source",
                    path: path.clone(),
                    error,
                });
            }
        }
        for path in self.directories.iter().rev() {
            if let Err(error) = fs::remove_dir(path)
                && error.kind() != io::ErrorKind::NotFound
                && first.is_none()
            {
                first = Some(RollbackFailure {
                    operation: "remove initialization directory",
                    path: path.clone(),
                    error,
                });
            }
        }
        self.files.clear();
        self.directories.clear();
        match first {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for InitTransaction {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.rollback();
        }
    }
}

fn write_and_sync(file: &mut File, path: &Path, contents: &[u8]) -> Result<(), InitCommandError> {
    file.write_all(contents)
        .and_then(|()| file.sync_all())
        .map_err(|error| InitCommandError::Filesystem {
            operation: "write initialized source",
            path: path.into(),
            error,
        })
}

#[derive(Debug)]
pub struct RollbackFailure {
    operation: &'static str,
    path: PathBuf,
    error: io::Error,
}

impl fmt::Display for RollbackFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {}: {}",
            self.operation,
            self.path.display(),
            self.error
        )
    }
}

impl std::error::Error for RollbackFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

#[derive(Debug)]
pub enum InitCommandError {
    MissingPackageName(PathBuf),
    ExistingSource(PathBuf),
    NotDirectory(PathBuf),
    Filesystem {
        operation: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    Rollback {
        original: Box<Self>,
        cleanup: RollbackFailure,
    },
}

impl fmt::Display for InitCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPackageName(path) => write!(
                formatter,
                "cannot derive a package name from {}; use --name",
                path.display()
            ),
            Self::ExistingSource(path) => write!(
                formatter,
                "initialization would overwrite existing source {}",
                path.display()
            ),
            Self::NotDirectory(path) => {
                write!(
                    formatter,
                    "initialization path is not a directory: {}",
                    path.display()
                )
            }
            Self::Filesystem {
                operation,
                path,
                error,
            } => write!(formatter, "{operation} {}: {error}", path.display()),
            Self::Rollback { original, cleanup } => {
                write!(
                    formatter,
                    "{original}; initialization rollback failed: {cleanup}"
                )
            }
        }
    }
}

impl std::error::Error for InitCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filesystem { error, .. } => Some(error),
            Self::Rollback { original, .. } => Some(original),
            Self::MissingPackageName(_) | Self::ExistingSource(_) | Self::NotDirectory(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::{ParsedCommand, parse_command_arguments};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn temporary_root(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "nocter-init-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn parsed(values: &[&str]) -> ParsedInitCommand {
        let command = parse_command_arguments(values.iter().map(OsString::from)).unwrap();
        let ParsedCommand::Init(command) = command else {
            panic!("expected init command")
        };
        command
    }

    #[test]
    fn executable_template_creates_only_the_owned_source_set() {
        let parent = temporary_root("executable");
        let result = execute_init(parsed(&["init", "hello"]), &parent).unwrap();

        assert_eq!(result.name(), "hello");
        assert_eq!(result.kind(), InitializedPackageKind::Executable);
        assert_eq!(
            result.root(),
            fs::canonicalize(parent.join("hello")).unwrap()
        );
        assert_eq!(
            fs::read_to_string(result.root().join("index.nct")).unwrap(),
            "#package: { name: \"hello\", version: \"0.1.0\", }\n#executable: {\n    name: \"hello\",\n}\n#test: {\n    name: \"unit\",\n    module: \"./tests/unit\",\n}\n\nuse std/io.print\n\nfunc main(): i32! {\n    print(\"Hello from hello\\n\")?\n    return 0\n}\n"
        );
        assert!(
            fs::read_to_string(result.root().join("index.nct"))
                .unwrap()
                .contains("func main(): i32!")
        );
        assert!(result.root().join("tests/unit/index.nct").is_file());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn library_template_escapes_the_explicit_name_and_exposes_root_api_to_tests() {
        let parent = temporary_root("library");
        let command = parse_command_arguments([
            OsString::from("init"),
            OsString::from("library"),
            OsString::from("--library"),
            OsString::from("--name"),
            OsString::from("quoted \"$name"),
        ])
        .unwrap();
        let ParsedCommand::Init(command) = command else {
            panic!("expected init command")
        };

        let result = execute_init(command, &parent).unwrap();

        assert_eq!(result.kind(), InitializedPackageKind::Library);
        assert!(
            fs::read_to_string(result.root().join("index.nct"))
                .unwrap()
                .contains("#package: { name: \"quoted \\\"\\$name\", version: \"0.1.0\", }")
        );
        assert!(
            fs::read_to_string(result.root().join("tests/unit/index.nct"))
                .unwrap()
                .starts_with("use /.greeting\n")
        );
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn existing_owned_source_blocks_every_mutation() {
        let parent = temporary_root("existing");
        let package = parent.join("package");
        fs::create_dir(&package).unwrap();
        fs::write(package.join("index.nct"), "user content\n").unwrap();

        let error = execute_init(parsed(&["init", "package"]), &parent).unwrap_err();

        let expected = fs::canonicalize(&package).unwrap().join("index.nct");
        assert!(matches!(error, InitCommandError::ExistingSource(ref path) if path == &expected));
        assert_eq!(
            fs::read_to_string(package.join("index.nct")).unwrap(),
            "user content\n"
        );
        assert!(package.join("index.nct").exists());
        assert!(!package.join("tests").exists());
        fs::remove_dir_all(parent).unwrap();
    }
}
