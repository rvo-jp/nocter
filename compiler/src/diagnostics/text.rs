use super::{Diagnostic, Severity};
use crate::source::{JsonSpan, SourceMap};
use std::io::{self, Write};

pub fn write_text_diagnostics(
    writer: &mut impl Write,
    diagnostics: &[Diagnostic],
) -> io::Result<()> {
    write_text_diagnostics_inner(writer, diagnostics, None)
}

pub fn write_text_diagnostics_with_sources(
    writer: &mut impl Write,
    diagnostics: &[Diagnostic],
    sources: &SourceMap,
) -> io::Result<()> {
    write_text_diagnostics_inner(writer, diagnostics, Some(sources))
}

fn write_text_diagnostics_inner(
    writer: &mut impl Write,
    diagnostics: &[Diagnostic],
    sources: Option<&SourceMap>,
) -> io::Result<()> {
    for diagnostic in diagnostics {
        write_text_diagnostic(writer, diagnostic, sources)?;
    }

    Ok(())
}

fn write_text_diagnostic(
    writer: &mut impl Write,
    diagnostic: &Diagnostic,
    sources: Option<&SourceMap>,
) -> io::Result<()> {
    let label = severity_label(diagnostic.severity);

    match diagnostic.primary_span.as_deref() {
        Some(span) => {
            writeln!(
                writer,
                "{}:{}:{}: {label}[{}]: {}",
                span.file,
                span.start_line,
                span.start_column_byte,
                diagnostic.code,
                diagnostic.message
            )?;
            if let Some(sources) = sources {
                write_source_snippet(writer, sources, span)?;
            }
        }
        None => writeln!(
            writer,
            "{label}[{}]: {}",
            diagnostic.code, diagnostic.message
        )?,
    }

    for note in &diagnostic.notes {
        match &note.span {
            Some(span) => {
                writeln!(
                    writer,
                    "  note: {}:{}:{}: {}",
                    span.file, span.start_line, span.start_column_byte, note.message
                )?;
                if let Some(sources) = sources {
                    write_source_snippet(writer, sources, span)?;
                }
            }
            None => writeln!(writer, "  note: {}", note.message)?,
        }
    }

    if let Some(help) = &diagnostic.help {
        writeln!(writer, "  help: {help}")?;
    }

    Ok(())
}

fn write_source_snippet(
    writer: &mut impl Write,
    sources: &SourceMap,
    span: &JsonSpan,
) -> io::Result<()> {
    let Some(file) = sources.file_for_json_span(span) else {
        return Ok(());
    };
    let Some(line) = file.line_text(span.start_line) else {
        return Ok(());
    };

    let line_number_width = span.end_line.max(span.start_line).to_string().len();
    let marker = marker_for_span(line, span);

    writeln!(writer, "{:>width$} |", "", width = line_number_width)?;
    writeln!(
        writer,
        "{:>width$} | {line}",
        span.start_line,
        width = line_number_width
    )?;
    writeln!(
        writer,
        "{:>width$} | {}",
        "",
        marker,
        width = line_number_width
    )?;

    Ok(())
}

fn marker_for_span(line: &str, span: &JsonSpan) -> String {
    let start = span.start_column_byte.saturating_sub(1).min(line.len());
    let end = if span.start_line == span.end_line {
        span.end_column_byte.saturating_sub(1).min(line.len())
    } else {
        line.len()
    };
    let marker_len = end.saturating_sub(start).max(1);
    let mut marker = String::new();

    for ch in line[..start].chars() {
        marker.push(if ch == '\t' { '\t' } else { ' ' });
    }

    marker.extend(std::iter::repeat('^').take(marker_len));
    marker
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{Diagnostic, DiagnosticNote};
    use crate::source::{ByteSpan, JsonSpan, SourceMap};

    #[test]
    fn renders_diagnostics_for_humans() {
        let mut diagnostic = Diagnostic::error("E0001", "expected expression");
        diagnostic.primary_span = Some(Box::new(JsonSpan {
            file: "app.nct".to_string(),
            absolute_path: None,
            start_byte: 4,
            end_byte: 5,
            start_line: 2,
            start_column_byte: 3,
            end_line: 2,
            end_column_byte: 4,
        }));
        diagnostic.notes.push(DiagnosticNote {
            message: "while parsing function body".to_string(),
            span: None,
        });
        diagnostic.help = Some("insert an expression".to_string());

        let mut output = Vec::new();
        write_text_diagnostics(&mut output, &[diagnostic]).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            concat!(
                "app.nct:2:3: error[E0001]: expected expression\n",
                "  note: while parsing function body\n",
                "  help: insert an expression\n",
            )
        );
    }

    #[test]
    fn renders_source_snippet_when_sources_are_available() {
        let mut sources = SourceMap::new();
        let source = sources.add_source(
            "app.nct",
            None,
            "func main(): i32 {\n    return \"bad\"\n}\n",
        );
        let mut diagnostic = Diagnostic::error(
            "E0312",
            "`return` value has type `&str`, but function `main` returns `i32`",
        );
        diagnostic.primary_span = Some(Box::new(
            sources
                .span_to_json(ByteSpan::new(source, 30, 35))
                .expect("span should map to source"),
        ));

        let mut output = Vec::new();
        write_text_diagnostics_with_sources(&mut output, &[diagnostic], &sources).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            concat!(
                "app.nct:2:12: error[E0312]: `return` value has type `&str`, but function `main` returns `i32`\n",
                "  |\n",
                "2 |     return \"bad\"\n",
                "  |            ^^^^^\n",
            )
        );
    }
}
