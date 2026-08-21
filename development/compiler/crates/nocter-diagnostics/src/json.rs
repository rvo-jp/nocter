use std::fmt::Write;

pub use nocter_json::write_string as write_json_string;
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

/// One diagnostic whose failure boundary has no useful source location.
#[derive(Clone, Copy, Debug)]
pub struct SpanlessDiagnostic<'a> {
    code: &'a str,
    message: &'a str,
    help: Option<&'a str>,
}

impl<'a> SpanlessDiagnostic<'a> {
    #[must_use]
    pub const fn new(code: &'a str, message: &'a str, help: Option<&'a str>) -> Self {
        Self {
            code,
            message,
            help,
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
    render_diagnostics_json(context, diagnostics.is_empty(), |output| {
        write_source_diagnostic_items_json(output, diagnostics, sources)
    })
}

/// Appends comma-separated source diagnostic objects without an enclosing JSON array.
///
/// This is the shared projection boundary for versioned envelopes that embed ordinary compiler
/// diagnostics beside domain-specific result data.
///
/// # Errors
///
/// Returns a source/range integrity failure.
pub fn write_source_diagnostic_items_json(
    output: &mut String,
    diagnostics: &[SourceDiagnostic],
    sources: &SourceMap,
) -> Result<(), DiagnosticRenderError> {
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write_diagnostic(output, diagnostic, sources)?;
    }
    Ok(())
}

/// Renders one spanless failure in a complete `nocter.diagnostics` version-1 envelope.
///
/// # Errors
///
/// This renderer currently performs no fallible source projection, but shares the result type of
/// source-backed rendering so callers can keep one presentation path.
pub fn render_spanless_diagnostic_json(
    context: DiagnosticJsonContext<'_>,
    diagnostic: SpanlessDiagnostic<'_>,
) -> Result<String, DiagnosticRenderError> {
    render_diagnostics_json(context, false, |output| {
        write_spanless_diagnostic_json(output, diagnostic);
        Ok(())
    })
}

/// Appends one spanless diagnostic object for a domain-specific versioned envelope.
pub fn write_spanless_diagnostic_json(output: &mut String, diagnostic: SpanlessDiagnostic<'_>) {
    output.push_str("{\"code\":");
    write_json_string(output, diagnostic.code);
    output.push_str(",\"severity\":\"error\",\"message\":");
    write_json_string(output, diagnostic.message);
    output.push_str(",\"primary_span\":null,\"notes\":[],\"help\":");
    write_optional_string(output, diagnostic.help);
    output.push('}');
}

fn render_diagnostics_json(
    context: DiagnosticJsonContext<'_>,
    ok: bool,
    write_diagnostics: impl FnOnce(&mut String) -> Result<(), DiagnosticRenderError>,
) -> Result<String, DiagnosticRenderError> {
    let mut output = String::new();
    output.push_str("{\"schema\":\"nocter.diagnostics\",\"version\":1,\"ok\":");
    output.push_str(if ok { "true" } else { "false" });
    output.push_str(",\"command\":");
    write_json_string(&mut output, context.command);
    output.push_str(",\"target\":");
    write_optional_string(&mut output, context.target);
    output.push_str(",\"root\":");
    write_optional_string(&mut output, context.root);
    output.push_str(",\"root_absolute_path\":");
    write_optional_string(&mut output, context.root_absolute_path);
    output.push_str(",\"diagnostics\":[");
    write_diagnostics(&mut output)?;
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

    #[test]
    fn renders_spanless_failure_through_the_same_envelope() {
        assert_eq!(
            render_spanless_diagnostic_json(
                DiagnosticJsonContext::new("check", None, Some("missing.nct"), None),
                SpanlessDiagnostic::new("E0702", "cannot read missing.nct", None),
            )
            .unwrap(),
            concat!(
                "{\"schema\":\"nocter.diagnostics\",\"version\":1,\"ok\":false,",
                "\"command\":\"check\",\"target\":null,\"root\":\"missing.nct\",",
                "\"root_absolute_path\":null,\"diagnostics\":[{\"code\":\"E0702\",",
                "\"severity\":\"error\",\"message\":\"cannot read missing.nct\",",
                "\"primary_span\":null,\"notes\":[],\"help\":null}]}\n"
            )
        );
    }
}
