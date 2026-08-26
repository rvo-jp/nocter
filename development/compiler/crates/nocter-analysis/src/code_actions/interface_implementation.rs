use std::fmt;

use nocter_checking::MissingInterfaceImplementationMethods;
use nocter_declarations::StandardDeclarationRole;
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceRole};
use nocter_syntax::{NodeKind, Punctuation, SyntaxElement, SyntaxOrigin, TokenKind};

use super::SemanticCodeAction;
use crate::presentation::required_interface_implementation_method_presentation;
use crate::{AnalysisSnapshot, SemanticCompletionError, SemanticSourceEdit};

#[derive(Debug)]
pub enum InterfaceImplementationActionError {
    MissingRecovery,
    MissingInterfaceImplementation(nocter_model::InterfaceImplementationId),
    MissingDeclarationSite(nocter_model::DeclarationSiteId),
    MissingSourceBinding(nocter_model::InterfaceImplementationId),
    MissingInstanceSource(nocter_model::InstanceId),
    MissingSyntax(SourceId),
    InvalidInstanceNode,
    MissingClosingBrace,
    InvalidSourceRange { source: SourceId, range: TextRange },
    MissingMethodName(nocter_model::CallableId),
    Completion(SemanticCompletionError),
}

impl fmt::Display for InterfaceImplementationActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRecovery => {
                formatter.write_str("missing-method code action has no declaration recovery")
            }
            Self::MissingInterfaceImplementation(id) => {
                write!(formatter, "missing interface implementation {id:?}")
            }
            Self::MissingDeclarationSite(id) => {
                write!(formatter, "missing interface implementation site {id:?}")
            }
            Self::MissingSourceBinding(id) => {
                write!(
                    formatter,
                    "missing source binding for interface implementation {id:?}"
                )
            }
            Self::MissingInstanceSource(id) => {
                write!(formatter, "missing editable source for instance {id:?}")
            }
            Self::MissingSyntax(source) => {
                write!(
                    formatter,
                    "missing syntax tree for interface implementation source {source}"
                )
            }
            Self::InvalidInstanceNode => {
                formatter.write_str("editable instance source binding is not an instance node")
            }
            Self::MissingClosingBrace => {
                formatter.write_str("editable instance declaration has no closing brace")
            }
            Self::InvalidSourceRange { source, range } => write!(
                formatter,
                "invalid interface implementation insertion range in {source}: {}..{}",
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

impl std::error::Error for InterfaceImplementationActionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Completion(error) => Some(error),
            Self::MissingRecovery
            | Self::MissingInterfaceImplementation(_)
            | Self::MissingDeclarationSite(_)
            | Self::MissingSourceBinding(_)
            | Self::MissingInstanceSource(_)
            | Self::MissingSyntax(_)
            | Self::InvalidInstanceNode
            | Self::MissingClosingBrace
            | Self::InvalidSourceRange { .. }
            | Self::MissingMethodName(_) => None,
        }
    }
}

impl From<SemanticCompletionError> for InterfaceImplementationActionError {
    fn from(error: SemanticCompletionError) -> Self {
        Self::Completion(error)
    }
}

pub(super) fn missing_method_action(
    snapshot: &AnalysisSnapshot,
    requested_source: SourceId,
    diagnostic_code: &str,
    diagnostic_range: TextRange,
    missing: &MissingInterfaceImplementationMethods,
) -> Result<Option<SemanticCodeAction>, InterfaceImplementationActionError> {
    let context = insertion_context(snapshot, requested_source, missing)?;
    let Some(context) = context else {
        return Ok(None);
    };
    let mut signatures = Vec::with_capacity(missing.required().len());
    for required in missing.required() {
        let presentation = required_interface_implementation_method_presentation(
            context.recovery,
            required,
            context.module,
        )
        .ok_or(InterfaceImplementationActionError::MissingMethodName(
            required.interface_method(),
        ))?;
        signatures.push(presentation.code().to_owned());
    }
    let Some(abort) = context
        .recovery
        .standard_semantics()
        .and_then(|standard| standard.callable(StandardDeclarationRole::ProcessAbort))
    else {
        return Ok(None);
    };
    let probe_offset = context.closing;
    let Some(mut terminator) =
        terminator_import_edits(snapshot, context.source.id(), probe_offset, abort)?
    else {
        return Ok(None);
    };
    let method_edit = method_insertion(
        context.source,
        context.opening,
        context.closing,
        &signatures,
        &terminator.name,
    )?;
    terminator.edits.push(method_edit);
    Ok(Some(SemanticCodeAction {
        title: action_title(context.recovery, missing)?.into(),
        diagnostic_code: diagnostic_code.into(),
        diagnostic_range,
        edits: terminator.edits.into_boxed_slice(),
    }))
}

