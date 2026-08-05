use super::errors::{exit_for_diagnostics, internal_error_exit};
use super::pipeline::check_file_with_target;
use crate::ast::AstEnvelope;
use crate::diagnostics::Diagnostic;
use crate::diagnostics::DiagnosticsEnvelope;
use crate::lexer::{TokensEnvelope, lex};
use crate::parser::{parse, parse_package_file};
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
                    return internal_error_exit();
                }
            }
        }
        Err(diagnostic) => {
            let status = exit_for_diagnostics(std::slice::from_ref(&diagnostic), ExitCode::FAILURE);
            let envelope = TokensEnvelope::new(
                file.to_string_lossy().into_owned(),
                canonical_absolute_string(file),
                Vec::new(),
                vec![diagnostic],
            );
            (envelope, status)
        }
    };

    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => {
            println!("{json}");
            status
        }
        Err(error) => {
            eprintln!("internal compiler error: failed to serialize token JSON: {error}");
            internal_error_exit()
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
                if file.file_name().is_some_and(|name| name == "nocter.nct") {
                    let parsed = parse_package_file(&sources, source, &lexed.tokens);
                    let status = if parsed.diagnostics.is_empty() {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    };
                    let source_file = sources
                        .get(source)
                        .expect("loaded source id must resolve in source map");
                    let envelope = AstEnvelope::new(
                        source_file.display_path().to_string(),
                        source_file
                            .absolute_path()
                            .map(|path| path.to_string_lossy().into_owned()),
                        parsed.package_file.map(|file| file.to_json(&sources)),
                        parsed.diagnostics,
                    );
                    (envelope, status)
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
        }
        Err(diagnostic) => {
            let status = exit_for_diagnostics(std::slice::from_ref(&diagnostic), ExitCode::FAILURE);
            let envelope = AstEnvelope::new(
                file.to_string_lossy().into_owned(),
                canonical_absolute_string(file),
                None,
                vec![diagnostic],
            );
            (envelope, status)
        }
    };

    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => {
            println!("{json}");
            status
        }
        Err(error) => {
            eprintln!("internal compiler error: failed to serialize AST JSON: {error}");
            internal_error_exit()
        }
    }
}

pub(super) fn run_check_json(file: &Path, target: &str) -> ExitCode {
    let output = check_file_with_target(file, target);
    let status = if output.is_ok() {
        ExitCode::SUCCESS
    } else {
        exit_for_diagnostics(&output.diagnostics, ExitCode::FAILURE)
    };

    write_diagnostics_json(
        "check",
        Some(target.to_string()),
        Some(output.root),
        output.root_absolute_path,
        output.diagnostics,
        status,
    )
}

pub(super) fn write_diagnostics_json(
    command: impl Into<String>,
    target: Option<String>,
    root: Option<String>,
    root_absolute_path: Option<String>,
    diagnostics: Vec<Diagnostic>,
    status: ExitCode,
) -> ExitCode {
    let envelope = DiagnosticsEnvelope::new(command, target, root, root_absolute_path, diagnostics);

    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => {
            println!("{json}");
            status
        }
        Err(error) => {
            eprintln!("internal compiler error: failed to serialize diagnostics JSON: {error}");
            internal_error_exit()
        }
    }
}

fn canonical_absolute_string(path: &Path) -> Option<String> {
    path.canonicalize()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}
