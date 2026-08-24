//! Filesystem-independent inspection of one normalized Nocter source.
//!
//! This crate owns source-inspection snapshots and their versioned public projections. It does not
//! discover packages, resolve names, select targets, or reconstruct a second syntax model.

use std::collections::HashMap;
use std::fmt;
use std::fmt::Write;

use nocter_diagnostics::{
    DiagnosticRenderError, SourceDiagnostic, lexical_diagnostic, syntax_diagnostics,
    write_json_string, write_source_diagnostic_items_json,
};
use nocter_source::{SourceError, SourceFile, SourceMap, SourceName, TextRange};
use nocter_syntax::{ExpectedSyntax, ParseGoal, SyntaxElement, SyntaxTree, TokenKind, parse};

mod formatter;
mod syntax_tokens;

/// Formatting failure selected before any filesystem publication.
#[derive(Debug)]
pub enum FormatError {
    Diagnostics(Box<[SourceDiagnostic]>),
    ChangedSyntax,
}

impl FormatError {
    #[must_use]
    pub const fn diagnostics(&self) -> Option<&[SourceDiagnostic]> {
        match self {
            Self::Diagnostics(diagnostics) => Some(diagnostics),
            Self::ChangedSyntax => None,
        }
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagnostics(_) => formatter.write_str("source cannot be formatted"),
            Self::ChangedSyntax => formatter.write_str("formatter output changed concrete syntax"),
        }
    }
}

impl std::error::Error for FormatError {}

/// The grammar root selected for one standalone inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionGoal {
    SourceFile,
}

impl From<InspectionGoal> for ParseGoal {
    fn from(goal: InspectionGoal) -> Self {
        match goal {
            InspectionGoal::SourceFile => Self::SourceFile,
        }
    }
}

/// One immutable lexer/parser snapshot used by every source-inspection projection.
#[derive(Clone, Debug)]
pub struct SourceInspection {
    sources: SourceMap,
    syntax: SyntaxTree,
}

impl SourceInspection {
    /// Normalizes and parses one in-memory source without filesystem or package access.
    ///
    /// # Errors
    ///
    /// Returns the source normalization failure before allocating a syntax snapshot.
    ///
    /// # Panics
    ///
    /// Panics only if `SourceMap` violates its contract by losing the identity it just allocated.
    pub fn new(name: SourceName, bytes: &[u8], goal: InspectionGoal) -> Result<Self, SourceError> {
        let mut sources = SourceMap::new();
        let source = sources.add_bytes(name, bytes)?;
        let syntax = parse(
            sources
                .get(source)
                .expect("new inspection source remains in its source map"),
            goal.into(),
        );
        Ok(Self { sources, syntax })
    }

    #[must_use]
    pub const fn sources(&self) -> &SourceMap {
        &self.sources
    }

    #[must_use]
    pub const fn syntax(&self) -> &SyntaxTree {
        &self.syntax
    }

    #[must_use]
    pub fn tokens_succeeded(&self) -> bool {
        self.syntax.lexed().diagnostics().is_empty()
    }

    #[must_use]
    pub fn ast_succeeded(&self) -> bool {
        !self.syntax.has_errors()
    }

    /// Formats the retained source through the concrete-syntax layout model.
    ///
    /// # Errors
    ///
    /// Returns source diagnostics when parsing failed or comments prevent a lossless rewrite, and
    /// an integrity failure when reparsing the candidate output changes concrete syntax.
    ///
    /// # Panics
    ///
    /// Panics only if the inspection snapshot loses its own source identity.
    pub fn format(&self) -> Result<String, FormatError> {
        let source = self
            .sources
            .get(self.syntax.source())
            .expect("inspection source remains in its source map");
        formatter::format(source, &self.syntax)
    }