struct InsertionContext<'snapshot> {
    recovery: &'snapshot nocter_checking::DeclarationAnalysisRecovery,
    module: nocter_model::ModuleId,
    source: &'snapshot nocter_source::SourceFile,
    opening: ByteOffset,
    closing: ByteOffset,
}

fn insertion_context<'snapshot>(
    snapshot: &'snapshot AnalysisSnapshot,
    requested_source: SourceId,
    missing: &MissingInterfaceImplementationMethods,
) -> Result<Option<InsertionContext<'snapshot>>, InterfaceImplementationActionError> {
    let recovery = snapshot
        .semantic_authority()
        .and_then(|authority| authority.declaration_analysis())
        .ok_or(InterfaceImplementationActionError::MissingRecovery)?;
    let graph = recovery.graph();
    let interface_implementation = graph
        .declarations()
        .interface_implementations()
        .get(missing.interface_implementation())
        .ok_or(
            InterfaceImplementationActionError::MissingInterfaceImplementation(
                missing.interface_implementation(),
            ),
        )?;
    let module = graph
        .declaration_sites()
        .get(interface_implementation.site())
        .map(|site| site.module())
        .ok_or(InterfaceImplementationActionError::MissingDeclarationSite(
            interface_implementation.site(),
        ))?;
    let bindings = recovery
        .source_index()
        .bindings_for(SemanticEntity::InterfaceImplementation(
            missing.interface_implementation(),
        ));
    let declaration = bindings
        .iter()
        .find(|binding| binding.role() == SourceRole::Declaration)
        .map(|binding| binding.origin())
        .ok_or(InterfaceImplementationActionError::MissingSourceBinding(
            missing.interface_implementation(),
        ))?;
    if declaration.source() != requested_source {
        return Ok(None);
    }
    let instance_origin = editable_instance_origin(recovery, interface_implementation.owner())?;
    let SyntaxOrigin::Node(instance) = instance_origin.syntax() else {
        return Err(InterfaceImplementationActionError::InvalidInstanceNode);
    };
    let insertion_source = instance_origin.source();
    let syntax = snapshot
        .syntax_trees()
        .iter()
        .find(|tree| tree.source() == insertion_source)
        .ok_or(InterfaceImplementationActionError::MissingSyntax(
            insertion_source,
        ))?;
    if syntax.node(instance).map(nocter_syntax::SyntaxNode::kind)
        != Some(NodeKind::InstanceDeclaration)
    {
        return Err(InterfaceImplementationActionError::InvalidInstanceNode);
    }
    let braces = syntax
        .children(instance)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Token(token)
                if matches!(
                    token.kind(),
                    TokenKind::Punctuation(Punctuation::LeftBrace | Punctuation::RightBrace)
                ) =>
            {
                Some(*token)
            }
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
        .collect::<Vec<_>>();
    let opening = braces
        .iter()
        .find(|token| token.kind() == TokenKind::Punctuation(Punctuation::LeftBrace))
        .ok_or(InterfaceImplementationActionError::MissingClosingBrace)?;
    let closing = braces
        .iter()
        .rfind(|token| token.kind() == TokenKind::Punctuation(Punctuation::RightBrace))
        .ok_or(InterfaceImplementationActionError::MissingClosingBrace)?;
    let source = snapshot.sources().get(insertion_source).ok_or(
        InterfaceImplementationActionError::MissingSyntax(insertion_source),
    )?;
    Ok(Some(InsertionContext {
        recovery,
        module,
        source,
        opening: opening.range().end(),
        closing: closing.range().start(),
    }))
}

/// Selects an existing implementation fragment by the source roles frozen during lowering.
///
/// The `impl` fact belongs to the public contract, while method bodies belong in an implementation
/// fragment when one exists. Analysis therefore consumes the semantic instance's source bindings
/// and never reconstructs module topology.
fn editable_instance_origin(
    recovery: &nocter_checking::DeclarationAnalysisRecovery,
    owner: nocter_model::InstanceId,
) -> Result<nocter_source_index::SourceOrigin, InterfaceImplementationActionError> {
    recovery
        .source_index()
        .bindings_for(SemanticEntity::Instance(owner))
        .iter()
        .find(|binding| binding.role() == SourceRole::Implementation)
        .or_else(|| {
            recovery
                .source_index()
                .bindings_for(SemanticEntity::Instance(owner))
                .iter()
                .find(|binding| binding.role() == SourceRole::Declaration)
        })
        .map(|binding| binding.origin())
        .ok_or(InterfaceImplementationActionError::MissingInstanceSource(
            owner,
        ))
}

struct TerminatorEditPlan {
    name: Box<str>,
    edits: Vec<SemanticSourceEdit>,
}

