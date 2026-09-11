use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_syntax::{
    ContextualSpelling, Keyword, NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxTree,
    TokenKind,
};

use super::{SemanticCompletion, SemanticCompletionKind};
use crate::AnalysisSnapshot;

/// Returns syntax-owned keywords whose grammar context is fixed at the cursor.
///
/// This layer deliberately does not enumerate declarations or infer semantic targets. It supplies
/// the contextual words that cannot enter the declaration/name indexes, including declaration
/// introducers and the intrinsic `copy` requirement.
pub(super) fn completions(
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    offset: ByteOffset,
) -> Box<[SemanticCompletion]> {
    let Some(tree) = snapshot
        .syntax_trees()
        .iter()
        .find(|tree| tree.source() == source)
    else {
        return Box::new([]);
    };
    let Some(source_file) = snapshot.sources().get(source) else {
        return Box::new([]);
    };

    if let Some(where_clause) = innermost_node(tree, offset, NodeKind::WhereClause)
        && has_visible_generic_syntax(tree, where_clause)
        && !has_descendant(tree, where_clause, NodeKind::CopyPredicate)
        && current_where_prefix(tree, source_file, where_clause, offset)
            .is_some_and(|prefix| ContextualSpelling::Copy.as_str().starts_with(prefix))
    {
        return Box::new([SemanticCompletion::new(
            ContextualSpelling::Copy.as_str(),
            SemanticCompletionKind::Keyword,
            Some("intrinsic generic copy requirement".into()),
        )]);
    }

    let mut completions = Vec::new();
    if let Some(prefix) = top_level_declaration_prefix(tree, source_file, offset) {
        completions.extend(
            [
                (
                    ContextualSpelling::Static.as_str(),
                    "static NAME: Type = value",
                ),
                (Keyword::Test.as_str(), "test name { ... }"),
            ]
            .into_iter()
            .filter(|(keyword, _)| keyword.starts_with(prefix))
            .map(|(keyword, detail)| {
                SemanticCompletion::new(
                    keyword,
                    SemanticCompletionKind::Keyword,
                    Some(detail.into()),
                )
            }),
        );
    }

    if let Some(modifiers) = callable_modifier_prefix(tree, source_file, offset) {
        completions.extend(
            [
                (
                    Keyword::NoAlloc.as_str(),
                    "allocation-free callable guarantee",
                    modifiers.noalloc,
                ),
                (
                    Keyword::Blocking.as_str(),
                    "synchronous waiting callable effect",
                    modifiers.blocking,
                ),
                (
                    Keyword::Async.as_str(),
                    "deferred producer execution",
                    modifiers.asynchronous,
                ),
            ]
            .into_iter()
            .filter(|(keyword, _, allowed)| *allowed && keyword.starts_with(modifiers.prefix))
            .map(|(keyword, detail, _)| {
                SemanticCompletion::new(
                    keyword,
                    SemanticCompletionKind::Keyword,
                    Some(detail.into()),
                )
            }),
        );
    }

    completions.into_boxed_slice()
}

struct CallableModifierPrefix<'a> {
    prefix: &'a str,
    noalloc: bool,
    blocking: bool,
    asynchronous: bool,
}

fn callable_modifier_prefix<'a>(
    tree: &SyntaxTree,
    source: &'a nocter_source::SourceFile,
    offset: ByteOffset,
) -> Option<CallableModifierPrefix<'a>> {
    if tree
        .nodes()
        .any(|(_, node)| node.kind() == NodeKind::Block && node.range().contains_cursor(offset))
    {
        return None;
    }
    let container = tree
        .nodes()
        .filter(|(_, node)| {
            declaration_container(node.kind()) && node.range().contains_cursor(offset)
        })
        .min_by_key(|(_, node)| node.range().len())
        .map(|(_, node)| node.kind());
    if container.is_some_and(|kind| {
        !matches!(
            kind,
            NodeKind::InterfaceDeclaration
                | NodeKind::ConstructDeclaration
                | NodeKind::InstanceDeclaration
        )
    }) {
        return None;
    }
    let end = usize::try_from(offset.get()).ok()?;
    let line = source
        .text()
        .get(..end)?
        .rsplit_once('\n')
        .map_or_else(|| source.text().get(..end), |(_, line)| Some(line))?;
    let trailing_space = line.as_bytes().last().is_some_and(u8::is_ascii_whitespace);
    let mut words = line.split_ascii_whitespace().collect::<Vec<_>>();
    if words
        .first()
        .is_some_and(|word| *word == "pub" || (word.starts_with("pub(") && word.ends_with(')')))
    {
        words.remove(0);
    }
    let prefix = if trailing_space {
        ""
    } else {
        words.pop().unwrap_or("")
    };
    if !prefix
        .chars()
        .all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return None;
    }
    let (noalloc, blocking, asynchronous) = match words.as_slice() {
        [] => (true, true, true),
        ["noalloc"] => (false, true, false),
        ["blocking" | "async"] | ["noalloc", "blocking" | "async"] => (false, false, false),
        _ => return None,
    };
    let asynchronous = asynchronous && container != Some(NodeKind::ConstructDeclaration);
    Some(CallableModifierPrefix {
        prefix,
        noalloc,
        blocking,
        asynchronous,
    })
}

