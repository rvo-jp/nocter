use std::fmt;

use nocter_checking::MissingConformanceMethods;
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceRole, SyntaxOrigin};
use nocter_syntax::{NodeKind, Punctuation, SyntaxElement, TokenKind};

use super::SemanticCodeAction;
use crate::presentation::required_conformance_method_presentation;
use crate::{AnalysisSnapshot, SemanticCompletionError, SemanticSourceEdit};

#[derive(Debug)]
pub enum ConformanceActionError {
    MissingRecovery,
    MissingConformance(nocter_model::ConformanceId),
    MissingDeclarationSite(nocter_model::DeclarationSiteId),
    MissingSourceBinding(nocter_model::ConformanceId),
    MissingSyntax(SourceId),
    InvalidConformanceNode,
    MissingClosingBrace,
    InvalidSourceRange { source: SourceId, range: TextRange },
    MissingMethodName(nocter_model::CallableId),
    Completion(SemanticCompletionError),
}

impl fmt::Display for ConformanceActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRecovery => {
                formatter.write_str("missing-method code action has no declaration recovery")
            }
            Self::MissingConformance(id) => {
                write!(formatter, "missing conformance declaration {id:?}")
            }
            Self::MissingDeclarationSite(id) => {
                write!(formatter, "missing conformance declaration site {id:?}")
            }
            Self::MissingSourceBinding(id) => {
                write!(formatter, "missing source binding for conformance {id:?}")
            }
            Self::MissingSyntax(source) => {
                write!(
                    formatter,
                    "missing syntax tree for conformance source {source}"
                )
            }
            Self::InvalidConformanceNode => {
                formatter.write_str("conformance source binding is not a conformance node")
            }
            Self::MissingClosingBrace => {
                formatter.write_str("conformance declaration has no closing brace")
            }
            Self::InvalidSourceRange { source, range } => write!(
                formatter,
                "invalid conformance insertion range in {source}: {}..{}",
                range.start().get(),
                range.end().get(),
            ),
            Self::MissingMethodName(id) => {
                write!(formatter, "required interface method {id:?} has no name")
            }
            Self::Completion(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConformanceActionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Completion(error) => Some(error),
            Self::MissingRecovery
            | Self::MissingConformance(_)
            | Self::MissingDeclarationSite(_)
            | Self::MissingSourceBinding(_)
            | Self::MissingSyntax(_)
            | Self::InvalidConformanceNode
            | Self::MissingClosingBrace
            | Self::InvalidSourceRange { .. }
            | Self::MissingMethodName(_) => None,
        }
    }
}

impl From<SemanticCompletionError> for ConformanceActionError {
    fn from(error: SemanticCompletionError) -> Self {
        Self::Completion(error)
    }
}

pub(super) fn missing_method_action(
    snapshot: &AnalysisSnapshot,
    requested_source: SourceId,
    diagnostic_code: &str,
    diagnostic_range: TextRange,
    missing: &MissingConformanceMethods,
) -> Result<Option<SemanticCodeAction>, ConformanceActionError> {
    let context = insertion_context(snapshot, requested_source, missing)?;
    let Some(context) = context else {
        return Ok(None);
    };
    let mut signatures = Vec::with_capacity(missing.required().len());
    for required in missing.required() {
        let presentation =
            required_conformance_method_presentation(context.recovery, required, context.module)
                .ok_or(ConformanceActionError::MissingMethodName(
                    required.interface_method(),
                ))?;
        signatures.push(presentation.code().to_owned());
    }
    let method_edit = method_insertion(context.source, context.closing, &signatures)?;
    let Some(mut edits) =
        abort_import_edits(snapshot, requested_source, method_edit.range().start())?
    else {
        return Ok(None);
    };
    edits.push(method_edit);
    Ok(Some(SemanticCodeAction {
        title: action_title(context.recovery, missing)?.into(),
        diagnostic_code: diagnostic_code.into(),
        diagnostic_range,
        edits: edits.into_boxed_slice(),
    }))
}

struct InsertionContext<'snapshot> {
    recovery: &'snapshot nocter_checking::DeclarationAnalysisRecovery,
    module: nocter_model::ModuleId,
    source: &'snapshot nocter_source::SourceFile,
    closing: ByteOffset,
}

fn insertion_context<'snapshot>(
    snapshot: &'snapshot AnalysisSnapshot,
    requested_source: SourceId,
    missing: &MissingConformanceMethods,
) -> Result<Option<InsertionContext<'snapshot>>, ConformanceActionError> {
    let recovery = snapshot
        .declaration_recovery()
        .ok_or(ConformanceActionError::MissingRecovery)?;
    let graph = recovery.graph();
    let conformance = graph
        .declarations()
        .conformances()
        .get(missing.conformance())
        .ok_or(ConformanceActionError::MissingConformance(
            missing.conformance(),
        ))?;
    let module = graph
        .declaration_sites()
        .get(conformance.site())
        .map(|site| site.module())
        .ok_or(ConformanceActionError::MissingDeclarationSite(
            conformance.site(),
        ))?;
    let origin = recovery
        .source_index()
        .bindings_for(SemanticEntity::Conformance(missing.conformance()))
        .iter()
        .find(|binding| binding.role() == SourceRole::Declaration)
        .map(|binding| binding.origin())
        .ok_or(ConformanceActionError::MissingSourceBinding(
            missing.conformance(),
        ))?;
    if origin.source() != requested_source {
        return Ok(None);
    }
    let SyntaxOrigin::Node(node) = origin.syntax() else {
        return Err(ConformanceActionError::InvalidConformanceNode);
    };
    let syntax = snapshot
        .syntax_trees()
        .iter()
        .find(|tree| tree.source() == requested_source)
        .ok_or(ConformanceActionError::MissingSyntax(requested_source))?;
    if syntax.node(node).map(nocter_syntax::SyntaxNode::kind) != Some(NodeKind::ConformDeclaration)
    {
        return Err(ConformanceActionError::InvalidConformanceNode);
    }
    let closing = syntax
        .children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Token(token)
                if token.kind() == TokenKind::Punctuation(Punctuation::RightBrace) =>
            {
                Some(*token)
            }
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
        .next_back()
        .ok_or(ConformanceActionError::MissingClosingBrace)?;
    let source = snapshot
        .sources()
        .get(requested_source)
        .ok_or(ConformanceActionError::MissingSyntax(requested_source))?;
    Ok(Some(InsertionContext {
        recovery,
        module,
        source,
        closing: closing.range().start(),
    }))
}

