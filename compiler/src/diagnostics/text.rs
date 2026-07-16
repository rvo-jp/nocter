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
    if file.line_text(span.start_line).is_none() {
        return Ok(());
    }

    let line_number_width = span.end_line.max(span.start_line).to_string().len();

    writeln!(writer, "{:>width$} |", "", width = line_number_width)?;
    for line_number in span.start_line..=span.end_line.max(span.start_line) {
        let Some(line) = file.line_text(line_number) else {
            continue;
        };
        let marker = marker_for_line_span(line, span, line_number);

        writeln!(
            writer,
            "{:>width$} | {line}",
            line_number,
            width = line_number_width
        )?;
        writeln!(
            writer,
            "{:>width$} | {}",
            "",
            marker,
            width = line_number_width
        )?;
    }

    Ok(())
}

fn marker_for_line_span(line: &str, span: &JsonSpan, line_number: usize) -> String {
    let start = if line_number == span.start_line {
        span.start_column_byte.saturating_sub(1)
    } else {
        0
    };
    let end = if line_number == span.end_line {
        span.end_column_byte.saturating_sub(1)
    } else {
        line.len()
    };
    let start = floor_char_boundary(line, start.min(line.len()));
    let end = floor_char_boundary(line, end.min(line.len()));
    let marker_len = line[start..end].chars().count().max(1);
    let mut marker = String::new();

    for ch in line[..start].chars() {
        marker.push(if ch == '\t' { '\t' } else { ' ' });
    }

    marker.extend(std::iter::repeat('^').take(marker_len));
    marker
}

fn floor_char_boundary(line: &str, index: usize) -> usize {
    let mut index = index.min(line.len());
    while index > 0 && !line.is_char_boundary(index) {
        index -= 1;
    }
    index
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

    #[test]
    fn renders_multiline_source_snippet_when_span_crosses_lines() {
        let mut sources = SourceMap::new();
        let text = "func main(): i32 {\n    let value = true\n    return value\n}\n";
        let source = sources.add_source("app.nct", None, text);
        let start = text.find("let value").expect("start text should exist");
        let end = text.find("value\n}").expect("end text should exist") + "value".len();
        let mut diagnostic = Diagnostic::error("E0002", "expression cannot span these statements");
        diagnostic.primary_span = Some(Box::new(
            sources
                .span_to_json(ByteSpan::new(source, start, end))
                .expect("span should map to source"),
        ));

        let mut output = Vec::new();
        write_text_diagnostics_with_sources(&mut output, &[diagnostic], &sources).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            concat!(
                "app.nct:2:5: error[E0002]: expression cannot span these statements\n",
                "  |\n",
                "2 |     let value = true\n",
                "  |     ^^^^^^^^^^^^^^^^\n",
                "3 |     return value\n",
                "  | ^^^^^^^^^^^^^^^^\n",
            )
        );
    }

    #[test]
    fn renders_source_snippet_for_note_spans() {
        let mut sources = SourceMap::new();
        let text = concat!(
            "func expects(value: i32): i32 {\n",
            "    return value\n",
            "}\n",
            "\n",
            "func main(): i32 {\n",
            "    return expects(\"bad\")\n",
            "}\n",
        );
        let source = sources.add_source("app.nct", None, text);
        let argument_start = text.find("\"bad\"").expect("argument text should exist");
        let parameter_start = text
            .find("value: i32")
            .expect("parameter text should exist");
        let mut diagnostic = Diagnostic::error(
            "E0321",
            "argument 1 has type `&str`, but parameter `value` expects `i32`",
        );
        diagnostic.primary_span = Some(Box::new(
            sources
                .span_to_json(ByteSpan::new(
                    source,
                    argument_start,
                    argument_start + "\"bad\"".len(),
                ))
                .expect("argument span should map to source"),
        ));
        diagnostic.notes.push(DiagnosticNote {
            message: "parameter `value` is declared here".to_string(),
            span: Some(
                sources
                    .span_to_json(ByteSpan::new(
                        source,
                        parameter_start,
                        parameter_start + "value: i32".len(),
                    ))
                    .expect("parameter span should map to source"),
            ),
        });

        let mut output = Vec::new();
        write_text_diagnostics_with_sources(&mut output, &[diagnostic], &sources).unwrap();
        let output = String::from_utf8(output).unwrap();

        assert!(
            output.contains("6 |     return expects(\"bad\")\n  |                    ^^^^^"),
            "expected primary snippet, got:\n{output}"
        );
        assert!(
            output.contains("  note: app.nct:1:14: parameter `value` is declared here"),
            "expected note location, got:\n{output}"
        );
        assert!(
            output.contains("1 | func expects(value: i32): i32 {\n  |              ^^^^^^^^^^"),
            "expected note snippet, got:\n{output}"
        );
    }
}
