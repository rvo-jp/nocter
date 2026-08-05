use super::compile_options::{BuildCommand, CompileInput, SourceCommand};
use super::errors::{exit_for_diagnostics, write_human_diagnostics};
use super::json::write_diagnostics_json;
use super::package_plan::{check_targets, selected_executables};
use super::pipeline::{
    build_file_to_path_with_target, build_package_executable_to_path_with_target,
    check_file_with_target, check_package_executable_with_target, check_package_module_with_target,
};
use super::run::{run_file, run_package_file};
use crate::diagnostics::Diagnostic;
use crate::package::{PackageGraph, PackageGraphOptions, load_package_graph};
use std::path::Path;
use std::process::ExitCode;

const PACKAGE_COMMAND_ERROR: &str = "E0800";

pub(super) fn run_check_command(command: &SourceCommand) -> ExitCode {
    match &command.input {
        CompileInput::File { file } => {
            write_check_output(check_file_with_target(file, &command.target))
        }
        CompileInput::Package { root } => run_package_check(command, root),
    }
}

pub(super) fn run_fetch_command(command: &SourceCommand) -> ExitCode {
    let CompileInput::Package { root } = &command.input else {
        return write_package_command_error("fetch requires package mode");
    };
    let load = load_package_graph(root, graph_options(command));
    if load.diagnostics.is_empty() {
        ExitCode::SUCCESS
    } else {
        write_package_graph_errors(&load.diagnostics)
    }
}

pub(super) fn run_check_json_command(command: &SourceCommand) -> ExitCode {
    match &command.input {
        CompileInput::File { file } => super::json::run_check_json(file, &command.target),
        CompileInput::Package { root } => {
            let load = load_package_graph(root, graph_options(command));
            let root_path = root.join("nocter.nct");
            if !load.diagnostics.is_empty() {
                let status = exit_for_diagnostics(&load.diagnostics, ExitCode::FAILURE);
                return write_diagnostics_json(
                    "check",
                    Some(command.target.clone()),
                    Some(root_path.to_string_lossy().into_owned()),
                    canonical_string(&root_path),
                    load.diagnostics,
                    status,
                );
            }
            let graph = load.graph.expect("successful package graph load");
            let package = graph.root_package();
            let diagnostics = collect_package_check_diagnostics(command, &graph);
            let status = if diagnostics.is_empty() {
                ExitCode::SUCCESS
            } else {
                exit_for_diagnostics(&diagnostics, ExitCode::FAILURE)
            };
            write_diagnostics_json(
                "check",
                Some(command.target.clone()),
                Some(package.package_file_path().to_string_lossy().into_owned()),
                canonical_string(package.package_file_path()),
                diagnostics,
                status,
            )
        }
    }
}

pub(super) fn run_build_command(command: &BuildCommand) -> ExitCode {
    match &command.source.input {
        CompileInput::File { file } => {
            let output = match command.output.as_deref() {
                Some(path) => build_file_to_path_with_target(file, path, &command.source.target),
                None => super::pipeline::build_file_with_target(file, &command.source.target),
            };
            write_build_output(output)
        }
        CompileInput::Package { root } => run_package_build(command, root),
    }
}

pub(super) fn run_run_command(command: &SourceCommand) -> ExitCode {
    match &command.input {
        CompileInput::File { file } => run_file(file, &command.target),
        CompileInput::Package { root } => {
            let load = load_package_graph(root, graph_options(command));
            if !load.diagnostics.is_empty() {
                return write_package_graph_errors(&load.diagnostics);
            }
            let graph = load.graph.expect("successful package graph load");
            let package = graph.root_package();
            let selected = match selected_executables(package, command.executable.as_deref()) {
                Ok(selected) if selected.len() == 1 => selected[0],
                Ok(selected) if selected.is_empty() => {
                    return write_package_command_error(format!(
                        "package `{}` declares no executable target",
                        package.display_name()
                    ));
                }
                Ok(_) => {
                    return write_package_command_error(format!(
                        "package `{}` declares multiple executables; use `--executable <name>`",
                        package.display_name()
                    ));
                }
                Err(message) => return write_package_command_error(message),
            };
            run_package_file(selected.entry().source_path(), &graph, &command.target)
        }
    }
}

