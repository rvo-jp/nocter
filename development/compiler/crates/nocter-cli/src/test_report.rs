use std::fmt::Write;

use nocter_command::{DiagnosticFormat, TestCommandResult, TestRunOutcome, TestRunResult};
use nocter_diagnostics::{
    DiagnosticRenderError, SpanlessDiagnostic, render_source_diagnostic, write_json_string,
    write_source_diagnostic_items_json, write_spanless_diagnostic_json,
};

pub(crate) fn render_test_human(result: &TestCommandResult) -> Option<String> {
    if result.presentation().format() == DiagnosticFormat::Json {
        return None;
    }
    let mut output = String::new();
    for run in result.runs() {
        let status = match run.outcome() {
            TestRunOutcome::Passed => "PASS",
            TestRunOutcome::Failed => "FAIL",
            TestRunOutcome::CompileFailed => "COMPILE FAILED",
            TestRunOutcome::RunnerFailed => "RUNNER FAILED",
        };
        write!(output, "{status} {}", run.target().name()).expect("writing to String cannot fail");
        if let Some(test) = run.test() {
            write!(output, " :: {}", test.name()).expect("writing to String cannot fail");
        }
        if let Some(code) = run.exit_code() {
            write!(output, " (exit {code})").expect("writing to String cannot fail");
        } else if let Some(signal) = run.signal() {
            write!(output, " (signal {signal})").expect("writing to String cannot fail");
        }
        output.push('\n');
        append_stream(&mut output, "stdout", run.stdout());
        append_stream(&mut output, "stderr", run.stderr());
        for diagnostic in run.diagnostics() {
            writeln!(
                output,
                "  error[{}]: {}",
                diagnostic.code(),
                diagnostic.message()
            )
            .expect("writing to String cannot fail");
        }
        if !run.source_diagnostics().is_empty() {
            match run.sources() {
                Some(sources) => {
                    for diagnostic in run.source_diagnostics() {
                        match render_source_diagnostic(diagnostic, sources) {
                            Ok(rendered) => output.push_str(&rendered),
                            Err(error) => writeln!(
                                output,
                                "  error[E0900]: cannot render test diagnostic: {error}"
                            )
                            .expect("writing to String cannot fail"),
                        }
                    }
                }
                None => output
                    .push_str("  error[E0900]: test run lost its diagnostic source snapshot\n"),
            }
        }
    }
    writeln!(
        output,
        "{} passed; {} failed",
        result.summary().passed(),
        result.summary().failed()
    )
    .expect("writing to String cannot fail");
    Some(output)
}

fn append_stream(output: &mut String, name: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    writeln!(output, "  {name}:").expect("writing to String cannot fail");
    for line in String::from_utf8_lossy(bytes).lines() {
        writeln!(output, "    {line}").expect("writing to String cannot fail");
    }
}

pub(crate) fn render_test_json(
    result: &TestCommandResult,
) -> Result<String, DiagnosticRenderError> {
    let mut output = String::new();
    write_envelope_start(
        &mut output,
        result.succeeded(),
        Some(result.package().as_str()),
        Some(result.presentation().target().name()),
    );
    output.push_str("],\"runs\":[");
    for (index, run) in result.runs().iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write_run(&mut output, run)?;
    }
    writeln!(
        output,
        "],\"summary\":{{\"passed\":{},\"failed\":{}}}}}",
        result.summary().passed(),
        result.summary().failed()
    )
    .expect("writing to String cannot fail");
    Ok(output)
}

pub(crate) fn render_test_source_failure_json(
    target: Option<&str>,
    diagnostics: &[nocter_diagnostics::SourceDiagnostic],
    sources: &nocter_source::SourceMap,
) -> Result<String, DiagnosticRenderError> {
    let mut output = String::new();
    write_envelope_start(&mut output, false, None, target);
    write_source_diagnostic_items_json(&mut output, diagnostics, sources)?;
    output.push_str("],\"runs\":[],\"summary\":{\"passed\":0,\"failed\":1}}\n");
    Ok(output)
}

