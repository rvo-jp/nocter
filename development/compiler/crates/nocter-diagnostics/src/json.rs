use std::fmt::Write;

use nocter_source::SourceMap;

use crate::projection::{absolute_source_name, project_origin};
use crate::{DiagnosticOrigin, DiagnosticRenderError, SourceDiagnostic};

/// Stable top-level facts for one machine-readable diagnostic response.
#[derive(Clone, Copy, Debug)]
pub struct DiagnosticJsonContext<'a> {
    command: &'a str,
    target: Option<&'a str>,
    root: Option<&'a str>,
    root_absolute_path: Option<&'a str>,
}

impl<'a> DiagnosticJsonContext<'a> {
    #[must_use]
    pub const fn new(
        command: &'a str,
        target: Option<&'a str>,
        root: Option<&'a str>,
        root_absolute_path: Option<&'a str>,
    ) -> Self {
        Self {
            command,
            target,
            root,
            root_absolute_path,
        }
    }
}

/// Renders one complete `nocter.diagnostics` version-1 JSON envelope.
///
/// # Errors
///
/// Returns the same source/range integrity failures as human presentation.
pub fn render_source_diagnostics_json(
    context: DiagnosticJsonContext<'_>,
    diagnostics: &[SourceDiagnostic],
    sources: &SourceMap,
) -> Result<String, DiagnosticRenderError> {
    let mut output = String::new();
    output.push_str("{\"schema\":\"nocter.diagnostics\",\"version\":1,\"ok\":");
    output.push_str(if diagnostics.is_empty() {
        "true"
    } else {
        "false"
    });
    output.push_str(",\"command\":");
    write_json_string(&mut output, context.command);
    output.push_str(",\"target\":");
    write_optional_string(&mut output, context.target);
    output.push_str(",\"root\":");
    write_optional_string(&mut output, context.root);
    output.push_str(",\"root_absolute_path\":");
    write_optional_string(&mut output, context.root_absolute_path);
    output.push_str(",\"diagnostics\":[");
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write_diagnostic(&mut output, diagnostic, sources)?;
    }
    output.push_str("]}\n");
    Ok(output)
}

fn write_diagnostic(
    output: &mut String,
    diagnostic: &SourceDiagnostic,
    sources: &SourceMap,
) -> Result<(), DiagnosticRenderError> {
    output.push_str("{\"code\":");
    write_json_string(output, diagnostic.code());
    output.push_str(",\"severity\":\"error\",\"message\":");
    write_json_string(output, diagnostic.message());
    output.push_str(",\"primary_span\":");
    write_span(output, diagnostic.primary(), sources)?;
    output.push_str(",\"notes\":[");
    for (index, note) in diagnostic.notes().iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"message\":");
        write_json_string(output, note.message());
        output.push_str(",\"span\":");
        write_span(output, note.origin(), sources)?;
        output.push('}');
    }
    output.push_str("],\"help\":");
    write_optional_string(output, diagnostic.help());
    output.push('}');
    Ok(())
}

fn write_span(
    output: &mut String,
    origin: DiagnosticOrigin,
    sources: &SourceMap,
) -> Result<(), DiagnosticRenderError> {
    let projected = project_origin(origin, sources)?;
    output.push_str("{\"file\":");
    write_json_string(output, projected.source.name().as_str());
    output.push_str(",\"absolute_path\":");
    write_optional_string(output, absolute_source_name(projected.source));
    write!(
        output,
        ",\"start_byte\":{},\"end_byte\":{},\"start_line\":{},\"start_column_byte\":{},\"end_line\":{},\"end_column_byte\":{}",
        projected.range.start().get(),
        projected.range.end().get(),
        projected.start.line() + 1,
        projected.start.byte_column() + 1,
        projected.end.line() + 1,
        projected.end.byte_column() + 1,
    )
    .expect("writing to String cannot fail");
    output.push('}');
    Ok(())
}

fn write_optional_string(output: &mut String, value: Option<&str>) {
    match value {
        Some(value) => write_json_string(output, value),
        None => output.push_str("null"),
    }
}

fn write_json_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1f}' => {
                write!(output, "\\u{:04x}", u32::from(character))
                    .expect("writing to String cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

#[cfg(test)]
mod tests {
    use nocter_source::{ByteOffset, SourceMap, SourceName, TextRange};

    use super::*;
    use crate::{DiagnosticNote, SourceDiagnostic};

    #[test]
    fn renders_exact_versioned_envelope_with_byte_coordinates_and_escaping() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("src/β.nct"), "aβ\n".as_bytes())
            .unwrap();
        let file = sources.get(source).unwrap();
        let primary = file.span(TextRange::new(ByteOffset::new(1), ByteOffset::new(3)));
        let note = file.span(TextRange::new(ByteOffset::new(0), ByteOffset::new(1)));
        let diagnostic = SourceDiagnostic::new(
            "E9999",
            "bad \"value\"\nnext",
            primary,
            [DiagnosticNote::new("before\\after", note)],
            Some("use β"),
        );

        let rendered = render_source_diagnostics_json(
            DiagnosticJsonContext::new("check", Some("arm64-darwin"), Some("root.nct"), None),
            &[diagnostic],
            &sources,
        )
        .unwrap();

        assert_eq!(
            rendered,
            concat!(
                "{\"schema\":\"nocter.diagnostics\",\"version\":1,\"ok\":false,",
                "\"command\":\"check\",\"target\":\"arm64-darwin\",\"root\":\"root.nct\",",
                "\"root_absolute_path\":null,\"diagnostics\":[{\"code\":\"E9999\",",
                "\"severity\":\"error\",\"message\":\"bad \\\"value\\\"\\nnext\",",
                "\"primary_span\":{\"file\":\"src/β.nct\",\"absolute_path\":null,",
                "\"start_byte\":1,\"end_byte\":3,\"start_line\":1,\"start_column_byte\":2,",
                "\"end_line\":1,\"end_column_byte\":4},\"notes\":[{\"message\":",
                "\"before\\\\after\",\"span\":{\"file\":\"src/β.nct\",",
                "\"absolute_path\":null,\"start_byte\":0,\"end_byte\":1,",
                "\"start_line\":1,\"start_column_byte\":1,\"end_line\":1,",
                "\"end_column_byte\":2}}],\"help\":\"use β\"}]}\n"
            )
        );
    }

    #[test]
    fn renders_success_as_one_empty_diagnostic_envelope() {
        assert_eq!(
            render_source_diagnostics_json(
                DiagnosticJsonContext::new("check", None, None, None),
                &[],
                &SourceMap::new(),
            )
            .unwrap(),
            "{\"schema\":\"nocter.diagnostics\",\"version\":1,\"ok\":true,\"command\":\"check\",\"target\":null,\"root\":null,\"root_absolute_path\":null,\"diagnostics\":[]}\n"
        );
    }
}
