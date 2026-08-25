use nocter_source::{SourceFile, TextRange};

use crate::{Comment, CommentKind, NodeId, NodeKind, SyntaxElement, SyntaxTree};

/// Normalized documentation owned by one immutable syntax snapshot.
#[derive(Clone, Debug)]
pub(super) struct DocumentationAttachments {
    file: Option<Box<str>>,
    nodes: Box<[Option<Box<str>>]>,
}

impl DocumentationAttachments {
    pub(super) fn empty() -> Self {
        Self {
            file: None,
            nodes: Box::new([]),
        }
    }

    pub(super) fn build(source: &SourceFile, syntax: &SyntaxTree) -> Self {
        let file = collect_file_documentation(source, syntax.lexed().comments());
        let mut nodes = vec![None; syntax.nodes().len()];
        let parents = parent_nodes(syntax);
        let candidates = documentable_nodes(syntax, &parents);

        for group in comment_groups(
            source,
            syntax
                .lexed()
                .comments()
                .iter()
                .copied()
                .filter(|comment| comment.kind() == CommentKind::ItemDocumentation),
        ) {
            let Some(candidate) = candidates.iter().find(|candidate| {
                candidate.anchor.start() >= group.range.end()
                    && is_attachment_gap(source, group.range.end(), candidate.anchor.start())
            }) else {
                continue;
            };
            nodes[candidate.node.index()] = Some(group.markdown);
        }

        Self {
            file,
            nodes: nodes.into_boxed_slice(),
        }
    }

    pub(super) fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    pub(super) fn node(&self, node: NodeId) -> Option<&str> {
        self.nodes.get(node.index())?.as_deref()
    }
}

struct CommentGroup {
    range: TextRange,
    markdown: Box<str>,
}

#[derive(Clone, Copy)]
struct DocumentableNode {
    node: NodeId,
    anchor: TextRange,
}

fn collect_file_documentation(source: &SourceFile, comments: &[Comment]) -> Option<Box<str>> {
    let groups = comment_groups(
        source,
        comments
            .iter()
            .copied()
            .filter(|comment| comment.kind() == CommentKind::FileDocumentation),
    );
    if groups.is_empty() {
        return None;
    }
    Some(
        groups
            .into_iter()
            .map(|group| group.markdown)
            .collect::<Vec<_>>()
            .join("\n\n")
            .into_boxed_str(),
    )
}

fn comment_groups(
    source: &SourceFile,
    comments: impl Iterator<Item = Comment>,
) -> Vec<CommentGroup> {
    let mut groups: Vec<CommentGroup> = Vec::new();
    for comment in comments {
        let range = comment.span().range();
        let markdown = normalize_comment(
            source
                .text_at(range)
                .expect("comment span belongs to its syntax source"),
        );
        if let Some(previous) = groups.last_mut()
            && is_attachment_gap(source, previous.range.end(), range.start())
        {
            previous.range = TextRange::new(previous.range.start(), range.end());
            let joined = format!("{}\n{markdown}", previous.markdown);
            previous.markdown = joined.into_boxed_str();
            continue;
        }
        groups.push(CommentGroup {
            range,
            markdown: markdown.into_boxed_str(),
        });
    }
    groups
}

fn normalize_comment(raw: &str) -> String {
    if let Some(content) = raw.strip_prefix("///").or_else(|| raw.strip_prefix("//!")) {
        return strip_one_leading_space(content).to_owned();
    }
    let content = raw
        .strip_prefix("/**")
        .or_else(|| raw.strip_prefix("/*!"))
        .expect("documentation comment has a documentation marker");
    normalize_block(content.strip_suffix("*/").unwrap_or(content))
}

fn normalize_block(content: &str) -> String {
    if !content.contains('\n') {
        return strip_one_trailing_space(strip_one_leading_space(content)).to_owned();
    }

    let mut lines = content.split('\n').collect::<Vec<_>>();
    if lines.first().is_some_and(|line| is_horizontal_space(line)) {
        lines.remove(0);
    }
    if lines.last().is_some_and(|line| is_horizontal_space(line)) {
        lines.pop();
    }
    let indentation = lines
        .iter()
        .filter(|line| !is_horizontal_space(line))
        .map(|line| horizontal_indent(line))
        .min()
        .unwrap_or(0);
    let mut normalized = lines
        .into_iter()
        .map(|line| &line[indentation.min(horizontal_indent(line))..])
        .collect::<Vec<_>>();
    let decorated = normalized
        .iter()
        .filter(|line| !is_horizontal_space(line))
        .all(|line| line.starts_with('*'));
    if decorated {
        for line in &mut normalized {
            if let Some(content) = line.strip_prefix('*') {
                *line = strip_one_leading_space(content);
            }
        }
    }
    normalized.join("\n")
}

fn strip_one_leading_space(text: &str) -> &str {
    text.strip_prefix(' ')
        .or_else(|| text.strip_prefix('\t'))
        .unwrap_or(text)
}

fn strip_one_trailing_space(text: &str) -> &str {
    text.strip_suffix(' ')
        .or_else(|| text.strip_suffix('\t'))
        .unwrap_or(text)
}

fn horizontal_indent(text: &str) -> usize {
    text.bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count()
}

fn is_horizontal_space(text: &str) -> bool {
    text.bytes().all(|byte| matches!(byte, b' ' | b'\t'))
}

fn is_attachment_gap(
    source: &SourceFile,
    start: nocter_source::ByteOffset,
    end: nocter_source::ByteOffset,
) -> bool {
    let Some(gap) = source.text_at(TextRange::new(start, end)) else {
        return false;
    };
    gap.bytes().all(|byte| matches!(byte, b' ' | b'\t' | b'\n'))
        && !gap.as_bytes().windows(2).any(|window| window == b"\n\n")
        && !has_whitespace_only_blank_line(gap)
}