    /// Renders the `nocter.tokens` version-1 envelope from the retained lexer snapshot.
    ///
    /// # Errors
    ///
    /// Returns an integrity error if a retained span does not belong to the source snapshot.
    pub fn render_tokens_json(&self) -> Result<String, DiagnosticRenderError> {
        let source = self.source()?;
        let diagnostics = self
            .syntax
            .lexed()
            .diagnostics()
            .iter()
            .copied()
            .map(lexical_diagnostic)
            .collect::<Vec<_>>();
        let mut output = String::new();
        write_envelope_start(
            &mut output,
            "nocter.tokens",
            diagnostics.is_empty(),
            source,
            &diagnostics,
            &self.sources,
        )?;
        output.push_str(",\"tokens\":[");
        for (id, token) in self.syntax.lexed().tokens().iter().copied().enumerate() {
            if id != 0 {
                output.push(',');
            }
            let range = token.span().range();
            write!(output, "{{\"id\":{id},\"kind\":").expect("writing to String cannot fail");
            write_json_string(&mut output, token.kind().as_str());
            output.push_str(",\"text\":");
            write_json_string(&mut output, text_at(source, range)?);
            write_range(&mut output, range);
            output.push_str(",\"joint_to_next\":");
            output.push_str(if token.is_joint_to_next() {
                "true"
            } else {
                "false"
            });
            output.push('}');
        }
        output.push_str("],\"comments\":[");
        for (id, comment) in self.syntax.lexed().comments().iter().copied().enumerate() {
            if id != 0 {
                output.push(',');
            }
            let range = comment.span().range();
            write!(output, "{{\"id\":{id},\"kind\":").expect("writing to String cannot fail");
            write_json_string(&mut output, comment.kind().as_str());
            output.push_str(",\"text\":");
            write_json_string(&mut output, text_at(source, range)?);
            write_range(&mut output, range);
            output.push('}');
        }
        output.push_str("]}\n");
        Ok(output)
    }

    /// Renders the `nocter.ast` version-1 flat concrete-syntax envelope.
    ///
    /// # Errors
    ///
    /// Returns an integrity error if a retained span does not belong to the source snapshot.
    pub fn render_ast_json(&self) -> Result<String, DiagnosticRenderError> {
        let source = self.source()?;
        let diagnostics = syntax_diagnostics(std::slice::from_ref(&self.syntax));
        let syntax_tokens = syntax_tokens::ordered(&self.syntax);
        let token_ids = syntax_tokens
            .iter()
            .copied()
            .enumerate()
            .map(|(id, token)| (token, id))
            .collect::<HashMap<_, _>>();

        let mut output = String::new();
        write_envelope_start(
            &mut output,
            "nocter.ast",
            diagnostics.is_empty(),
            source,
            &diagnostics,
            &self.sources,
        )?;
        output.push_str(",\"documentation\":");
        write_optional_json_string(&mut output, self.syntax.file_documentation());
        write!(
            output,
            ",\"root\":{},\"nodes\":[",
            self.syntax.root_id().index()
        )
        .expect("writing to String cannot fail");
        for (position, (node_id, node)) in self.syntax.nodes().enumerate() {
            if position != 0 {
                output.push(',');
            }
            write!(output, "{{\"id\":{},\"kind\":", node_id.index())
                .expect("writing to String cannot fail");
            write_json_string(&mut output, node.kind().as_str());
            write_range(&mut output, node.range());
            output.push_str(",\"documentation\":");
            write_optional_json_string(&mut output, self.syntax.documentation(node_id));
            output.push_str(",\"children\":[");
            for (child_index, child) in self.syntax.children(node_id).iter().enumerate() {
                if child_index != 0 {
                    output.push(',');
                }
                match child {
                    SyntaxElement::Node(child) => {
                        write!(output, "{{\"kind\":\"node\",\"id\":{}}}", child.index())
                            .expect("writing to String cannot fail");
                    }
                    SyntaxElement::Token(child) => {
                        let id = token_ids
                            .get(child)
                            .ok_or(DiagnosticRenderError::UnknownSource(child.source()))?;
                        write!(output, "{{\"kind\":\"token\",\"id\":{id}}}")
                            .expect("writing to String cannot fail");
                    }
                    SyntaxElement::Missing(child) => {
                        output.push_str("{\"kind\":\"missing\",\"expected\":");
                        write_expected(&mut output, child.expected());
                        write_range(&mut output, child.span().range());
                        output.push('}');
                    }
                }
            }
            output.push_str("]}");
        }
        output.push_str("],\"tokens\":[");
        for (id, token) in syntax_tokens.into_iter().enumerate() {
            if id != 0 {
                output.push(',');
            }
            write!(
                output,
                "{{\"id\":{id},\"lexical_id\":{},\"kind\":",
                token.lexical().index()
            )
            .expect("writing to String cannot fail");
            write_json_string(&mut output, token.kind().as_str());
            output.push_str(",\"text\":");
            write_json_string(&mut output, text_at(source, token.range())?);
            write_range(&mut output, token.range());
            output.push('}');
        }
        output.push_str("]}\n");
        Ok(output)
    }

