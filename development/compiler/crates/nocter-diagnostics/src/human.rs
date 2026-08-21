use std::fmt::{self, Write};

use nocter_source::{ByteOffset, SourceFile, SourceId, SourceMap, TextRange};

use crate::projection::project_origin;
use crate::{DiagnosticOrigin, SourceDiagnostic};

/// Renders one phase-owned source diagnostic without inspecting its semantic error type.
///
/// Coordinates are one-based character columns over normalized UTF-8 source. Every source line
/// intersected by a multi-line range is shown. Tabs are expanded to four-column stops in both the
/// displayed source and its marker.
///
/// # Errors
///
/// Returns a typed projection failure when an origin does not belong to the supplied source map or
/// its normalized range is not a UTF-8 boundary in that source.
pub fn render_source_diagnostic(
    diagnostic: &SourceDiagnostic,
    sources: &SourceMap,
) -> Result<String, DiagnosticRenderError> {
    let mut output = String::new();
    writeln!(
        output,
        "error[{}]: {}",
        diagnostic.code(),
        diagnostic.message()
    )
    .expect("writing to String cannot fail");
    render_origin(&mut output, diagnostic.primary(), sources)?;
    for note in diagnostic.notes() {
        writeln!(output, "note: {}", note.message()).expect("writing to String cannot fail");
        render_origin(&mut output, note.origin(), sources)?;
    }
    if let Some(help) = diagnostic.help() {
        writeln!(output, "help: {help}").expect("writing to String cannot fail");
    }
    Ok(output)
}

fn render_origin(
    output: &mut String,
    origin: DiagnosticOrigin,
    sources: &SourceMap,
) -> Result<(), DiagnosticRenderError> {
    let projected = project_origin(origin, sources)?;
    let source = projected.source;
    let range = projected.range;
    let start = projected.start;
    let start_line = start.line();
    let start_column = character_column(source, start_line, range.start())?;
    writeln!(
        output,
        "  --> {}:{}:{}",
        source.name(),
        start_line + 1,
        start_column + 1
    )
    .expect("writing to String cannot fail");

    let end_line = selected_end_line(source, range)?;
    let width = decimal_width(end_line + 1);
    writeln!(output, "{:width$} |", "", width = width).expect("writing to String cannot fail");
    for line in start_line..=end_line {
        render_line(output, source, range, line, width)?;
    }
    Ok(())
}

fn render_line(
    output: &mut String,
    source: &SourceFile,
    range: TextRange,
    line: u32,
    number_width: usize,
) -> Result<(), DiagnosticRenderError> {
    let line_range =
        source
            .lines()
            .line_range(line)
            .ok_or(DiagnosticRenderError::InvalidRange {
                source: source.id(),
                range,
            })?;
    let full_text =
        source
            .text_at(line_range)
            .ok_or(DiagnosticRenderError::InvalidUtf8Boundary {
                source: source.id(),
                offset: line_range.start(),
            })?;
    let text = full_text.strip_suffix('\n').unwrap_or(full_text);
    let displayed_text = expand_tabs(text);
    writeln!(
        output,
        "{:>width$} | {displayed_text}",
        line + 1,
        width = number_width
    )
    .expect("writing to String cannot fail");

    let content_end = line_range.start().get()
        + u32::try_from(text.len()).expect("source length already fits u32");
    let selected_start = range.start().get().max(line_range.start().get());
    let selected_end = range.end().get().min(content_end);
    let start_in_line = selected_start.saturating_sub(line_range.start().get());
    let end_in_line = selected_end.saturating_sub(line_range.start().get());
    let start = usize::try_from(start_in_line).expect("source offsets fit usize");
    let end = usize::try_from(end_in_line).expect("source offsets fit usize");
    let prefix =
        text.get(..start.min(text.len()))
            .ok_or(DiagnosticRenderError::InvalidUtf8Boundary {
                source: source.id(),
                offset: ByteOffset::new(selected_start),
            })?;
    let selected = text.get(start.min(text.len())..end.min(text.len())).ok_or(
        DiagnosticRenderError::InvalidUtf8Boundary {
            source: source.id(),
            offset: ByteOffset::new(selected_end),
        },
    )?;
    let marker_start = display_width(prefix);
    let marker_width = display_width(selected).max(1);
    writeln!(
        output,
        "{:width$} | {}{}",
        "",
        " ".repeat(marker_start),
        "^".repeat(marker_width),
        width = number_width
    )
    .expect("writing to String cannot fail");
    Ok(())
}

fn selected_end_line(source: &SourceFile, range: TextRange) -> Result<u32, DiagnosticRenderError> {
    let end = if range.is_empty() {
        range.end()
    } else {
        ByteOffset::new(range.end().get() - 1)
    };
    source
        .lines()
        .line_column(end)
        .map(nocter_source::LineColumn::line)
        .ok_or(DiagnosticRenderError::InvalidRange {
            source: source.id(),
            range,
        })
}

