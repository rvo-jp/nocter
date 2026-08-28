use std::fs;
use std::path::Path;

use super::tests::semantic_server;

#[test]
fn recursive_text_search_uses_ordinary_package_editor_semantics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/text-search");
    let source = root.join("search.nct");
    let text = fs::read_to_string(&source).unwrap();
    let mut server = semantic_server(&root);
    server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
        root.display()
    ));
    server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
    let mut did_open = format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"languageId\":\"nocter\",\"version\":1,\"text\":",
        source.display()
    );
    nocter_json::write_string(&mut did_open, &text);
    did_open.push_str("}}}");
    let opened = server.receive(&did_open);
    assert!(opened.issue().is_none(), "{:?}", opened.issue());

    let (read_dir_line, read_dir_source) = source_line(&text, "var stream = read_dir");
    let read_dir_character = read_dir_source.find("read_dir").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        read_dir_line,
        read_dir_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub func read_dir(path: &str): ReadDir!"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        read_dir_line,
        read_dir_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/fs/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let (line_read_line, line_read_source) = source_line(&text, "reader.read_line_into");
    let line_read_character = line_read_source.find("read_line_into").unwrap();
    let hover = server.receive(&position_request(
        4,
        "textDocument/hover",
        &source,
        line_read_line,
        line_read_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub method &+BufReader.read_line_into(destination: &+String): bool!"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let (sort_line, sort_source) = source_line(&text, "paths.sort()");
    let sort_character = sort_source.find("sort").unwrap();
    let definition = server.receive(&position_request(
        5,
        "textDocument/definition",
        &source,
        sort_line,
        sort_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/slice/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());
}

fn source_line<'a>(source: &'a str, needle: &str) -> (usize, &'a str) {
    source
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(needle))
        .unwrap()
}

fn position_request(
    id: usize,
    method: &str,
    source: &Path,
    line: usize,
    character: usize,
) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
        source.display()
    )
}