fn terminator_import_edits(
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    offset: ByteOffset,
    terminator: nocter_model::CallableId,
) -> Result<Option<TerminatorEditPlan>, InterfaceImplementationActionError> {
    let completions = snapshot
        .semantic_completions(source, offset)
        .map_err(InterfaceImplementationActionError::Completion)?;
    for completion in &completions {
        if completion.entity() != Some(SemanticEntity::Callable(terminator)) {
            continue;
        }
        match completion.automatic_import() {
            Some(_) => {
                return Ok(Some(TerminatorEditPlan {
                    name: completion.label().into(),
                    edits: completion
                        .additional_edits()
                        .iter()
                        .map(|edit| SemanticSourceEdit::new(source, edit.range(), edit.new_text()))
                        .collect(),
                }));
            }
            None => {
                return Ok(Some(TerminatorEditPlan {
                    name: completion.label().into(),
                    edits: Vec::new(),
                }));
            }
        }
    }
    Ok(None)
}

fn action_title(
    recovery: &nocter_checking::DeclarationAnalysisRecovery,
    missing: &MissingInterfaceImplementationMethods,
) -> Result<String, InterfaceImplementationActionError> {
    if missing.required().len() == 1 {
        let required = &missing.required()[0];
        let method = recovery
            .graph()
            .declarations()
            .callables()
            .get(required.interface_method())
            .and_then(nocter_declarations::CallableDeclaration::name)
            .and_then(|name| recovery.graph().symbols().spelling(name))
            .ok_or(InterfaceImplementationActionError::MissingMethodName(
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
    opening: ByteOffset,
    closing: ByteOffset,
    signatures: &[String],
    terminator: &str,
) -> Result<SemanticSourceEdit, InterfaceImplementationActionError> {
    let line_start = source.text()[..usize::try_from(closing.get()).expect("bounded offset")]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let line_start = ByteOffset::new(u32::try_from(line_start).expect("bounded source length"));
    let leading_range = TextRange::new(line_start, closing);
    let leading = source.text_at(leading_range).ok_or(
        InterfaceImplementationActionError::InvalidSourceRange {
            source: source.id(),
            range: leading_range,
        },
    )?;
    let close_on_own_line = leading.bytes().all(|byte| matches!(byte, b' ' | b'\t'));
    let (range, declaration_indent, prefix) = if close_on_own_line {
        (
            TextRange::new(line_start, line_start),
            leading,
            String::new(),
        )
    } else {
        let opening_index = usize::try_from(opening.get()).expect("bounded offset");
        let declaration_line_start = source.text()[..opening_index]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let declaration_indent_len = source.text()[declaration_line_start..]
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count();
        let declaration_indent =
            &source.text()[declaration_line_start..declaration_line_start + declaration_indent_len];
        let existing_range = TextRange::new(opening, closing);
        let existing = source
            .text_at(existing_range)
            .ok_or(InterfaceImplementationActionError::InvalidSourceRange {
                source: source.id(),
                range: existing_range,
            })?
            .trim();
        (
            existing_range,
            declaration_indent,
            format!("\n{declaration_indent}    {existing}\n"),
        )
    };
    let member_indent = format!("{declaration_indent}    ");
    let body_indent = format!("{member_indent}    ");
    let methods = signatures
        .iter()
        .map(|signature| {
            format!("{member_indent}{signature} {{\n{body_indent}{terminator}()\n{member_indent}}}")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let text = format!("{prefix}{methods}\n");
    Ok(SemanticSourceEdit::new(source.id(), range, text))
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
        let opening = ByteOffset::new(u32::try_from(source.find('{').unwrap() + 1).unwrap());
        let closing = ByteOffset::new(u32::try_from(source.rfind('}').unwrap()).unwrap());
        let signatures = signatures
            .iter()
            .map(|signature| (*signature).to_owned())
            .collect::<Vec<_>>();
        let edit = method_insertion(file, opening, closing, &signatures, "abort").unwrap();
        let start = usize::try_from(edit.range().start().get()).unwrap();
        let end = usize::try_from(edit.range().end().get()).unwrap();
        format!("{}{}{}", &source[..start], edit.new_text(), &source[end..])
    }

    #[test]
    fn empty_same_line_body_expands_without_an_extra_blank_line() {
        assert_eq!(
            apply(
                "instance Value { impl Readable }\n",
                &["method &self.read(): i32"]
            ),
            concat!(
                "instance Value {\n",
                "    impl Readable\n",
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
                "instance Value {\n    impl Readable { .Item = i32 }\n}\n",
                &["method &self.read(): i32", "method &self.ready(): bool"]
            ),
            concat!(
                "instance Value {\n",
                "    impl Readable { .Item = i32 }\n",
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
