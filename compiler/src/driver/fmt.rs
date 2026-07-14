use crate::diagnostics::{write_text_diagnostics, write_text_diagnostics_with_sources};
use crate::format::format_source;
use crate::source::SourceMap;
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitCode;

pub(super) fn run_fmt(file: &Path, check: bool) -> ExitCode {
    let mut sources = SourceMap::new();
    let source = match sources.load_file(file) {
        Ok(source) => source,
        Err(diagnostic) => {
            return write_diagnostics_and_fail(&[diagnostic], None);
        }
    };

    let output = format_source(&sources, source);
    if !output.is_ok() {
        return write_diagnostics_and_fail(&output.diagnostics, Some(&sources));
    }

    let formatted = output
        .formatted
        .expect("successful format output must include formatted source");
    let original = sources
        .get(source)
        .expect("loaded source id must resolve in source map")
        .text();

    if original == formatted {
        return ExitCode::SUCCESS;
    }

    if check {
        eprintln!(
            "error: `{}` is not formatted; run `nocter fmt {}`",
            file.display(),
            file.display()
        );
        return ExitCode::FAILURE;
    }

    match fs::write(file, formatted) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "error: failed to write formatted source `{}`: {error}",
                file.display()
            );
            ExitCode::FAILURE
        }
    }
}

fn write_diagnostics_and_fail(
    diagnostics: &[crate::diagnostics::Diagnostic],
    sources: Option<&SourceMap>,
) -> ExitCode {
    let mut stderr = io::stderr().lock();
    let result = match sources {
        Some(sources) => write_text_diagnostics_with_sources(&mut stderr, diagnostics, sources),
        None => write_text_diagnostics(&mut stderr, diagnostics),
    };

    if let Err(error) = result {
        eprintln!("internal compiler error: failed to write diagnostics: {error}");
        return ExitCode::from(3);
    }

    ExitCode::FAILURE
}