fn has_whitespace_only_blank_line(gap: &str) -> bool {
    let bytes = gap.as_bytes();
    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte != b'\n' {
            continue;
        }
        let mut cursor = index + 1;
        while bytes
            .get(cursor)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
        {
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'\n') {
            return true;
        }
    }
    false
}

fn parent_nodes(syntax: &SyntaxTree) -> Vec<Option<NodeId>> {
    let mut parents = vec![None; syntax.nodes().len()];
    for (parent, _) in syntax.nodes() {
        for child in syntax.children(parent) {
            if let SyntaxElement::Node(child) = child {
                parents[child.index()] = Some(parent);
            }
        }
    }
    parents
}

fn documentable_nodes(syntax: &SyntaxTree, parents: &[Option<NodeId>]) -> Vec<DocumentableNode> {
    let mut candidates = syntax
        .nodes()
        .filter(|(_, node)| is_documentable(node.kind()))
        .map(|(node, syntax_node)| {
            let mut ancestor = parents[node.index()];
            let mut item = None;
            while let Some(parent) = ancestor {
                let parent_node = syntax
                    .node(parent)
                    .expect("parent node belongs to the same syntax tree");
                if is_documentable(parent_node.kind()) {
                    break;
                }
                if parent_node.kind() == NodeKind::Item {
                    item = Some(parent_node.range());
                    break;
                }
                ancestor = parents[parent.index()];
            }
            DocumentableNode {
                node,
                anchor: item.unwrap_or_else(|| syntax_node.range()),
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable_by_key(|candidate| {
        (
            candidate.anchor.start(),
            candidate.anchor.end(),
            candidate.node.index(),
        )
    });
    candidates
}

const fn is_documentable(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::FunctionDeclaration
            | NodeKind::PrimitiveTypeDeclaration
            | NodeKind::TypeAliasDeclaration
            | NodeKind::StructDeclaration
            | NodeKind::StructField
            | NodeKind::EnumDeclaration
            | NodeKind::EnumVariant
            | NodeKind::InterfaceDeclaration
            | NodeKind::AssociatedTypeDeclaration
            | NodeKind::InterfaceMethod
            | NodeKind::ConstructDeclaration
            | NodeKind::ConstructionFunction
            | NodeKind::LiteralDeclaration
            | NodeKind::InstanceDeclaration
            | NodeKind::InherentMethod
            | NodeKind::CoercionDeclaration
            | NodeKind::EqualityOperator
            | NodeKind::OrderingOperator
            | NodeKind::IndexOperator
            | NodeKind::ExpansionOperator
            | NodeKind::ConformDeclaration
            | NodeKind::AssociatedTypeBinding
            | NodeKind::ConformMethod
            | NodeKind::DropDeclaration
            | NodeKind::TestDeclaration
            | NodeKind::BindingStatement
    )
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};

    use super::*;
    use crate::{ParseGoal, parse};

    fn syntax(text: &str) -> (SourceMap, SyntaxTree) {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("index.nct"), text.as_bytes())
            .unwrap();
        let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        (sources, tree)
    }

    #[test]
    fn normalizes_line_and_decorated_block_markdown_once() {
        let (_, syntax) = syntax(
            "//! Module summary.\n//! Second line.\n\n/**\n * Opens a value.\n *\n * More detail.\n */\nfunc open(): void { return }\n",
        );
        assert_eq!(
            syntax.file_documentation(),
            Some("Module summary.\nSecond line.")
        );
        let (declaration, _) = syntax
            .nodes()
            .find(|(_, node)| node.kind() == NodeKind::FunctionDeclaration)
            .unwrap();
        assert_eq!(
            syntax.documentation(declaration),
            Some("Opens a value.\n\nMore detail.")
        );
    }

    #[test]
    fn empty_lines_and_ordinary_comments_break_item_attachment() {
        let (_, syntax) = syntax(
            "/// Detached.\n\nfunc first(): void { return }\n/// Also detached.\n// ordinary\nfunc second(): void { return }\n/// Attached.\nfunc third(): void { return }\n",
        );
        let declarations = syntax
            .nodes()
            .filter(|(_, node)| node.kind() == NodeKind::FunctionDeclaration)
            .map(|(node, _)| syntax.documentation(node))
            .collect::<Vec<_>>();
        assert_eq!(declarations, [None, None, Some("Attached.")]);
    }

    #[test]
    fn nested_members_do_not_compete_with_their_documented_owner() {
        let (_, syntax) =
            syntax("/// Value docs.\nstruct Value {\n    /// Field docs.\n    value: i32\n}\n");
        let declaration = syntax
            .nodes()
            .find(|(_, node)| node.kind() == NodeKind::StructDeclaration)
            .unwrap()
            .0;
        let field = syntax
            .nodes()
            .find(|(_, node)| node.kind() == NodeKind::StructField)
            .unwrap()
            .0;

        assert_eq!(syntax.documentation(declaration), Some("Value docs."));
        assert_eq!(syntax.documentation(field), Some("Field docs."));
    }

    #[test]
    fn target_attachment_does_not_steal_item_documentation() {
        let (_, syntax) = syntax(
            "/// Targeted entry.\n#target: \"arm64-darwin\"\nfunc main(): void { return }\n",
        );
        let (declaration, _) = syntax
            .nodes()
            .find(|(_, node)| node.kind() == NodeKind::FunctionDeclaration)
            .unwrap();
        assert_eq!(syntax.documentation(declaration), Some("Targeted entry."));
    }
}