fn current_where_prefix<'a>(
    tree: &SyntaxTree,
    source: &'a nocter_source::SourceFile,
    clause: NodeId,
    offset: ByteOffset,
) -> Option<&'a str> {
    let clause_range = tree.node(clause)?.range();
    let mut start = ByteOffset::new(clause_range.start().get().checked_add(5)?);
    if offset < start {
        return None;
    }
    for element in tree.children(clause) {
        let SyntaxElement::Token(token) = element else {
            continue;
        };
        if token.kind() == TokenKind::Punctuation(Punctuation::Comma)
            && token.range().end() <= offset
            && token.range().end() > start
        {
            start = token.range().end();
        }
    }
    let prefix = source.text_at(TextRange::new(start, offset))?.trim();
    prefix
        .chars()
        .all(|character| character == '_' || character.is_ascii_alphanumeric())
        .then_some(prefix)
}

fn has_visible_generic_syntax(tree: &SyntaxTree, where_clause: NodeId) -> bool {
    let Some(where_range) = tree
        .node(where_clause)
        .map(nocter_syntax::SyntaxNode::range)
    else {
        return false;
    };
    tree.nodes().any(|(_, candidate)| {
        declaration_container(candidate.kind())
            && candidate.range().contains_range(where_range)
            && tree.nodes().any(|(_, generic)| {
                matches!(
                    generic.kind(),
                    NodeKind::GenericParameters | NodeKind::PatternArguments
                ) && candidate.range().contains_range(generic.range())
                    && generic.range().end() <= where_range.start()
            })
    })
}

fn top_level_declaration_prefix<'a>(
    tree: &SyntaxTree,
    source: &'a nocter_source::SourceFile,
    offset: ByteOffset,
) -> Option<&'a str> {
    if tree.nodes().any(|(_, node)| {
        (node.kind() == NodeKind::Block || declaration_container(node.kind()))
            && node.range().contains_cursor(offset)
    }) {
        return None;
    }
    let Ok(end) = usize::try_from(offset.get()) else {
        return None;
    };
    let before = source.text().get(..end)?;
    let prefix = before
        .rsplit_once('\n')
        .map_or(before, |(_, line)| line)
        .trim();
    prefix
        .chars()
        .all(|character| character == '_' || character.is_ascii_alphanumeric())
        .then_some(prefix)
}

fn innermost_node(tree: &SyntaxTree, offset: ByteOffset, kind: NodeKind) -> Option<NodeId> {
    tree.nodes()
        .filter(|(_, node)| node.kind() == kind && node.range().contains_cursor(offset))
        .min_by_key(|(_, node)| node.range().len())
        .map(|(node, _)| node)
}

fn has_descendant(tree: &SyntaxTree, root: NodeId, kind: NodeKind) -> bool {
    let Some(root_range) = tree.node(root).map(nocter_syntax::SyntaxNode::range) else {
        return false;
    };
    tree.nodes()
        .any(|(_, node)| node.kind() == kind && root_range.contains_range(node.range()))
}

const fn declaration_container(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::FunctionDeclaration
            | NodeKind::TypeAliasDeclaration
            | NodeKind::StructDeclaration
            | NodeKind::EnumDeclaration
            | NodeKind::InterfaceDeclaration
            | NodeKind::ConstructDeclaration
            | NodeKind::InstanceDeclaration
            | NodeKind::DropDeclaration
            | NodeKind::TestDeclaration
            | NodeKind::InterfaceMethod
            | NodeKind::ConstructionFunction
            | NodeKind::LiteralDeclaration
            | NodeKind::InherentMethod
    )
}
