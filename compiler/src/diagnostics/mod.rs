//! Compiler-owned diagnostics shared by CLI, JSON output, and future LSP.

mod text;

use crate::source::{ByteSpan, JsonSpan, SourceMap};
use serde::Serialize;

pub use text::write_text_diagnostics;
pub use text::write_text_diagnostics_with_sources;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub primary_span: Option<Box<JsonSpan>>,
    pub notes: Vec<DiagnosticNote>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            message: message.into(),
            primary_span: None,
            notes: Vec::new(),
            help: None,
        }
    }

    pub fn with_primary_span_if_absent(mut self, sources: &SourceMap, span: ByteSpan) -> Self {
        if self.primary_span.is_none() {
            self.primary_span = sources.span_to_json(span).ok().map(Box::new);
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticNote {
    pub message: String,
    pub span: Option<JsonSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticsEnvelope {
    pub schema: &'static str,
    pub version: u32,
    pub ok: bool,
    pub command: String,
    pub target: Option<String>,
    pub root: Option<String>,
    pub root_absolute_path: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticsEnvelope {
    pub fn new(
        command: impl Into<String>,
        target: Option<String>,
        root: Option<String>,
        root_absolute_path: Option<String>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        let ok = diagnostics.is_empty();

        Self {
            schema: "nocter.diagnostics",
            version: 1,
            ok,
            command: command.into(),
            target,
            root,
            root_absolute_path,
            diagnostics,
        }
    }
}
