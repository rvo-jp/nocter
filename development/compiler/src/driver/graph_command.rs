use super::errors::write_human_diagnostics;
use crate::package::{DependencySource, PackageGraphOptions, load_package_graph};
use serde::Serialize;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphCommand {
    root: PathBuf,
    locked: bool,
    offline: bool,
    json: bool,
}

pub(super) fn parse_graph_command(args: &[OsString]) -> Result<GraphCommand, String> {
    let mut command = GraphCommand {
        root: PathBuf::from("."),
        locked: false,
        offline: false,
        json: false,
    };
    let mut index = 1;
    while index < args.len() {
        match args[index].to_string_lossy().as_ref() {
            "--root" => {
                command.root = PathBuf::from(
                    args.get(index + 1)
                        .ok_or("expected package root after `--root`")?,
                );
                index += 2;
            }
            "--locked" => {
                command.locked = true;
                index += 1;
            }
            "--offline" => {
                command.offline = true;
                index += 1;
            }
            "--format" => {
                if args.get(index + 1).is_none_or(|value| value != "json") {
                    return Err("expected `--format json`".to_string());
                }
                command.json = true;
                index += 2;
            }
            value => return Err(format!("unexpected argument `{value}`")),
        }
    }
    Ok(command)
}

#[derive(Serialize)]
struct GraphOutput {
    format: u8,
    root: String,
    packages: Vec<PackageOutput>,
}

#[derive(Serialize)]
struct PackageOutput {
    id: String,
    name: String,
    version: Option<String>,
    root: String,
    dependencies: Vec<DependencyOutput>,
}

#[derive(Serialize)]
struct DependencyOutput {
    name: String,
    source: String,
    lock: Option<String>,
    package: String,
}

pub(super) fn run_graph_command(command: &GraphCommand) -> ExitCode {
    let load = load_package_graph(
        &command.root,
        PackageGraphOptions {
            locked: command.locked,
            offline: command.offline,
        },
    );
    if !load.diagnostics.is_empty() {
        return write_human_diagnostics(&load.diagnostics, None, ExitCode::FAILURE);
    }
    let graph = load.graph.expect("successful graph load");
    let mut packages = graph
        .packages()
        .map(|package| {
            let mut dependencies = package
                .dependencies()
                .iter()
                .map(|dependency| {
                    let target = graph
                        .dependency(package.id(), dependency.name())
                        .expect("loaded dependency");
                    DependencyOutput {
                        name: dependency.name().to_string(),
                        source: match dependency.source() {
                            DependencySource::Git { .. } => "git",
                            DependencySource::Archive { .. } => "archive",
                            DependencySource::Path { .. } => "path",
                        }
                        .to_string(),
                        lock: package.lock(dependency.name()).map(|lock| lock.display()),
                        package: target.id().as_str().to_string(),
                    }
                })
                .collect::<Vec<_>>();
            dependencies.sort_by(|left, right| left.name.cmp(&right.name));
            PackageOutput {
                id: package.id().as_str().to_string(),
                name: package.display_name().to_string(),
                version: package.version().map(str::to_string),
                root: package.root().display().to_string(),
                dependencies,
            }
        })
        .collect::<Vec<_>>();
    packages.sort_by(|left, right| left.id.cmp(&right.id));
    let output = GraphOutput {
        format: 1,
        root: graph.root_package().id().as_str().to_string(),
        packages,
    };
    if command.json {
        println!(
            "{}",
            serde_json::to_string(&output).expect("graph JSON serialization")
        );
    } else {
        println!(
            "{} {}",
            graph.root_package().display_name(),
            graph.root_package().id().as_str()
        );
        for package in &output.packages {
            for dependency in &package.dependencies {
                println!(
                    "{} --{}--> {}",
                    package.id, dependency.name, dependency.package
                );
            }
        }
    }
    ExitCode::SUCCESS
}
