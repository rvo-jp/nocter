//! Source-level abstract syntax tree definitions.

use crate::diagnostics::Diagnostic;
use crate::source::JsonSpan;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonAstNode {
    pub kind: String,
    pub span: Option<JsonSpan>,
    pub items: Vec<JsonAstNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AstEnvelope {
    pub schema: &'static str,
    pub version: u32,
    pub ok: bool,
    pub command: &'static str,
    pub file: String,
    pub absolute_path: Option<String>,
    pub ast: Option<JsonAstNode>,
    pub diagnostics: Vec<Diagnostic>,
}

impl AstEnvelope {
    pub fn new(
        file: impl Into<String>,
        absolute_path: Option<String>,
        ast: Option<JsonAstNode>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        let ok = diagnostics.is_empty();

        Self {
            schema: "nocter.ast",
            version: 1,
            ok,
            command: "ast",
            file: file.into(),
            absolute_path,
            ast,
            diagnostics,
        }
    }
}