fn run_package_check(command: &SourceCommand, root: &Path) -> ExitCode {
    let load = load_package_graph(root, graph_options(command));
    if !load.diagnostics.is_empty() {
        return write_package_graph_errors(&load.diagnostics);
    }
    let graph = load.graph.expect("successful package graph load");
    let diagnostics = collect_package_check_diagnostics(command, &graph);
    if diagnostics.is_empty() {
        ExitCode::SUCCESS
    } else {
        write_human_diagnostics(&diagnostics, None, ExitCode::FAILURE)
    }
}

fn collect_package_check_diagnostics(
    command: &SourceCommand,
    graph: &PackageGraph,
) -> Vec<Diagnostic> {
    let targets = match check_targets(graph.root_package(), command.executable.as_deref()) {
        Ok(targets) => targets,
        Err(message) => return vec![package_command_diagnostic(message)],
    };
    let mut diagnostics = Vec::new();
    for target in targets {
        let output = if target.executable {
            check_package_executable_with_target(
                target.module.source_path(),
                graph,
                &command.target,
            )
        } else {
            check_package_module_with_target(target.module.source_path(), graph, &command.target)
        };
        diagnostics.extend(output.diagnostics);
    }
    diagnostics
}

fn run_package_build(command: &BuildCommand, root: &Path) -> ExitCode {
    let load = load_package_graph(root, graph_options(&command.source));
    if !load.diagnostics.is_empty() {
        return write_package_graph_errors(&load.diagnostics);
    }
    let graph = load.graph.expect("successful package graph load");
    let package = graph.root_package();
    let selected = match selected_executables(package, command.source.executable.as_deref()) {
        Ok(selected) => selected,
        Err(message) => return write_package_command_error(message),
    };
    if selected.is_empty() {
        return write_package_command_error(format!(
            "package `{}` declares no executable target",
            package.display_name()
        ));
    }
    if command.output.is_some() && selected.len() != 1 {
        return write_package_command_error(
            "`-o` requires exactly one executable selected with `--executable`",
        );
    }
    let mut failed = false;
    for executable in selected {
        let output_path = command
            .output
            .clone()
            .unwrap_or_else(|| package.root().join(executable.name()));
        let output = build_package_executable_to_path_with_target(
            executable.entry().source_path(),
            &graph,
            &output_path,
            &command.source.target,
        );
        if !output.is_ok() {
            failed = true;
            let exit = exit_for_diagnostics(&output.diagnostics, ExitCode::FAILURE);
            let _ = write_human_diagnostics(&output.diagnostics, Some(&output.sources), exit);
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn graph_options(command: &SourceCommand) -> PackageGraphOptions {
    PackageGraphOptions {
        locked: command.locked,
        offline: command.offline,
    }
}

fn write_check_output(output: super::pipeline::CheckOutput) -> ExitCode {
    if output.is_ok() {
        ExitCode::SUCCESS
    } else {
        let exit = exit_for_diagnostics(&output.diagnostics, ExitCode::FAILURE);
        write_human_diagnostics(&output.diagnostics, Some(&output.sources), exit)
    }
}

fn write_build_output(output: super::pipeline::BuildOutput) -> ExitCode {
    if output.is_ok() {
        ExitCode::SUCCESS
    } else {
        let exit = exit_for_diagnostics(&output.diagnostics, ExitCode::FAILURE);
        write_human_diagnostics(&output.diagnostics, Some(&output.sources), exit)
    }
}

fn write_package_graph_errors(diagnostics: &[Diagnostic]) -> ExitCode {
    let exit = exit_for_diagnostics(diagnostics, ExitCode::FAILURE);
    write_human_diagnostics(diagnostics, None, exit)
}

fn package_command_diagnostic(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(PACKAGE_COMMAND_ERROR, message)
}

fn write_package_command_error(message: impl Into<String>) -> ExitCode {
    write_package_command_diagnostic(package_command_diagnostic(message))
}

fn write_package_command_diagnostic(diagnostic: Diagnostic) -> ExitCode {
    write_human_diagnostics(&[diagnostic], None, ExitCode::FAILURE)
}

fn canonical_string(path: &Path) -> Option<String> {
    path.canonicalize()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}
