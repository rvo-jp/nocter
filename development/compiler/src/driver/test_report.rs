use crate::diagnostics::Diagnostic;
use serde::Serialize;
use std::io::{self, Write};
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TestOutcome {
    Passed,
    Failed,
    CompileFailed,
    RunnerFailed,
}

#[derive(Debug, Serialize)]
pub(super) struct TestTargetReport {
    pub(super) name: String,
    pub(super) outcome: TestOutcome,
    pub(super) exit_code: Option<i32>,
    pub(super) signal: Option<i32>,
    pub(super) stdout: String,
    pub(super) stderr: String,
    pub(super) diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Serialize)]
pub(super) struct TestSummary {
    pub(super) passed: usize,
    pub(super) failed: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct TestReport {
    schema: &'static str,
    version: u32,
    ok: bool,
    package: String,
    target: String,
    diagnostics: Vec<Diagnostic>,
    tests: Vec<TestTargetReport>,
    summary: TestSummary,
}

impl TestReport {
    pub(super) fn new(
        package: String,
        target: String,
        diagnostics: Vec<Diagnostic>,
        tests: Vec<TestTargetReport>,
    ) -> Self {
        let passed = tests
            .iter()
            .filter(|test| test.outcome == TestOutcome::Passed)
            .count();
        let failed = tests.len() - passed;
        let ok = diagnostics.is_empty() && failed == 0;
        Self {
            schema: "nocter.tests",
            version: 1,
            ok,
            package,
            target,
            diagnostics,
            tests,
            summary: TestSummary { passed, failed },
        }
    }

    pub(super) fn is_ok(&self) -> bool {
        self.ok
    }

    pub(super) fn tests(&self) -> &[TestTargetReport] {
        &self.tests
    }

    pub(super) fn summary(&self) -> &TestSummary {
        &self.summary
    }

    pub(super) fn write_json(&self, mut writer: impl Write) -> io::Result<()> {
        serde_json::to_writer_pretty(&mut writer, self)?;
        writeln!(writer)
    }
}

pub(super) fn exit_code(report: &TestReport) -> ExitCode {
    if report.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
