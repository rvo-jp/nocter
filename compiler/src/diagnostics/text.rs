use super::{Diagnostic, Severity};
use std::io::{self, Write};

pub fn write_text_diagnostics(
    writer: &mut impl Write,
    diagnostics: &[Diagnostic],
) -> io::Result<()> {
    for diagnostic in diagnostics {
        write_text_diagnostic(writer, diagnostic)?;
    }

    Ok(())
}

fn write_text_diagnostic(writer: &mut impl Write, diagnostic: &Diagnostic) -> io::Result<()> {
    let label = severity_label(diagnostic.severity);

    match diagnostic.primary_span.as_deref() {
        Some(span) => writeln!(
            writer,
            "{}:{}:{}: {label}[{}]: {}",
            span.file, span.start_line, span.start_column_byte, diagnostic.code, diagnostic.message
        )?,
        None => writeln!(
            writer,
            "{label}[{}]: {}",
            diagnostic.code, diagnostic.message
        )?,
    }

    for note in &diagnostic.notes {
        match &note.span {
            Some(span) => writeln!(
                writer,
                "  note: {}:{}:{}: {}",
                span.file, span.start_line, span.start_column_byte, note.message
            )?,
            None => writeln!(writer, "  note: {}", note.message)?,
        }
    }

    if let Some(help) = &diagnostic.help {
        writeln!(writer, "  help: {help}")?;
    }

    Ok(())
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
    use crate::source::JsonSpan;

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
}
