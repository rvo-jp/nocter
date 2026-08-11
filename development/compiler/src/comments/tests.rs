use super::*;
use crate::source::SourceId;

const SOURCE: SourceId = SourceId::new(0);

#[test]
fn collects_line_and_block_comments_in_source_order() {
    let text = "func main(): i32 {\n    // before\n    return 0 /* after */\n}\n";

    let comments = collect_comments(SOURCE, text);

    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].kind, CommentKind::Line);
    assert_eq!(comments[1].kind, CommentKind::Block);
    assert_eq!(slice(text, comments[0]), "// before");
    assert_eq!(slice(text, comments[1]), "/* after */");
}

#[test]
fn classifies_doc_comments() {
    let text = concat!(
        "//! file line\n",
        "/*! file block */\n",
        "/// item line\n",
        "/** item block */\n",
        "// normal line\n",
        "/* normal block */\n",
    );

    let comments = collect_comments(SOURCE, text);
    let kinds = comments
        .iter()
        .map(|comment| comment.kind)
        .collect::<Vec<_>>();

    assert_eq!(
        kinds,
        vec![
            CommentKind::FileDocLine,
            CommentKind::FileDocBlock,
            CommentKind::DocLine,
            CommentKind::DocBlock,
            CommentKind::Line,
            CommentKind::Block,
        ]
    );
    assert!(comments[0].kind.is_doc());
    assert!(comments[0].kind.is_file_doc());
    assert!(comments[2].kind.is_doc());
    assert!(!comments[2].kind.is_file_doc());
    assert!(!comments[4].kind.is_doc());
}

#[test]
fn treats_extra_slash_and_empty_block_markers_as_normal_comments() {
    let text = "//// normal\n/**/ normal block\n/***/ also normal */\n";

    let comments = collect_comments(SOURCE, text);

    assert_eq!(comments.len(), 3);
    assert_eq!(comments[0].kind, CommentKind::Line);
    assert_eq!(comments[1].kind, CommentKind::Block);
    assert_eq!(comments[2].kind, CommentKind::Block);
}

#[test]
fn ignores_comment_markers_inside_literals() {
    let text = concat!(
        "func main(): i32 {\n",
        "    let line = \"not // a comment\"\n",
        "    let block = \"not /* a comment */ either\"\n",
        "    let many = \"\"\"\n",
        "        not // a comment\n",
        "        not /* a comment */ either\n",
        "        \"\"\"\n",
        "    let slash = b'/'\n",
        "    // real\n",
        "    return 0\n",
        "}\n",
    );

    let comments = collect_comments(SOURCE, text);

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].kind, CommentKind::Line);
    assert_eq!(slice(text, comments[0]), "// real");
}

#[test]
fn treats_unclosed_block_comment_as_comment_to_eof() {
    let text = "func main(): i32 {\n    /* unclosed\n";

    let comments = collect_comments(SOURCE, text);

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].kind, CommentKind::Block);
    assert_eq!(slice(text, comments[0]), "/* unclosed\n");
}

#[test]
fn attaches_adjacent_item_documentation_to_target() {
    let text = concat!(
        "/// First line.\n",
        "/// Second line.\n",
        "pub func main(): i32 {\n",
        "    return 0\n",
        "}\n",
    );
    let target =
        DocumentationTarget::new(text.find("pub func").unwrap(), text.find("func").unwrap());

    let docs = attach_documentation(SOURCE, text, &[target]);

    assert_eq!(
        docs.get(text.find("func").unwrap()),
        Some("First line.\nSecond line.")
    );
}

#[test]
fn attaches_cleaned_block_documentation_to_target() {
    let text = concat!(
        "/**\n",
        " * Opens a file.\n",
        " * Returns an error on failure.\n",
        " */\n",
        "func open(): File! {\n",
        "    return error.new(\"io\", \"failed\")\n",
        "}\n",
    );
    let target = DocumentationTarget::new(text.find("func").unwrap(), text.find("func").unwrap());

    let docs = attach_documentation(SOURCE, text, &[target]);

    assert_eq!(
        docs.get(text.find("func").unwrap()),
        Some("Opens a file.\nReturns an error on failure.")
    );
}

#[test]
fn attaches_file_documentation() {
    let text = concat!(
        "//! File docs.\n",
        "/*! More file docs. */\n",
        "func main(): i32 {\n",
        "    return 0\n",
        "}\n",
    );

    let docs = attach_documentation(SOURCE, text, &[]);

    assert_eq!(docs.file(), Some("File docs.\nMore file docs."));
}

#[test]
fn does_not_attach_documentation_across_empty_line() {
    let text = concat!(
        "/// Detached.\n",
        "\n",
        "func main(): i32 {\n",
        "    return 0\n",
        "}\n",
    );
    let target = DocumentationTarget::new(text.find("func").unwrap(), text.find("func").unwrap());

    let docs = attach_documentation(SOURCE, text, &[target]);

    assert_eq!(docs.get(text.find("func").unwrap()), None);
}

#[test]
fn does_not_attach_documentation_across_normal_comment() {
    let text = concat!(
        "/// Detached.\n",
        "// normal\n",
        "func main(): i32 {\n",
        "    return 0\n",
        "}\n",
    );
    let target = DocumentationTarget::new(text.find("func").unwrap(), text.find("func").unwrap());

    let docs = attach_documentation(SOURCE, text, &[target]);

    assert_eq!(docs.get(text.find("func").unwrap()), None);
}

fn slice(text: &str, comment: Comment) -> &str {
    &text[comment.span.start..comment.span.end]
}
