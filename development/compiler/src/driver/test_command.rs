use super::errors::{temporary_executable_diagnostic, write_human_diagnostics};
use super::package_plan::selected_tests;
use super::pipeline::build_package_executable_to_path_with_target;
use super::temporary_executable::TemporaryExecutable;
use super::test_options::{TestCommand, TestOutputFormat};
use super::test_report::{TestOutcome, TestReport, TestTargetReport, exit_code};
use crate::diagnostics::Diagnostic;
use crate::package::{PackageGraphOptions, TestTarget, load_package_graph};
use crate::source::SourceMap;
use std::io::{self, Write};
use std::process::{Command, ExitCode, ExitStatus};

pub(super) fn run_test_command(command: &TestCommand) -> ExitCode {
    let load = load_package_graph(
        &command.root,
        PackageGraphOptions {
            locked: command.locked,
            offline: command.offline,
        },
    );
    if !load.diagnostics.is_empty() {
        return write_initial_failure(command, load.diagnostics);
    }
    let graph = load.graph.expect("successful package graph load");
    let package = graph.root_package();
    let targets = match selected_tests(package, command.selected.as_deref()) {
        Ok(targets) if !targets.is_empty() => targets,
        Ok(_) => {
            return write_initial_failure(
                command,
                vec![Diagnostic::error(
                    "E0800",
                    format!(
                        "package `{}` declares no test targets",
                        package.display_name()
                    ),
                )],
            );
        }
        Err(message) => {
            return write_initial_failure(command, vec![Diagnostic::error("E0800", message)]);
        }
    };

    let mut executions = Vec::with_capacity(targets.len());
    for target in targets {
        executions.push(execute_target(command, &graph, target));
    }
    let (reports, sources): (Vec<_>, Vec<_>) = executions
        .into_iter()
        .map(|execution| (execution.report, execution.sources))
        .unzip();
    let report = TestReport::new(
        package.display_name().to_string(),
        command.target.clone(),
        Vec::new(),
        reports,
    );
    match command.format {
        TestOutputFormat::Human => write_human_report(&report, &sources),
        TestOutputFormat::Json => write_json_report(&report),
    }
}

struct TestExecution {
    report: TestTargetReport,
    sources: Option<SourceMap>,
}

fn execute_target(
    command: &TestCommand,
    graph: &crate::package::PackageGraph,
    target: &TestTarget,
) -> TestExecution {
    let artifact = match TemporaryExecutable::new("test") {
        Ok(artifact) => artifact,
        Err(error) => {
            return TestExecution {
                report: TestTargetReport {
                    name: target.name().to_string(),
                    outcome: TestOutcome::RunnerFailed,
                    exit_code: None,
                    signal: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    diagnostics: vec![temporary_executable_diagnostic(format!(
                        "failed to prepare test artifact: {error}"
                    ))],
                },
                sources: None,
            };
        }
    };
    let output = build_package_executable_to_path_with_target(
        target.entry().source_path(),
        graph,
        artifact.path(),
        &command.target,
    );
    if !output.is_ok() {
        return TestExecution {
            report: TestTargetReport {
                name: target.name().to_string(),
                outcome: TestOutcome::CompileFailed,
                exit_code: None,
                signal: None,
                stdout: String::new(),
                stderr: String::new(),
                diagnostics: output.diagnostics,
            },
            sources: Some(output.sources),
        };
    }
    match Command::new(artifact.path())
        .current_dir(graph.root_package().root())
        .output()
    {
        Ok(output) => TestExecution {
            report: TestTargetReport {
                name: target.name().to_string(),
                outcome: if output.status.success() {
                    TestOutcome::Passed
                } else {
                    TestOutcome::Failed
                },
                exit_code: output.status.code(),
                signal: signal(&output.status),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                diagnostics: Vec::new(),
            },
            sources: None,
        },
        Err(error) => TestExecution {
            report: TestTargetReport {
                name: target.name().to_string(),
                outcome: TestOutcome::RunnerFailed,
                exit_code: None,
                signal: None,
                stdout: String::new(),
                stderr: String::new(),
                diagnostics: vec![temporary_executable_diagnostic(format!(
                    "failed to run test `{}`: {error}",
                    target.name()
                ))],
            },
            sources: None,
        },
    }
}

fn write_initial_failure(command: &TestCommand, diagnostics: Vec<Diagnostic>) -> ExitCode {
    match command.format {
        TestOutputFormat::Human => write_human_diagnostics(&diagnostics, None, ExitCode::FAILURE),
        TestOutputFormat::Json => write_json_report(&TestReport::new(
            command.root.to_string_lossy().into_owned(),
            command.target.clone(),
            diagnostics,
            Vec::new(),
        )),
    }
}

fn write_human_report(report: &TestReport, sources: &[Option<SourceMap>]) -> ExitCode {
    for (test, sources) in report.tests().iter().zip(sources) {
        println!(
            "test {} ... {}",
            test.name,
            if test.outcome == TestOutcome::Passed {
                "ok"
            } else {
                "FAILED"
            }
        );
        if test.outcome != TestOutcome::Passed {
            let _ = io::stdout().write_all(test.stdout.as_bytes());
            let _ = io::stderr().write_all(test.stderr.as_bytes());
            if !test.diagnostics.is_empty() {
                let _ =
                    write_human_diagnostics(&test.diagnostics, sources.as_ref(), ExitCode::FAILURE);
            }
        }
    }
    println!();
    println!(
        "test result: {}. {} passed; {} failed",
        if report.is_ok() { "ok" } else { "FAILED" },
        report.summary().passed,
        report.summary().failed
    );
    exit_code(report)
}

fn write_json_report(report: &TestReport) -> ExitCode {
    if let Err(error) = report.write_json(io::stdout().lock()) {
        eprintln!("internal compiler error: failed to serialize test JSON: {error}");
        return ExitCode::from(3);
    }
    exit_code(report)
}

#[cfg(unix)]
fn signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn signal(_status: &ExitStatus) -> Option<i32> {
    None
}
