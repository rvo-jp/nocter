use super::pipeline::check_file_with_entry;
use crate::ast::AstEnvelope;
use crate::diagnostics::DiagnosticsEnvelope;
use crate::lexer::{TokensEnvelope, lex};
use crate::parser::parse;
use crate::source::SourceMap;
use std::path::Path;
use std::process::ExitCode;

pub(super) fn run_tokens_json(file: &Path) -> ExitCode {
    let mut sources = SourceMap::new();

    let (envelope, status) = match sources.load_file(file) {
        Ok(source) => {
            let output = lex(&sources, source);
            let status = if output.diagnostics.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };

            match output.to_json_envelope(&sources, source) {
                Ok(envelope) => (envelope, status),
                Err(error) => {
                    eprintln!("internal compiler error: {error}");
                    return internal_error();
                }
            }
        }
        Err(diagnostic) => {
            let envelope = TokensEnvelope::new(
                file.to_string_lossy().into_owned(),
                canonical_absolute_string(file),
                Vec::new(),
                vec![diagnostic],
            );
            (envelope, ExitCode::FAILURE)
        }
    };

    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => {
            println!("{json}");
            status
        }
        Err(error) => {
            eprintln!("internal compiler error: failed to serialize token JSON: {error}");
            internal_error()
        }
    }
}

pub(super) fn run_ast_json(file: &Path) -> ExitCode {
    let mut sources = SourceMap::new();

    let (envelope, status) = match sources.load_file(file) {
        Ok(source) => {
            let lexed = lex(&sources, source);
            if !lexed.diagnostics.is_empty() {
                let diagnostics = lexed.diagnostics;
                let envelope = AstEnvelope::new(
                    file.to_string_lossy().into_owned(),
                    canonical_absolute_string(file),
                    None,
                    diagnostics,
                );
                (envelope, ExitCode::FAILURE)
            } else {
                let parsed = parse(&sources, source, &lexed.tokens);
                let status = if parsed.diagnostics.is_empty() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                };
                let file = sources
                    .get(source)
                    .expect("loaded source id must resolve in source map");
                let envelope = AstEnvelope::new(
                    file.display_path().to_string(),
                    file.absolute_path()
                        .map(|path| path.to_string_lossy().into_owned()),
                    parsed.ast.map(|ast| ast.to_json(&sources)),
                    parsed.diagnostics,
                );
                (envelope, status)
            }
        }
        Err(diagnostic) => {
            let envelope = AstEnvelope::new(
                file.to_string_lossy().into_owned(),
                canonical_absolute_string(file),
                None,
                vec![diagnostic],
            );
            (envelope, ExitCode::FAILURE)
        }
    };

    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => {
            println!("{json}");
            status
        }
        Err(error) => {
            eprintln!("internal compiler error: failed to serialize AST JSON: {error}");
            internal_error()
        }
    }
}

pub(super) fn run_check_json(file: &Path, entry_name: &str) -> ExitCode {
    let output = check_file_with_entry(file, entry_name);
    let status = if output.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    };
    let envelope = DiagnosticsEnvelope::new(
        "check",
        None,
        Some(output.root),
        output.root_absolute_path,
        output.diagnostics,
    );

    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => {
            println!("{json}");
            status
        }
        Err(error) => {
            eprintln!("internal compiler error: failed to serialize diagnostics JSON: {error}");
            internal_error()
        }
    }
}

fn canonical_absolute_string(path: &Path) -> Option<String> {
    path.canonicalize()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

fn internal_error() -> ExitCode {
    ExitCode::from(3)
}
