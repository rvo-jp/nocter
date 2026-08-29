use std::fmt;
use std::path::PathBuf;

use nocter_json::{Member, Value, write_value};
use nocter_package::{
    DependencySource, PackageResolutionError, PackageResolutionPolicy, PackageResolutionRequest,
    ResolvedPackageSelection, ResolvedPackageSnapshot,
};

use crate::compiler::{CommandCompiler, CommandPackageQueryError};
use crate::{CommandPackageContext, GraphOutputFormat, PreparedGraphCommand};

/// Source authority associated with one resolved package edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphDependencySource {
    Standard,
    Git,
    Archive,
    Path,
}

impl GraphDependencySource {
    const fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Git => "git",
            Self::Archive => "archive",
            Self::Path => "path",
        }
    }
}

/// One labeled exact edge in a projected package graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphDependency {
    alias: Box<str>,
    source: GraphDependencySource,
    lock: Option<Box<str>>,
    resolved: Box<str>,
}

impl GraphDependency {
    #[must_use]
    pub const fn alias(&self) -> &str {
        &self.alias
    }

    #[must_use]
    pub const fn source(&self) -> GraphDependencySource {
        self.source
    }

    #[must_use]
    pub fn lock(&self) -> Option<&str> {
        self.lock.as_deref()
    }

    #[must_use]
    pub const fn resolved(&self) -> &str {
        &self.resolved
    }
}

/// One exact package node projected independently from the syntax-owning graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPackage {
    id: Box<str>,
    name: Box<str>,
    version: Option<Box<str>>,
    root: Box<str>,
    dependencies: Box<[GraphDependency]>,
}

impl GraphPackage {
    #[must_use]
    pub const fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    #[must_use]
    pub const fn root(&self) -> &str {
        &self.root
    }

    #[must_use]
    pub const fn dependencies(&self) -> &[GraphDependency] {
        &self.dependencies
    }
}

/// Complete deterministic presentation input for one read-only exact package graph.
#[derive(Debug)]
pub struct GraphCommandResult {
    root: Box<str>,
    packages: Box<[GraphPackage]>,
    format: GraphOutputFormat,
}

impl GraphCommandResult {
    #[must_use]
    pub const fn root(&self) -> &str {
        &self.root
    }

    #[must_use]
    pub const fn packages(&self) -> &[GraphPackage] {
        &self.packages
    }

    #[must_use]
    pub const fn format(&self) -> GraphOutputFormat {
        self.format
    }

    #[must_use]
    pub fn render(&self) -> String {
        match self.format {
            GraphOutputFormat::Human => self.render_human(),
            GraphOutputFormat::Json => self.render_json(),
        }
    }

    fn render_human(&self) -> String {
        use std::fmt::Write as _;

        let mut output = format!("root: {}\n", self.root);
        for package in &self.packages {
            writeln!(output, "package {}", package.id).expect("writing to String cannot fail");
            writeln!(output, "  name: {}", package.name).expect("writing to String cannot fail");
            writeln!(
                output,
                "  version: {}",
                package.version.as_deref().unwrap_or("-")
            )
            .expect("writing to String cannot fail");
            writeln!(output, "  root: {}", package.root).expect("writing to String cannot fail");
            for dependency in &package.dependencies {
                writeln!(
                    output,
                    "  dependency {}: {} (source: {}, lock: {})",
                    dependency.alias,
                    dependency.resolved,
                    dependency.source.name(),
                    dependency.lock.as_deref().unwrap_or("-"),
                )
                .expect("writing to String cannot fail");
            }
        }
        output
    }

