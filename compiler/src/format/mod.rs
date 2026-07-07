//! Source formatter for Nocter syntax.

mod comments;
mod expressions;
mod items;
mod statements;
mod types;

#[cfg(test)]
mod tests;

use crate::ast::AstFile;
use crate::diagnostics::Diagnostic;
use crate::lexer::lex;
use crate::parser::parse;
use crate::source::{SourceId, SourceMap};
use comments::first_comment_span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOutput {
    pub formatted: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl FormatOutput {
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub fn format_source(sources: &SourceMap, source: SourceId) -> FormatOutput {
    let Some(file) = sources.get(source) else {
        return FormatOutput {
            formatted: None,
            diagnostics: vec![Diagnostic::error(
                "E0600",
                format!("unknown source id {}", source.raw()),
            )],
        };
    };

    if let Some(span) = first_comment_span(source, file.text()) {
        let mut diagnostic =
            Diagnostic::error("E0601", "formatter v0 cannot safely preserve comments yet");
        diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
        diagnostic.help = Some(
            "remove comments before formatting, or wait for the comment-preserving formatter"
                .to_string(),
        );
        return FormatOutput {
            formatted: None,
            diagnostics: vec![diagnostic],
        };
    }

    let lexed = lex(sources, source);
    if !lexed.diagnostics.is_empty() {
        return FormatOutput {
            formatted: None,
            diagnostics: lexed.diagnostics,
        };
    }

    let parsed = parse(sources, source, &lexed.tokens);
    if !parsed.diagnostics.is_empty() {
        return FormatOutput {
            formatted: None,
            diagnostics: parsed.diagnostics,
        };
    }

    let Some(ast) = parsed.ast else {
        return FormatOutput {
            formatted: None,
            diagnostics: vec![Diagnostic::error(
                "E0600",
                "parser did not produce an AST and did not report a diagnostic",
            )],
        };
    };

    FormatOutput {
        formatted: Some(format_ast(&ast)),
        diagnostics: Vec::new(),
    }
}

pub fn format_ast(ast: &AstFile) -> String {
    let mut formatter = Formatter::new();
    formatter.format_file(ast);
    formatter.finish()
}

struct Formatter {
    output: String,
    indent: usize,
}

impl Formatter {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    fn finish(self) -> String {
        self.output
    }

    fn write(&mut self, text: &str) {
        self.output.push_str(text);
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    fn blank_line(&mut self) {
        self.newline();
        self.newline();
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    fn indented(&mut self, write: impl FnOnce(&mut Self)) {
        self.indent += 1;
        write(self);
        self.indent -= 1;
    }

    fn write_comma_separated<T>(&mut self, values: &[T], mut write: impl FnMut(&mut Self, &T)) {
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                self.write(", ");
            }
            write(self, value);
        }
    }
}
