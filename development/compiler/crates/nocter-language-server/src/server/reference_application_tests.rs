use std::fs;
use std::path::Path;

use super::tests::semantic_server;

#[test]
fn recursive_text_search_uses_ordinary_package_editor_semantics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/text-search");
    let source = root.join("search.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (read_dir_line, read_dir_source) = source_line(&text, "var stream = fs.read_dir");
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

    let (buffered_output_line, buffered_output_source) = source_line(&text, "BufWriter.new");
    let buffered_output_character = buffered_output_source.find("BufWriter").unwrap();
    let hover = server.receive(&position_request(
        4,
        "textDocument/hover",
        &source,
        buffered_output_line,
        buffered_output_character,
    ));
    let response = hover.response().unwrap();
    assert!(response.contains("pub struct BufWriter"), "{response}");
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let (line_read_line, line_read_source) = source_line(&text, "reader.read_line_into");
    let line_read_character = line_read_source.find("read_line_into").unwrap();
    let hover = server.receive(&position_request(
        5,
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
        6,
        "textDocument/definition",
        &source,
        sort_line,
        sort_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/slice/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());
}

#[test]
fn json_normalize_uses_public_json_editor_semantics_end_to_end() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/json-normalize");
    let source = root.join("normalize.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (parse_line, parse_source) = source_line(&text, "let value = json.parse");
    let parse_character = parse_source.find("parse").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        parse_line,
        parse_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub func parse(text: &str): Value!"),
        "{response}"
    );
    for internal in ["ParserState", "Continuation", "GenerationFrame", "ByteSink"] {
        assert!(!response.contains(internal), "{response}");
    }
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        parse_line,
        parse_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/json/index.nct"), "{response}");
    assert!(!response.contains("parsing.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let (write_line, write_source) = source_line(&text, "json.write(&+output, &value)");
    let write_argument = write_source.find("&value").unwrap() + 2;
    let signature = server.receive(&position_request(
        4,
        "textDocument/signatureHelp",
        &source,
        write_line,
        write_argument,
    ));
    let response = signature.response().unwrap();
    assert!(
        response.contains("func write<File>(destination: &+File, value: &Value): void!"),
        "{response}"
    );
    assert!(response.contains("\"activeParameter\":1"), "{response}");
    assert!(!response.contains("WriterSink"), "{response}");
    assert!(signature.issue().is_none(), "{:?}", signature.issue());

    let incomplete = text.replace("output.write_text(\"\\n\")", "output.");
    let mut incomplete_json = String::new();
    nocter_json::write_string(&mut incomplete_json, &incomplete);
    let changed = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":2}},\"contentChanges\":[{{\"text\":{incomplete_json}}}]}}}}",
        source.display()
    ));
    assert_eq!(
        changed.analysis().unwrap().snapshot().unwrap().status(),
        nocter_analysis::AnalysisStatus::SyntaxFailed
    );
    let (completion_line, completion_source) = source_line(&incomplete, "output.");
    let completion_character = completion_source.find("output.").unwrap() + "output.".len();
    let completion = server.receive(&position_request(
        5,
        "textDocument/completion",
        &source,
        completion_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in ["write", "write_text", "flush"] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    for internal in ["emit", "ByteSink", "WriterSink"] {
        assert!(!response.contains(internal), "{response}");
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

#[test]
fn text_banner_uses_public_text_and_output_editor_semantics_end_to_end() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/text-banner");
    let source = root.join("banner.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (trim_line, trim_source) = source_line(&text, "input.trim_ascii()");
    let trim_character = trim_source.find("trim_ascii").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        trim_line,
        trim_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub noalloc method &str.trim_ascii(): &str from self"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let (println_line, println_source) = source_line(&text, "io.println(&report)");
    let println_character = println_source.find("println").unwrap();
    let hover = server.receive(&position_request(
        3,
        "textDocument/hover",
        &source,
        println_line,
        println_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub noalloc func println(text: &str): void!"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        4,
        "textDocument/definition",
        &source,
        println_line,
        println_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/io/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let incomplete = text.replace("io.println(&report)?", "io.pr");
    let mut incomplete_json = String::new();
    nocter_json::write_string(&mut incomplete_json, &incomplete);
    let changed = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":2}},\"contentChanges\":[{{\"text\":{incomplete_json}}}]}}}}",
        source.display()
    ));
    assert_eq!(
        changed.analysis().unwrap().snapshot().unwrap().status(),
        nocter_analysis::AnalysisStatus::CompilationFailed
    );
    let (completion_line, completion_source) = incomplete
        .lines()
        .enumerate()
        .find(|(_, line)| line.trim_end().ends_with("io.pr"))
        .unwrap();
    let completion_character = completion_source.find("io.pr").unwrap() + "io.pr".len();
    let completion = server.receive(&position_request(
        5,
        "textDocument/completion",
        &source,
        completion_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for function in ["print", "println"] {
        assert!(
            response.contains(&format!("\"label\":\"io.{function}\"")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

fn open_package_source(root: &Path, source: &Path) -> (super::LanguageServer, String) {
    let text = fs::read_to_string(source).unwrap();
    let mut server = semantic_server(root);
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
    let snapshot = opened.analysis().unwrap().snapshot().unwrap();
    assert_eq!(
        snapshot.status(),
        nocter_analysis::AnalysisStatus::Complete,
        "{:?}",
        snapshot.diagnostics()
    );
    assert!(opened.issue().is_none(), "{:?}", opened.issue());
    (server, text)
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