    fn source(&self) -> Result<&SourceFile, DiagnosticRenderError> {
        self.sources
            .get(self.syntax.source())
            .ok_or(DiagnosticRenderError::UnknownSource(self.syntax.source()))
    }
}

fn write_optional_json_string(output: &mut String, value: Option<&str>) {
    if let Some(value) = value {
        write_json_string(output, value);
    } else {
        output.push_str("null");
    }
}

fn write_envelope_start(
    output: &mut String,
    schema: &str,
    ok: bool,
    source: &SourceFile,
    diagnostics: &[SourceDiagnostic],
    sources: &SourceMap,
) -> Result<(), DiagnosticRenderError> {
    output.push_str("{\"schema\":");
    write_json_string(output, schema);
    output.push_str(",\"version\":1,\"ok\":");
    output.push_str(if ok { "true" } else { "false" });
    output.push_str(",\"source\":{\"path\":");
    write_json_string(output, source.name().as_str());
    write!(output, ",\"byte_length\":{}}}", source.len().get())
        .expect("writing to String cannot fail");
    output.push_str(",\"diagnostics\":[");
    write_source_diagnostic_items_json(output, diagnostics, sources)?;
    output.push(']');
    Ok(())
}

fn text_at(source: &SourceFile, range: TextRange) -> Result<&str, DiagnosticRenderError> {
    source
        .text_at(range)
        .ok_or(DiagnosticRenderError::InvalidRange {
            source: source.id(),
            range,
        })
}

fn write_range(output: &mut String, range: TextRange) {
    write!(
        output,
        ",\"start_byte\":{},\"end_byte\":{}",
        range.start().get(),
        range.end().get()
    )
    .expect("writing to String cannot fail");
}

fn write_expected(output: &mut String, expected: ExpectedSyntax) {
    output.push_str("{\"kind\":");
    let (kind, text) = expected_parts(expected);
    write_json_string(output, kind);
    output.push_str(",\"text\":");
    match text {
        Some(text) => write_json_string(output, text),
        None => output.push_str("null"),
    }
    output.push('}');
}

fn expected_parts(expected: ExpectedSyntax) -> (&'static str, Option<&'static str>) {
    match expected {
        ExpectedSyntax::Token(token) => token_expected_parts(token),
        ExpectedSyntax::Keyword(keyword) => ("keyword", Some(keyword.as_str())),
        ExpectedSyntax::Punctuation(punctuation) => ("punctuation", Some(punctuation.as_str())),
        ExpectedSyntax::Contextual(text) => ("contextual_keyword", Some(text)),
        ExpectedSyntax::Name => ("name", None),
        ExpectedSyntax::Visibility => ("visibility", None),
        ExpectedSyntax::PackageDirectiveName => ("package_directive_name", None),
        ExpectedSyntax::DirectiveValue => ("directive_value", None),
        ExpectedSyntax::StringLiteral => ("string_literal", None),
        ExpectedSyntax::ModuleSegment => ("module_segment", None),
        ExpectedSyntax::Type => ("type", None),
        ExpectedSyntax::Parameter => ("parameter", None),
        ExpectedSyntax::TargetableItem => ("targetable_declaration", None),
        ExpectedSyntax::Item => ("declaration", None),
        ExpectedSyntax::DeclarationMember => ("declaration_member", None),
        ExpectedSyntax::DeclarationTypePattern => ("declaration_type_pattern", None),
        ExpectedSyntax::Receiver => ("receiver", None),
        ExpectedSyntax::Block => ("block", None),
        ExpectedSyntax::LiteralShape => ("literal_shape", None),
        ExpectedSyntax::Expression => ("expression", None),
        ExpectedSyntax::AssignmentTarget => ("assignment_target", None),
        ExpectedSyntax::EnumPattern => ("enum_pattern", None),
        ExpectedSyntax::ClosureHead => ("closure_head", None),
        ExpectedSyntax::Predicate => ("predicate", None),
        ExpectedSyntax::Capability => ("capability", None),
        ExpectedSyntax::Newline => ("newline", None),
    }
}

