use super::{Comment, CommentKind, collect_comments};
use crate::source::SourceId;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentationTarget {
    pub attach_start: usize,
    pub node_start: usize,
}

impl DocumentationTarget {
    pub const fn new(attach_start: usize, node_start: usize) -> Self {
        Self {
            attach_start,
            node_start,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttachedDocumentation {
    file: Option<String>,
    items: HashMap<usize, String>,
}

impl AttachedDocumentation {
    pub fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    pub fn get(&self, node_start: usize) -> Option<&str> {
        self.items.get(&node_start).map(String::as_str)
    }
}

pub fn attach_documentation(
    source: SourceId,
    text: &str,
    targets: &[DocumentationTarget],
) -> AttachedDocumentation {
    let comments = collect_comments(source, text);
    let mut targets = targets.to_vec();
    targets.sort_by_key(|target| (target.attach_start, target.node_start));

    let mut attached = AttachedDocumentation::default();
    let mut index = 0usize;
    while index < comments.len() {
        let comment = comments[index];
        if !comment.kind.is_doc() {
            index += 1;
            continue;
        }

        let is_file_doc = comment.kind.is_file_doc();
        let mut end = comment.span.end;
        let mut parts = vec![documentation_text(text, comment)];
        index += 1;

        while index < comments.len()
            && comments[index].kind.is_doc()
            && comments[index].kind.is_file_doc() == is_file_doc
            && gap_allows_attachment(text, end, comments[index].span.start)
        {
            let comment = comments[index];
            parts.push(documentation_text(text, comment));
            end = comment.span.end;
            index += 1;
        }

        let documentation = join_documentation(parts);
        if documentation.is_empty() {
            continue;
        }

        if is_file_doc {
            if attached.file.is_none() {
                attached.file = Some(documentation);
            }
            continue;
        }

        if let Some(target) = targets.iter().copied().find(|target| {
            target.attach_start >= end && gap_allows_attachment(text, end, target.attach_start)
        }) {
            attached
                .items
                .entry(target.node_start)
                .or_insert(documentation);
        }
    }

    attached
}

fn documentation_text(text: &str, comment: Comment) -> String {
    let raw = text
        .get(comment.span.start..comment.span.end)
        .unwrap_or_default();

    match comment.kind {
        CommentKind::DocLine | CommentKind::FileDocLine => clean_line_doc(raw),
        CommentKind::DocBlock | CommentKind::FileDocBlock => clean_block_doc(raw),
        CommentKind::Line | CommentKind::Block => String::new(),
    }
}

fn clean_line_doc(raw: &str) -> String {
    raw.get(3..)
        .map(strip_one_leading_horizontal_space)
        .unwrap_or_default()
        .trim_end_matches(is_horizontal_space)
        .to_string()
}

fn clean_block_doc(raw: &str) -> String {
    let mut body = raw.get(3..).unwrap_or_default();
    if let Some(stripped) = body.strip_suffix("*/") {
        body = stripped;
    }

    let mut lines = body.lines().map(clean_block_doc_line).collect::<Vec<_>>();

    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

fn clean_block_doc_line(line: &str) -> String {
    let line = line.trim_start_matches(is_horizontal_space);
    let line = line
        .strip_prefix('*')
        .map(strip_one_leading_horizontal_space)
        .unwrap_or(line);

    line.trim_end_matches(is_horizontal_space).to_string()
}

fn strip_one_leading_horizontal_space(text: &str) -> &str {
    let Some(first) = text.as_bytes().first() else {
        return text;
    };

    if matches!(first, b' ' | b'\t') {
        &text[1..]
    } else {
        text
    }
}

fn join_documentation(parts: Vec<String>) -> String {
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn gap_allows_attachment(text: &str, start: usize, end: usize) -> bool {
    if start > end {
        return false;
    }

    let Some(gap) = text.get(start..end) else {
        return false;
    };

    gap.bytes().all(|byte| matches!(byte, b' ' | b'\t' | b'\n')) && !has_blank_line(gap)
}

fn has_blank_line(gap: &str) -> bool {
    let mut saw_newline = false;
    for byte in gap.bytes() {
        match byte {
            b' ' | b'\t' => {}
            b'\n' if saw_newline => return true,
            b'\n' => saw_newline = true,
            _ => saw_newline = false,
        }
    }

    false
}

fn is_horizontal_space(character: char) -> bool {
    matches!(character, ' ' | '\t')
}