pub(crate) fn render_test_spanless_failure_json(
    target: Option<&str>,
    code: &str,
    message: &str,
) -> String {
    let mut output = String::new();
    write_envelope_start(&mut output, false, None, target);
    write_spanless_diagnostic_json(&mut output, SpanlessDiagnostic::new(code, message, None));
    output.push_str("],\"runs\":[],\"summary\":{\"passed\":0,\"failed\":1}}\n");
    output
}

fn write_envelope_start(
    output: &mut String,
    ok: bool,
    package: Option<&str>,
    target: Option<&str>,
) {
    output.push_str("{\"schema\":\"nocter.tests\",\"version\":1,\"ok\":");
    output.push_str(if ok { "true" } else { "false" });
    output.push_str(",\"package\":");
    write_optional_string(output, package);
    output.push_str(",\"target\":");
    write_optional_string(output, target);
    output.push_str(",\"diagnostics\":[");
}

fn write_run(output: &mut String, run: &TestRunResult) -> Result<(), DiagnosticRenderError> {
    output.push_str("{\"target\":");
    write_json_string(output, run.target().name());
    output.push_str(",\"test\":");
    write_optional_string(output, run.test_name());
    output.push_str(",\"outcome\":");
    write_json_string(output, run.outcome().name());
    output.push_str(",\"exit_code\":");
    write_optional_i32(output, run.exit_code());
    output.push_str(",\"signal\":");
    write_optional_i32(output, run.signal());
    output.push_str(",\"stdout\":");
    write_captured_output(output, run.stdout());
    output.push_str(",\"stderr\":");
    write_captured_output(output, run.stderr());
    output.push_str(",\"diagnostics\":[");
    if !run.source_diagnostics().is_empty() {
        if let Some(sources) = run.sources() {
            write_source_diagnostic_items_json(output, run.source_diagnostics(), sources)?;
        } else {
            write_spanless_diagnostic_json(
                output,
                SpanlessDiagnostic::new(
                    "E0900",
                    "test run lost its diagnostic source snapshot",
                    None,
                ),
            );
        }
    }
    for (index, diagnostic) in run.diagnostics().iter().enumerate() {
        if index != 0 || !run.source_diagnostics().is_empty() {
            output.push(',');
        }
        write_spanless_diagnostic_json(
            output,
            SpanlessDiagnostic::new(diagnostic.code(), diagnostic.message(), None),
        );
    }
    output.push_str("]}");
    Ok(())
}

fn write_captured_output(output: &mut String, bytes: &[u8]) {
    if let Ok(text) = std::str::from_utf8(bytes) {
        output.push_str("{\"encoding\":\"utf-8\",\"text\":");
        write_json_string(output, text);
    } else {
        output.push_str("{\"encoding\":\"base64\",\"text\":");
        write_json_string(output, &base64(bytes));
    }
    output.push('}');
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(char::from(ALPHABET[usize::from(first >> 2)]));
        encoded.push(char::from(
            ALPHABET[usize::from((first & 0x03) << 4 | second >> 4)],
        ));
        encoded.push(if chunk.len() > 1 {
            char::from(ALPHABET[usize::from((second & 0x0f) << 2 | third >> 6)])
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            char::from(ALPHABET[usize::from(third & 0x3f)])
        } else {
            '='
        });
    }
    encoded
}

fn write_optional_string(output: &mut String, value: Option<&str>) {
    match value {
        Some(value) => write_json_string(output, value),
        None => output.push_str("null"),
    }
}

fn write_optional_i32(output: &mut String, value: Option<i32>) {
    match value {
        Some(value) => write!(output, "{value}").expect("writing to String cannot fail"),
        None => output.push_str("null"),
    }
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn base64_preserves_partial_groups() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
    }
}