    fn render_json(&self) -> String {
        let packages = self
            .packages
            .iter()
            .map(|package| {
                object([
                    ("id", string(&package.id)),
                    ("name", string(&package.name)),
                    (
                        "version",
                        package.version.as_deref().map_or(Value::Null, string),
                    ),
                    ("root", string(&package.root)),
                    (
                        "dependencies",
                        Value::Array(
                            package
                                .dependencies
                                .iter()
                                .map(|dependency| {
                                    object([
                                        ("alias", string(&dependency.alias)),
                                        ("source", string(dependency.source.name())),
                                        (
                                            "lock",
                                            dependency.lock.as_deref().map_or(Value::Null, string),
                                        ),
                                        ("resolved", string(&dependency.resolved)),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect();
        let value = object([
            ("schema", string("nocter.package_graph")),
            ("version", Value::Number("1".into())),
            ("root", string(&self.root)),
            ("packages", Value::Array(packages)),
        ]);
        let mut output = String::new();
        write_value(&mut output, &value);
        output.push('\n');
        output
    }
}

/// Resolves and projects one exact package graph without granting lock or package-store mutation.
///
/// # Errors
///
/// Returns the read-only resolver's exact failure or rejects a canonical package root that cannot
/// be represented in the version-1 UTF-8 graph envelope.
pub fn execute_prepared_graph(
    command: PreparedGraphCommand,
    context: &CommandPackageContext,
) -> Result<GraphCommandResult, GraphCommandError> {
    let (input, resolution, format) = command.into_parts();
    let mut compiler = CommandCompiler::default();
    let selection = compiler
        .resolve_package_selection(PackageResolutionRequest::new(
            input.root(),
            context.nocter_home(),
            context.standard().clone(),
            PackageResolutionPolicy::new(resolution.locked(), resolution.offline()),
        ))
        .map_err(|error| match error {
            CommandPackageQueryError::Resolution(error) => GraphCommandError::Resolution(error),
            CommandPackageQueryError::Computation(error) => GraphCommandError::Computation(error),
        })?;
    project_graph(&selection, format)
}

fn project_graph(
    selection: &ResolvedPackageSelection,
    format: GraphOutputFormat,
) -> Result<GraphCommandResult, GraphCommandError> {
    let root = Box::<str>::from(selection.root().as_str());
    let packages = selection
        .graph()
        .packages()
        .iter()
        .map(project_package)
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(GraphCommandResult {
        root,
        packages,
        format,
    })
}

fn project_package(package: &ResolvedPackageSnapshot) -> Result<GraphPackage, GraphCommandError> {
    let declaration = package.declaration();
    let dependencies = package
        .dependencies()
        .iter()
        .map(|(alias, resolved)| {
            let source = declaration
                .and_then(|declaration| declaration.dependencies().get(alias))
                .map_or(
                    GraphDependencySource::Standard,
                    |dependency| match dependency.source() {
                        DependencySource::Git { .. } => GraphDependencySource::Git,
                        DependencySource::Archive { .. } => GraphDependencySource::Archive,
                        DependencySource::Path { .. } => GraphDependencySource::Path,
                    },
                );
            GraphDependency {
                alias: alias.clone(),
                source,
                lock: package
                    .locks()
                    .get(alias)
                    .map(nocter_package::ExactDependencyLock::literal),
                resolved: resolved.as_str().into(),
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let root = package
        .root()
        .to_str()
        .map(Box::<str>::from)
        .ok_or_else(|| GraphCommandError::NonUnicodeRoot(package.root().into()))?;
    Ok(GraphPackage {
        id: package.identity().as_str().into(),
        name: package.display_name().into(),
        version: declaration
            .map(nocter_package::PackageDeclaration::version)
            .map(|version| Box::<str>::from(version.value())),
        root,
        dependencies,
    })
}

fn string(value: &str) -> Value {
    Value::String(value.into())
}

fn object<const N: usize>(members: [(&str, Value); N]) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| Member {
                name: name.into(),
                value,
            })
            .collect(),
    )
}

#[derive(Debug)]
pub enum GraphCommandError {
    Resolution(PackageResolutionError),
    Computation(nocter_compiler_computation::CompilerComputationError),
    NonUnicodeRoot(PathBuf),
}

impl GraphCommandError {
    #[must_use]
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::Resolution(PackageResolutionError::Filesystem { .. })
            | Self::NonUnicodeRoot(_) => "E0702",
            Self::Computation(_) | Self::Resolution(_) => "E0800",
        }
    }
}

impl fmt::Display for GraphCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolution(error) => error.fmt(formatter),
            Self::Computation(error) => write!(formatter, "package query failed: {error}"),
            Self::NonUnicodeRoot(path) => write!(
                formatter,
                "package root is not valid Unicode and cannot be represented in graph output: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for GraphCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resolution(error) => Some(error),
            Self::Computation(error) => Some(error),
            Self::NonUnicodeRoot(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_model::PackageIdentity;
    use nocter_package::StandardPackage;

    use super::*;
    use crate::{ParsedCommand, parse_command_arguments};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn temporary_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "nocter-graph-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn write_package(root: &Path, root_source: &str) {
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join("index.nct"),
            format!("//! Package root.\n{root_source}"),
        )
        .unwrap();
    }

    #[test]
    fn projects_the_exact_read_only_graph_in_identity_order() {
        let directory = temporary_root();
        let standard = directory.join("std");
        let dependency = directory.join("dependency");
        let package = directory.join("package");
        let home = directory.join("home");
        fs::create_dir(&home).unwrap();
        write_package(
            &standard,
            "#package: { name: \"std\", version: \"0.14.0\", }\n",
        );
        write_package(
            &dependency,
            "#package: { name: \"local\", version: \"0.0.0\", }\n",
        );
        write_package(
            &package,
            "#package: { name: \"application\", version: \"1.2.3\", }\n#dependencies: { local: { path: \"../dependency\" } }\n",
        );
        let command = parse_command_arguments([
            OsString::from("graph"),
            OsString::from("--root"),
            package.as_os_str().to_owned(),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap();
        let ParsedCommand::Graph(command) = command else {
            panic!("expected graph command")
        };
        let command = command.prepare(&directory).unwrap();
        let context = CommandPackageContext::new(
            &home,
            StandardPackage::new(PackageIdentity::new("toolchain-std-v0.14.0"), &standard),
        );

        let result = execute_prepared_graph(command, &context).unwrap();

        assert_eq!(result.packages().len(), 3);
        let root = result
            .packages()
            .iter()
            .find(|candidate| candidate.id() == result.root())
            .unwrap();
        assert_eq!(root.name(), "application");
        assert_eq!(root.version(), Some("1.2.3"));
        assert_eq!(root.dependencies().len(), 2);
        assert!(root.dependencies().iter().any(|dependency| {
            dependency.alias() == "local"
                && dependency.source() == GraphDependencySource::Path
                && dependency.lock().is_none()
        }));
        assert!(root.dependencies().iter().any(|dependency| {
            dependency.alias() == "std"
                && dependency.source() == GraphDependencySource::Standard
                && dependency.lock().is_none()
        }));
        let json = result.render();
        assert!(json.starts_with("{\"schema\":\"nocter.package_graph\",\"version\":1,"));
        assert!(json.ends_with('\n'));
        assert!(matches!(
            nocter_json::parse(json.trim_end()).unwrap(),
            nocter_json::Value::Object(_)
        ));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_remote_lock_is_reported_without_creating_package_state() {
        let directory = temporary_root();
        let standard = directory.join("std");
        let package = directory.join("package");
        let home = directory.join("home");
        fs::create_dir(&home).unwrap();
        write_package(
            &standard,
            "#package: { name: \"std\", version: \"0.0.0\", }\n",
        );
        write_package(
            &package,
            "#package: { name: \"app\", version: \"0.0.0\", }\n#dependencies: { remote: { archive: \"https://example.test/archive.tar.gz\" } }\n",
        );
        let command = parse_command_arguments([
            OsString::from("graph"),
            OsString::from("--root"),
            package.as_os_str().to_owned(),
        ])
        .unwrap();
        let ParsedCommand::Graph(command) = command else {
            panic!("expected graph command")
        };
        let command = command.prepare(&directory).unwrap();
        let context = CommandPackageContext::new(
            &home,
            StandardPackage::new(PackageIdentity::new("toolchain-std"), &standard),
        );

        let error = execute_prepared_graph(command, &context).unwrap_err();

        assert!(matches!(
            error,
            GraphCommandError::Resolution(PackageResolutionError::LockRequired { .. })
        ));
        assert!(!package.join(".nocter").exists());
        assert!(
            !fs::read_to_string(package.join("index.nct"))
                .unwrap()
                .contains("#lock")
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