fn token_expected_parts(token: TokenKind) -> (&'static str, Option<&'static str>) {
    match token {
        TokenKind::Keyword(keyword) => ("keyword", Some(keyword.as_str())),
        TokenKind::Punctuation(punctuation) => ("punctuation", Some(punctuation.as_str())),
        other => (other.as_str(), None),
    }
}

#[cfg(test)]
mod tests {
    use nocter_source::SourceName;

    use super::{InspectionGoal, SourceInspection};

    #[test]
    fn tokens_preserve_comments_text_jointness_and_lexer_diagnostics() {
        let inspection = SourceInspection::new(
            SourceName::new("/tmp/app.nct"),
            b"let x=1 // note\n@",
            InspectionGoal::SourceFile,
        )
        .unwrap();

        let json = inspection.render_tokens_json().unwrap();

        assert!(json.starts_with(
            "{\"schema\":\"nocter.tokens\",\"version\":1,\"ok\":false,\"source\":{\"path\":\"/tmp/app.nct\",\"byte_length\":17}"
        ));
        assert!(json.contains("\"kind\":\"punctuation\",\"text\":\"=\""));
        assert!(json.contains("\"joint_to_next\":true"));
        assert!(json.contains("\"kind\":\"line\",\"text\":\"// note\""));
        assert!(json.contains("\"code\":\"E0100\""));
        assert!(json.ends_with('\n'));
    }

    #[test]
    fn ast_is_flat_and_retains_parser_missing_elements() {
        let inspection = SourceInspection::new(
            SourceName::new("/tmp/app.nct"),
            b"func main(: void\n",
            InspectionGoal::SourceFile,
        )
        .unwrap();

        let json = inspection.render_ast_json().unwrap();

        assert!(json.starts_with(
            "{\"schema\":\"nocter.ast\",\"version\":1,\"ok\":false,\"source\":{\"path\":\"/tmp/app.nct\""
        ));
        assert!(json.contains("\"root\":"));
        assert!(json.contains("\"nodes\":["));
        assert!(json.contains("\"kind\":\"missing\""));
        assert!(json.contains("\"tokens\":["));
        assert!(json.ends_with('\n'));
    }

    #[test]
    fn ast_projects_syntax_owned_file_and_item_documentation() {
        let inspection = SourceInspection::new(
            SourceName::new("/tmp/app.nct"),
            b"//! Module API.\n\n/// Starts the application.\nfunc main(): void { return }\n",
            InspectionGoal::SourceFile,
        )
        .unwrap();

        let json = inspection.render_ast_json().unwrap();

        assert!(json.contains("\"documentation\":\"Module API.\""));
        assert!(json.contains("\"documentation\":\"Starts the application.\""));
        assert!(json.contains("\"documentation\":null"));
    }

    #[test]
    fn source_goal_exposes_package_directives_in_the_unified_root() {
        let inspection = SourceInspection::new(
            SourceName::new("/tmp/index.nct"),
            b"#package: { name: \"demo\", version: \"0.0.0\", }\n",
            InspectionGoal::SourceFile,
        )
        .unwrap();

        let json = inspection.render_ast_json().unwrap();

        assert!(json.contains("\"kind\":\"source_file\""));
        assert!(json.contains("\"kind\":\"package_directive\""));
    }
}
