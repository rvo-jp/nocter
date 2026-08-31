use std::fmt;
use std::io::{self, BufRead, Write};

use nocter_lsp::{FrameError, FrameReader, write_frame};

use crate::{LanguageServer, LanguageServerEnvironment, ServerIssue};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanguageServerExit {
    Protocol(i32),
    EndOfInput,
}

impl LanguageServerExit {
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Protocol(code) => code,
            Self::EndOfInput => 1,
        }
    }
}

/// Runs the sequential LSP transport without writing non-protocol data to its output.
///
/// Recoverable notification issues are reported to `report_issue`; framing and output failures
/// terminate the loop.
///
/// # Errors
///
/// Returns a framing/input or protocol-output I/O failure.
pub fn run_language_server(
    input: impl BufRead,
    mut output: impl Write,
    server_version: &str,
    environment: LanguageServerEnvironment,
    mut report_issue: impl FnMut(&ServerIssue),
) -> Result<LanguageServerExit, LanguageServerRunError> {
    let mut frames = FrameReader::new(input);
    let mut server = LanguageServer::new(server_version, environment);
    loop {
        let Some(body) = frames.read().map_err(LanguageServerRunError::Frame)? else {
            return Ok(LanguageServerExit::EndOfInput);
        };
        let step = server.receive(&body);
        if let Some(response) = step.response() {
            write_frame(&mut output, response).map_err(LanguageServerRunError::Output)?;
        }
        for message in step.outbound_messages() {
            write_frame(&mut output, message).map_err(LanguageServerRunError::Output)?;
        }
        for issue in step.issues() {
            report_issue(issue);
        }
        if let Some(code) = step.exit_code() {
            return Ok(LanguageServerExit::Protocol(code));
        }
    }
}

#[derive(Debug)]
pub enum LanguageServerRunError {
    Frame(FrameError),
    Output(io::Error),
}

impl fmt::Display for LanguageServerRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => error.fmt(formatter),
            Self::Output(error) => write!(formatter, "cannot write LSP response: {error}"),
        }
    }
}

impl std::error::Error for LanguageServerRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            Self::Output(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use nocter_lsp::FrameReader;
    use nocter_model::{CompilationTarget, PackageIdentity};
    use nocter_package::StandardPackage;

    use super::*;

    #[test]
    fn runs_framed_initialize_shutdown_and_clean_exit_without_extra_output() {
        let bodies = [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#,
            r#"{"jsonrpc":"2.0","method":"initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#,
            r#"{"jsonrpc":"2.0","method":"exit"}"#,
        ];
        let mut input = Vec::new();
        for body in bodies {
            write_frame(&mut input, body).unwrap();
        }
        let mut output = Vec::new();
        let mut issues = Vec::new();
        let root = std::env::temp_dir();
        let environment = LanguageServerEnvironment::new(
            &root,
            crate::LanguageServerToolchain::new(
                CompilationTarget::Arm64Darwin,
                &root,
                StandardPackage::new(PackageIdentity::new("toolchain:std"), &root, "0.0.0"),
            ),
        );
        let exit = run_language_server(
            Cursor::new(input),
            &mut output,
            "dev",
            environment,
            |issue| issues.push(issue.to_string()),
        )
        .unwrap();
        assert_eq!(exit, LanguageServerExit::Protocol(0));
        assert!(issues.is_empty());

        let mut frames = FrameReader::new(Cursor::new(output));
        let initialize = frames.read().unwrap().unwrap();
        assert!(initialize.contains("\"id\":1"));
        assert!(initialize.contains("\"positionEncoding\":\"utf-16\""));
        assert_eq!(
            frames.read().unwrap().as_deref(),
            Some(r#"{"jsonrpc":"2.0","id":2,"result":null}"#)
        );
        assert_eq!(frames.read().unwrap(), None);
    }
}