fn character_column(
    source: &SourceFile,
    line: u32,
    offset: ByteOffset,
) -> Result<usize, DiagnosticRenderError> {
    let line_range =
        source
            .lines()
            .line_range(line)
            .ok_or(DiagnosticRenderError::InvalidRange {
                source: source.id(),
                range: TextRange::empty(offset),
            })?;
    let prefix = source
        .text_at(TextRange::new(line_range.start(), offset))
        .ok_or(DiagnosticRenderError::InvalidUtf8Boundary {
            source: source.id(),
            offset,
        })?;
    Ok(display_width(prefix))
}

fn display_width(text: &str) -> usize {
    text.chars().fold(0, |column, character| {
        if character == '\t' {
            column + (4 - column % 4)
        } else {
            column + 1
        }
    })
}

fn expand_tabs(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut column = 0;
    for character in text.chars() {
        if character == '\t' {
            let spaces = 4 - column % 4;
            output.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
        } else {
            output.push(character);
            column += 1;
        }
    }
    output
}

fn decimal_width(value: u32) -> usize {
    value.to_string().len()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticRenderError {
    NonUnicodePath,
    UnknownSource(SourceId),
    InvalidRange {
        source: SourceId,
        range: TextRange,
    },
    InvalidUtf8Boundary {
        source: SourceId,
        offset: ByteOffset,
    },
}

impl fmt::Display for DiagnosticRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUnicodePath => formatter.write_str("diagnostic path is not Unicode"),
            Self::UnknownSource(source) => {
                write!(formatter, "diagnostic refers to unknown {source}")
            }
            Self::InvalidRange { source, range } => write!(
                formatter,
                "diagnostic range {}..{} is outside {source}",
                range.start().get(),
                range.end().get()
            ),
            Self::InvalidUtf8Boundary { source, offset } => write!(
                formatter,
                "diagnostic offset {} is not a UTF-8 boundary in {source}",
                offset.get()
            ),
        }
    }
}

impl std::error::Error for DiagnosticRenderError {}

#[cfg(test)]
mod tests {
    use nocter_source::SourceName;
    use nocter_source_index::SourceOrigin;
    use nocter_syntax::{
        NodeId, NodeKind, ParseGoal, SyntaxElement, SyntaxToken, SyntaxTree, parse,
    };

    use super::*;
    use crate::{DiagnosticNote, SourceDiagnostic};

    #[test]
    fn renders_unicode_multiline_primary_notes_and_help_from_one_envelope() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(
                SourceName::new("src/app.nct"),
                "//! café\nfunc main(): i32 {\n\tlet value = 1\n\treturn value\n}\n".as_bytes(),
            )
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::ModuleSource);
        let block = find_node(&tree, tree.root_id(), NodeKind::Block).unwrap();
        let primary = SourceOrigin::from_node(&tree, block).unwrap();
        let name = find_token(&tree, tree.root_id(), &sources, "value").unwrap();
        let note = SourceOrigin::from_token(&tree, name).unwrap();
        let diagnostic = SourceDiagnostic::new(
            "E9999",
            "example failure",
            primary,
            [DiagnosticNote::new("value declared here", note)],
            Some("change the example"),
        );

        let rendered = render_source_diagnostic(&diagnostic, &sources).unwrap();
        assert!(rendered.starts_with("error[E9999]: example failure\n"));
        assert!(rendered.contains("  --> src/app.nct:2:18\n"));
        assert!(rendered.contains("3 |     let value = 1\n"));
        assert!(rendered.contains("  | ^^^^^^^^^^^^^^^^^\n"));
        assert!(rendered.contains("4 |     return value\n"));
        assert!(rendered.contains("note: value declared here\n"));
        assert!(rendered.contains("  --> src/app.nct:3:9\n"));
        assert!(rendered.ends_with("help: change the example\n"));
    }

    #[test]
    fn empty_ranges_and_tabs_share_one_character_coordinate_system() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("tabbed.nct"), b"\tvalue\n")
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::ModuleSource);
        let token = find_token(&tree, tree.root_id(), &sources, "value").unwrap();
        let origin = SourceOrigin::from_token(&tree, token).unwrap();
        let diagnostic = SourceDiagnostic::new("E9998", "tabbed", origin, [], None::<&str>);

        assert_eq!(
            render_source_diagnostic(&diagnostic, &sources).unwrap(),
            "error[E9998]: tabbed\n  --> tabbed.nct:1:5\n  |\n1 |     value\n  |     ^^^^^\n"
        );
    }

    fn find_node(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
        if tree.node(node)?.kind() == kind {
            return Some(node);
        }
        tree.children(node)
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Node(child) => find_node(tree, *child, kind),
                SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
            })
    }

    fn find_token(
        tree: &SyntaxTree,
        node: NodeId,
        sources: &SourceMap,
        text: &str,
    ) -> Option<SyntaxToken> {
        tree.children(node)
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Node(child) => find_token(tree, *child, sources, text),
                SyntaxElement::Token(token) => sources
                    .get(token.source())?
                    .text_at(token.range())
                    .is_some_and(|candidate| candidate == text)
                    .then_some(*token),
                SyntaxElement::Missing(_) => None,
            })
    }
}