fn abort_import_edits(
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    offset: ByteOffset,
) -> Result<Option<Vec<SemanticSourceEdit>>, ConformanceActionError> {
    let completions = snapshot
        .semantic_completions(source, offset)
        .map_err(ConformanceActionError::Completion)?;
    for completion in &completions {
        if completion.label() != "abort" {
            continue;
        }
        match completion.automatic_import() {
            Some("std/process.abort") => {
                return Ok(Some(
                    completion
                        .additional_edits()
                        .iter()
                        .map(|edit| SemanticSourceEdit::new(source, edit.range(), edit.new_text()))
                        .collect(),
                ));
            }
            None => return Ok(Some(Vec::new())),
            Some(_) => {}
        }
    }
    Ok(None)
}

fn action_title(
    recovery: &nocter_checking::DeclarationAnalysisRecovery,
    missing: &MissingConformanceMethods,
) -> Result<String, ConformanceActionError> {
    if missing.required().len() == 1 {
        let required = &missing.required()[0];
        let method = recovery
            .graph()
            .declarations()
            .callables()
            .get(required.interface_method())
            .and_then(nocter_declarations::CallableDeclaration::name)
            .and_then(|name| recovery.graph().symbols().spelling(name))
            .ok_or(ConformanceActionError::MissingMethodName(
                required.interface_method(),
            ))?;
        Ok(format!("Implement required method `{method}`"))
    } else {
        Ok(format!(
            "Implement {} required methods",
            missing.required().len()
        ))
    }
}

fn method_insertion(
    source: &nocter_source::SourceFile,
    closing: ByteOffset,
    signatures: &[String],
) -> Result<SemanticSourceEdit, ConformanceActionError> {
    let line_start = source.text()[..usize::try_from(closing.get()).expect("bounded offset")]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let line_start = ByteOffset::new(u32::try_from(line_start).expect("bounded source length"));
    let leading_range = TextRange::new(line_start, closing);
    let leading =
        source
            .text_at(leading_range)
            .ok_or(ConformanceActionError::InvalidSourceRange {
                source: source.id(),
                range: leading_range,
            })?;
    let close_on_own_line = leading.bytes().all(|byte| matches!(byte, b' ' | b'\t'));
    let (offset, declaration_indent, prefix) = if close_on_own_line {
        (line_start, leading, "")
    } else {
        (closing, "", "\n")
    };
    let member_indent = format!("{declaration_indent}    ");
    let body_indent = format!("{member_indent}    ");
    let methods = signatures
        .iter()
        .map(|signature| {
            format!("{member_indent}{signature} {{\n{body_indent}abort()\n{member_indent}}}")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let text = format!("{prefix}{methods}\n");
    Ok(SemanticSourceEdit::new(
        source.id(),
        TextRange::new(offset, offset),
        text,
    ))
}

#[cfg(test)]
mod tests {
    use nocter_source::{ByteOffset, SourceMap, SourceName};

    use super::method_insertion;

    fn apply(source: &str, signatures: &[&str]) -> String {
        let mut sources = SourceMap::new();
        let id = sources
            .add_bytes(SourceName::new("index.nct"), source.as_bytes())
            .unwrap();
        let file = sources.get(id).unwrap();
        let closing = ByteOffset::new(u32::try_from(source.rfind('}').unwrap()).unwrap());
        let signatures = signatures
            .iter()
            .map(|signature| (*signature).to_owned())
            .collect::<Vec<_>>();
        let edit = method_insertion(file, closing, &signatures).unwrap();
        let offset = usize::try_from(edit.range().start().get()).unwrap();
        format!(
            "{}{}{}",
            &source[..offset],
            edit.new_text(),
            &source[offset..]
        )
    }

    #[test]
    fn empty_same_line_body_expands_without_an_extra_blank_line() {
        assert_eq!(
            apply(
                "conform Readable for Value {}\n",
                &["method &self.read(): i32"]
            ),
            concat!(
                "conform Readable for Value {\n",
                "    method &self.read(): i32 {\n",
                "        abort()\n",
                "    }\n",
                "}\n",
            )
        );
    }

    #[test]
    fn existing_members_keep_the_closing_brace_and_separate_generated_methods() {
        assert_eq!(
            apply(
                "conform Readable for Value {\n    type Item = i32\n}\n",
                &["method &self.read(): i32", "method &self.ready(): bool"]
            ),
            concat!(
                "conform Readable for Value {\n",
                "    type Item = i32\n",
                "    method &self.read(): i32 {\n",
                "        abort()\n",
                "    }\n",
                "\n",
                "    method &self.ready(): bool {\n",
                "        abort()\n",
                "    }\n",
                "}\n",
            )
        );
    }
}
