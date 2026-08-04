use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[test]
fn lsp_command_initializes_and_publishes_diagnostics() {
    let project = TempProject::new("cli-lsp-diagnostics");
    let source = project.write_source("bad.nct", "func main(: i32 {\n");
    let uri = file_uri(&source);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri,
                        "languageId": "nocter",
                        "version": 1,
                        "text": "func main(: i32 {\n"
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );

    let messages = read_frames(&output.stdout);
    assert!(
        messages.iter().any(|message| message["id"] == 1
            && message["result"]["capabilities"]["textDocumentSync"]["change"] == 1),
        "expected initialize response, got:\n{}",
        text(&output.stdout)
    );

    let diagnostics = messages
        .iter()
        .find(|message| message["method"] == "textDocument/publishDiagnostics")
        .and_then(|message| message["params"]["diagnostics"].as_array())
        .expect("expected diagnostics notification");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "E0200"),
        "expected E0200 diagnostic, got:\n{diagnostics:#?}"
    );
}

#[test]
fn lsp_command_publishes_typecheck_diagnostic_context() {
    let project = TempProject::new("cli-lsp-diagnostic-context");
    let app = project.write_source(
        "app.nct",
        "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
    );
    let config = project.write_source(
        "config.nct",
        "pub func answer(value: i32): i32 {\n    return value\n}\n",
    );
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config.canonicalize().unwrap());

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": app_uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n"
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );

    let messages = read_frames(&output.stdout);
    let diagnostics = messages
        .iter()
        .find(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == json!(app_uri)
        })
        .and_then(|message| message["params"]["diagnostics"].as_array())
        .expect("expected diagnostics notification");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["code"] == "E0320")
        .expect("expected E0320 diagnostic");

    assert!(diagnostic["message"].as_str().is_some_and(|message| {
        message.contains("help: pass exactly the parameters declared by the function")
    }));
    assert_eq!(
        diagnostic["relatedInformation"][0]["message"],
        json!("function `answer` is declared here")
    );
    assert_eq!(
        diagnostic["relatedInformation"][0]["location"]["uri"],
        json!(config_uri)
    );
}

#[test]
fn lsp_command_single_file_semantic_tokens_classify_builtin_types() {
    let project = TempProject::new("cli-lsp-single-file-semantic-types");
    let source_text = "use ./missing.nope\n\nfunc main(path: &str): void! {\n    let byte: u8 = 0 as u8\n    return\n}\n";
    let source = project.write_source("app.nct", source_text);
    let uri = file_uri(&source);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/semanticTokens/full",
                "params": {
                    "textDocument": {
                        "uri": uri
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );

    let messages = read_frames(&output.stdout);
    let semantic_data = messages
        .iter()
        .find(|message| message["id"] == 2)
        .and_then(|message| message["result"]["data"].as_array())
        .expect("expected semantic token response");
    let tokens = decode_semantic_tokens(semantic_data);

    for lexeme in ["str", "void", "u8"] {
        assert!(
            tokens.iter().any(|token| {
                token.lexeme(source_text) == Some(lexeme) && token.kind == SEMANTIC_TOKEN_TYPE
            }),
            "expected semantic tokens to classify `{lexeme}` as a type, got {tokens:#?}"
        );
    }
}

#[test]
fn lsp_command_semantic_tokens_classify_enum_pattern_variants() {
    let project = TempProject::new("cli-lsp-enum-pattern-semantic");
    let source_text = r#"enum Choice {
    hit(value: i32)
    miss(value: i32)
}

func main(choice: Choice): i32 {
    if choice is Choice.hit(_) {
    }
    let code = match choice {
        Choice.hit(_) { 1 }
        Choice.miss(_) { 2 }
    }
    return code
}
"#;
    let source = project.write_source("app.nct", source_text);
    let uri = file_uri(&source);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/semanticTokens/full",
                "params": {
                    "textDocument": {
                        "uri": uri
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );

    let messages = read_frames(&output.stdout);
    let semantic_data = messages
        .iter()
        .find(|message| message["id"] == 2)
        .and_then(|message| message["result"]["data"].as_array())
        .expect("expected semantic token response");
    let tokens = decode_semantic_tokens(semantic_data);

    for start in [
        source_text
            .find("hit(_)")
            .expect("expected if-is hit pattern"),
        source_text
            .rfind("hit(_)")
            .expect("expected match hit pattern"),
        source_text
            .rfind("miss(_)")
            .expect("expected match miss pattern"),
    ] {
        let token = token_starting_at(&tokens, source_text, start)
            .expect("expected semantic token for pattern variant");
        assert_eq!(token.kind, SEMANTIC_TOKEN_PROPERTY);
    }

    assert!(
        tokens
            .iter()
            .all(|token| token.lexeme(source_text) != Some("_")),
        "payload discard should not be classified as an identifier"
    );
}

#[test]
fn lsp_command_completes_enum_pattern_members() {
    let project = TempProject::new("cli-lsp-enum-pattern-completion");
    let source_text = r#"enum Choice {
    hit(value: i32)
    miss
}

func main(choice: Choice): i32 {
    if choice is Choice.hit(_) {
    }
    return match choice {
        Choice.hit(_) { 1 }
        Choice.miss { 2 }
    }
}
"#;
    let source = project.write_source("app.nct", source_text);
    let uri = file_uri(&source);
    let completion_offset = source_text
        .find("Choice.hit")
        .expect("expected if-is pattern")
        + "Choice.".len();
    let (line, character) = lsp_position_for_ascii_byte_offset(source_text, completion_offset);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": {
                        "uri": uri
                    },
                    "position": {
                        "line": line,
                        "character": character
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );

    let messages = read_frames(&output.stdout);
    let completion_items = response_with_id(&messages, 2)["result"]["items"]
        .as_array()
        .expect("expected completion items");

    assert_eq!(
        completion_item_with_label(completion_items, "hit").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_ENUM_MEMBER as u64)
    );
    assert_eq!(
        completion_item_with_label(completion_items, "miss").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_ENUM_MEMBER as u64)
    );
    assert!(completion_item_with_label(completion_items, "Choice").is_none());
}

#[test]
fn lsp_command_hides_imported_signature_dependencies_from_completion() {
    let project = TempProject::new("cli-lsp-hidden-signature-dependency");
    let source_text = "use ./factory.make\n\nfunc main(): i32 {\n    return 0\n}\n";
    let source = project.write_source("app.nct", source_text);
    project.write_source(
        "factory.nct",
        r#"pub struct Produced {
    value: i32
}

pub func make(): Produced {
    return Produced { value: 7 }
}
"#,
    );
    let uri = file_uri(&source);
    let completion_offset = source_text.find("return 0").unwrap();
    let (line, character) = lsp_position_for_ascii_byte_offset(source_text, completion_offset);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": {
                        "uri": uri
                    },
                    "position": {
                        "line": line,
                        "character": character
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );

    let messages = read_frames(&output.stdout);
    let completion_items = response_with_id(&messages, 2)["result"]["items"]
        .as_array()
        .expect("expected completion items");

    assert!(completion_item_with_label(completion_items, "make").is_some());
    assert!(
        completion_items.iter().all(|item| {
            item["label"]
                .as_str()
                .is_none_or(|label| !label.contains(".Produced"))
        }),
        "signature-only dependency leaked into LSP completion: {completion_items:#?}"
    );
}

#[test]
fn lsp_command_serves_v0_editor_features() {
    let project = TempProject::new("cli-lsp-editor-features");
    let source_text = "/// Returns the answer.\nfunc answer(): i32 {\n    return 42\n}\n\nstruct Config {\n    path: &str\n}\n\nfunc main(): i32 {\n    let value = answer()\n    return value\n}\n";
    let source = project.write_source("app.nct", source_text);
    let uri = file_uri(&source);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/documentSymbol",
                "params": {
                    "textDocument": {
                        "uri": uri.clone()
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": {
                        "uri": uri.clone()
                    },
                    "position": {
                        "line": 1,
                        "character": 6
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": {
                        "uri": uri.clone()
                    },
                    "position": {
                        "line": 10,
                        "character": 18
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "textDocument/references",
                "params": {
                    "textDocument": {
                        "uri": uri.clone()
                    },
                    "position": {
                        "line": 10,
                        "character": 18
                    },
                    "context": {
                        "includeDeclaration": true
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 6,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": {
                        "uri": uri.clone()
                    },
                    "position": {
                        "line": 11,
                        "character": 4
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": "textDocument/semanticTokens/full",
                "params": {
                    "textDocument": {
                        "uri": uri.clone()
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 8,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );

    let messages = read_frames(&output.stdout);
    let symbols = response_with_id(&messages, 2)["result"]
        .as_array()
        .expect("expected document symbols");
    for name in ["answer", "Config", "main"] {
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol["name"].as_str() == Some(name)),
            "expected document symbol `{name}`, got {symbols:#?}"
        );
    }

    let hover = response_with_id(&messages, 3)["result"]["contents"]["value"]
        .as_str()
        .expect("expected hover contents");
    assert!(hover.contains("answer"), "hover:\n{hover}");
    assert!(hover.contains("Returns the answer."), "hover:\n{hover}");

    let definitions = response_with_id(&messages, 4)["result"]
        .as_array()
        .expect("expected definition links");
    assert_eq!(definitions.len(), 1);
    let definition = &definitions[0];
    assert_eq!(definition["targetUri"], json!(uri));
    assert_eq!(definition["targetRange"]["start"]["line"], json!(1));
    assert_eq!(definition["targetRange"]["start"]["character"], json!(5));
    assert_eq!(
        definition["originSelectionRange"]["start"]["line"],
        json!(10)
    );
    assert_eq!(
        definition["originSelectionRange"]["start"]["character"],
        json!(16)
    );

    let references = response_with_id(&messages, 5)["result"]
        .as_array()
        .expect("expected references");
    assert_eq!(references.len(), 2);
    assert_eq!(references[0]["uri"], json!(uri));
    assert_eq!(references[0]["range"]["start"]["line"], json!(1));
    assert_eq!(references[0]["range"]["start"]["character"], json!(5));
    assert_eq!(references[1]["uri"], json!(uri));
    assert_eq!(references[1]["range"]["start"]["line"], json!(10));
    assert_eq!(references[1]["range"]["start"]["character"], json!(16));

    let completion_items = response_with_id(&messages, 6)["result"]["items"]
        .as_array()
        .expect("expected completion items");
    for label in ["return", "answer", "Config"] {
        assert!(
            completion_items
                .iter()
                .any(|item| item["label"].as_str() == Some(label)),
            "expected completion `{label}`, got {completion_items:#?}"
        );
    }

    let semantic_data = response_with_id(&messages, 7)["result"]["data"]
        .as_array()
        .expect("expected semantic token data");
    assert!(!semantic_data.is_empty(), "messages:\n{messages:#?}");
}

#[test]
fn lsp_hover_preserves_nested_process_result_and_static_provenance() {
    let project = TempProject::new("cli-lsp-process-env-hover");
    project.write_nocter_home_file(
        "std/process.nct",
        r#"pub func env(name: &str): &str?! from static {
    return none
}
"#,
    );
    let source_text = r#"use std/process.env as lookup

func main(): i32! {
    let value = lookup("HOME")? otherwise { return 0 }
    return 0
}
"#;
    let source = project.write_source("process_env_hover.nct", source_text);
    let uri = file_uri(&source);
    let call_start = source_text.rfind("lookup").unwrap();
    let (line, character) = lsp_position_for_ascii_byte_offset(source_text, call_start);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let messages = read_frames(&output.stdout);
    let hover = &response_with_id(&messages, 2)["result"];
    let hover_text = hover["contents"]["value"].as_str().unwrap();
    assert!(
        hover_text.contains("func lookup(name: &str): (&str)?! from static")
            && hover_text.contains("Result provenance:** static storage"),
        "{hover_text}"
    );
    assert_eq!(hover["range"]["start"]["line"], json!(line));
    assert_eq!(hover["range"]["start"]["character"], json!(character));
    assert_eq!(
        hover["range"]["end"]["character"],
        json!(character + "lookup".len())
    );
}

#[test]
fn lsp_hover_presents_catch_binding_type() {
    let project = TempProject::new("cli-lsp-catch-binding-hover");
    let source_text = r#"func attempt(): i32! {
    return 1
}

func main(): i32! {
    let value = attempt() catch problem {
        return problem
    }
    return value
}
"#;
    let source = project.write_source("catch_binding_hover.nct", source_text);
    let uri = file_uri(&source);
    let hover_offset = source_text
        .find("problem {")
        .expect("expected catch binding");
    let (line, character) = lsp_position_for_ascii_byte_offset(source_text, hover_offset);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let messages = read_frames(&output.stdout);
    let hover = &response_with_id(&messages, 2)["result"];
    assert_eq!(
        hover["contents"]["value"].as_str(),
        Some("```nocter\ncatch problem: error\n```")
    );
    assert_eq!(hover["range"]["start"]["line"], json!(line));
    assert_eq!(hover["range"]["start"]["character"], json!(character));
}

#[test]
fn lsp_preserves_stored_composed_outcomes_across_protocol_queries() {
    let project = TempProject::new("cli-lsp-stored-composed-outcome");
    let source_text = r#"func main(): i32 {
    let saved = lookup()
    let forwarded = saved
    return 0
}

func lookup(): i32!? {
    return 42
}
"#;
    let source = project.write_source("stored_composed_outcome.nct", source_text);
    let uri = file_uri(&source);
    let hover_offset = source_text.find("forwarded = saved").unwrap() + "forwarded = ".len();
    let completion_offset = source_text.find("return 0").unwrap();
    let (hover_line, hover_character) =
        lsp_position_for_ascii_byte_offset(source_text, hover_offset);
    let (completion_line, completion_character) =
        lsp_position_for_ascii_byte_offset(source_text, completion_offset);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": uri.clone() },
                    "position": { "line": hover_line, "character": hover_character }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": { "uri": uri },
                    "position": {
                        "line": completion_line,
                        "character": completion_character
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let messages = read_frames(&output.stdout);
    let hover = &response_with_id(&messages, 2)["result"];
    assert_eq!(
        hover["contents"]["value"].as_str(),
        Some("```nocter\nlet saved: i32!?\n```")
    );
    assert_eq!(hover["range"]["start"]["line"], json!(hover_line));
    assert_eq!(hover["range"]["start"]["character"], json!(hover_character));

    let completion_items = response_with_id(&messages, 3)["result"]["items"]
        .as_array()
        .expect("expected completion items");
    let saved = completion_item_with_label(completion_items, "saved")
        .expect("expected stored outcome completion");
    assert_eq!(saved["detail"], json!("let saved: i32!?"));
}

#[test]
fn lsp_command_exposes_generic_bound_and_provenance_source_ranges() {
    let project = TempProject::new("cli-lsp-bound-provenance-ranges");
    let source_text = r#"interface Read<T> {
    pub method &self.read(): &T from self
}

interface Measure {
    pub method &self.measure(): usize
}

struct Box<T> {
    value: T
}

impl<T> Read<T> for Box<T> {
    method &self.read(): &T from self {
        return &self.value
    }
}

impl<T> Measure for Box<T> {
    method &self.measure(): usize {
        return 1
    }
}

func borrow<B: Read<T> + Measure, T>(value: &B): &T from value {
    return value.read()
}
"#;
    let source = project.write_source("bounds.nct", source_text);
    let uri = file_uri(&source);
    let call_start = source_text.rfind("read()").unwrap();
    let declaration_start = source_text.find("read():").unwrap();
    let (call_line, call_character) = lsp_position_for_ascii_byte_offset(source_text, call_start);
    let (declaration_line, declaration_character) =
        lsp_position_for_ascii_byte_offset(source_text, declaration_start);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": uri.clone() },
                    "position": { "line": call_line, "character": call_character }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": uri.clone() },
                    "position": { "line": call_line, "character": call_character }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": { "uri": uri.clone() },
                    "position": { "line": call_line, "character": call_character }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );
    let messages = read_frames(&output.stdout);

    let hover = &response_with_id(&messages, 2)["result"];
    let hover_text = hover["contents"]["value"]
        .as_str()
        .expect("expected hover contents");
    assert!(hover_text.contains("method") && hover_text.contains("from self"));
    assert_eq!(hover["range"]["start"]["line"], json!(call_line));
    assert_eq!(hover["range"]["start"]["character"], json!(call_character));
    assert_eq!(
        hover["range"]["end"]["character"],
        json!(call_character + "read".len())
    );

    let definitions = response_with_id(&messages, 3)["result"]
        .as_array()
        .expect("expected definition links");
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0]["targetUri"], json!(uri));
    assert_eq!(
        definitions[0]["targetSelectionRange"]["start"]["line"],
        json!(declaration_line)
    );
    assert_eq!(
        definitions[0]["targetSelectionRange"]["start"]["character"],
        json!(declaration_character)
    );
    assert_eq!(
        definitions[0]["originSelectionRange"]["start"]["line"],
        json!(call_line)
    );
    assert_eq!(
        definitions[0]["originSelectionRange"]["start"]["character"],
        json!(call_character)
    );

    let completion_items = response_with_id(&messages, 4)["result"]["items"]
        .as_array()
        .expect("expected completion items");
    let read = completion_item_with_label(completion_items, "read")
        .expect("expected bound method completion");
    assert!(
        read["detail"]
            .as_str()
            .is_some_and(|detail| { detail.contains("method") && detail.contains("from self") })
    );
    assert!(
        completion_item_with_label(completion_items, "measure").is_some(),
        "expected second capability method completion"
    );
}

#[test]
fn lsp_command_serves_closures_default_methods_and_incomplete_bodies() {
    let project = TempProject::new("cli-lsp-phase10-editor-contract");
    let source_text = r#"interface Identity {
    pub method &self.keep<T>(value: T): T {
        return value
    }
}

copy struct Unit { marker: i32 }
impl Identity for Unit {}

func main(): i32 {
    let factor = 2
    let transform = (&factor; value: i32): i32 { value * factor }
    let unit = Unit { marker: 0 }
    return unit.keep(42)
}
"#;
    let incomplete_text = r#"copy struct Box {
    value: i32
}

func main(): i32 {
    let box = Box { value: 4 }
    let transform = (&box; input: i32): i32 {
        return box."#;
    let source = project.write_source("app.nct", source_text);
    let uri = file_uri(&source);
    let hover_offset = source_text.find("transform =").unwrap();
    let signature_offset = source_text.rfind("42").unwrap();
    let (hover_line, hover_character) =
        lsp_position_for_ascii_byte_offset(source_text, hover_offset);
    let (signature_line, signature_character) =
        lsp_position_for_ascii_byte_offset(source_text, signature_offset);
    let (completion_line, completion_character) =
        lsp_position_for_ascii_byte_offset(incomplete_text, incomplete_text.len());

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": uri.clone() },
                    "position": { "line": hover_line, "character": hover_character }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/signatureHelp",
                "params": {
                    "textDocument": { "uri": uri.clone() },
                    "position": { "line": signature_line, "character": signature_character }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": { "uri": uri.clone(), "version": 2 },
                    "contentChanges": [{ "text": incomplete_text }]
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": { "uri": uri },
                    "position": {
                        "line": completion_line,
                        "character": completion_character
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );
    let messages = read_frames(&output.stdout);

    let hover = &response_with_id(&messages, 2)["result"];
    assert!(
        hover["contents"]["value"]
            .as_str()
            .is_some_and(|value| { value.contains("let transform: closure (i32): i32") })
    );

    let signature = &response_with_id(&messages, 3)["result"];
    assert_eq!(
        signature["signatures"][0]["label"],
        json!("method &Unit.keep<i32>(value: i32): i32")
    );

    let completion_items = response_with_id(&messages, 4)["result"]["items"]
        .as_array()
        .expect("expected recovered completion items");
    let value = completion_item_with_label(completion_items, "value")
        .expect("expected field completion inside unclosed closure");
    assert_eq!(value["detail"], json!("field Box.value: i32"));
}

#[test]
fn lsp_command_exits_with_failure_without_shutdown() {
    let project = TempProject::new("cli-lsp-exit-without-shutdown");

    let output = nocter_lsp(
        &project,
        &[json!({
            "jsonrpc": "2.0",
            "method": "exit",
            "params": null
        })],
    );

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout:\n{}",
        text(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr:\n{}",
        text(&output.stderr)
    );
}

#[test]
fn lsp_command_rejects_requests_after_shutdown_and_ignores_notifications() {
    let project = TempProject::new("cli-lsp-shutdown-state");
    let bad_text = "func main(: i32 {\n";
    let source = project.write_source("bad.nct", bad_text);
    let uri = file_uri(&source);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "shutdown",
                "params": null
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": {
                        "uri": uri.clone()
                    },
                    "position": {
                        "line": 0,
                        "character": 0
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri,
                        "languageId": "nocter",
                        "version": 1,
                        "text": bad_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }),
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );

    let messages = read_frames(&output.stdout);
    let response = response_with_id(&messages, 3);
    assert_eq!(response["error"]["code"], json!(-32600));
    assert_eq!(
        response["error"]["message"],
        json!("server is shutting down")
    );
    assert!(
        messages
            .iter()
            .all(|message| message["method"] != "textDocument/publishDiagnostics"),
        "shutdown should suppress later diagnostics, got {messages:#?}"
    );
}

fn nocter_lsp(project: &TempProject, messages: &[Value]) -> Output {
    let mut child = Command::new(NOCTER)
        .arg("lsp")
        .current_dir(project.root())
        .env("NOCTER_HOME", project.nocter_home())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        for message in messages {
            write_frame(stdin, message);
        }
    }
    drop(child.stdin.take());

    child.wait_with_output().unwrap()
}

fn response_with_id(messages: &[Value], id: u64) -> &Value {
    messages
        .iter()
        .find(|message| message["id"] == json!(id))
        .unwrap_or_else(|| panic!("expected response id {id}, got:\n{messages:#?}"))
}

fn write_frame<W: Write>(writer: &mut W, message: &Value) {
    let body = serde_json::to_vec(message).unwrap();
    write!(writer, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    writer.write_all(&body).unwrap();
}

fn read_frames(bytes: &[u8]) -> Vec<Value> {
    let mut messages = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        let header_end = find_header_end(&bytes[index..]).expect("expected LSP header") + index;
        let header = std::str::from_utf8(&bytes[index..header_end]).unwrap();
        let content_length = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .expect("expected Content-Length header");
        let body_start = header_end + 4;
        let body_end = body_start + content_length;
        messages.push(serde_json::from_slice(&bytes[body_start..body_end]).unwrap());
        index = body_end;
    }

    messages
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

const SEMANTIC_TOKEN_TYPE: usize = 4;
const SEMANTIC_TOKEN_PROPERTY: usize = 5;
const LSP_COMPLETION_ITEM_KIND_ENUM_MEMBER: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecodedSemanticToken {
    line: usize,
    character: usize,
    length: usize,
    kind: usize,
}

impl DecodedSemanticToken {
    fn lexeme<'a>(&self, text: &'a str) -> Option<&'a str> {
        let line = text.lines().nth(self.line)?;
        line.get(self.character..self.character + self.length)
    }
}

fn decode_semantic_tokens(values: &[Value]) -> Vec<DecodedSemanticToken> {
    let mut tokens = Vec::new();
    let mut line = 0usize;
    let mut character = 0usize;

    for chunk in values.chunks_exact(5) {
        let delta_line = chunk[0].as_u64().expect("expected delta line") as usize;
        let delta_character = chunk[1].as_u64().expect("expected delta character") as usize;
        line += delta_line;
        if delta_line == 0 {
            character += delta_character;
        } else {
            character = delta_character;
        }

        tokens.push(DecodedSemanticToken {
            line,
            character,
            length: chunk[2].as_u64().expect("expected token length") as usize,
            kind: chunk[3].as_u64().expect("expected token kind") as usize,
        });
    }

    tokens
}

fn token_starting_at<'a>(
    tokens: &'a [DecodedSemanticToken],
    text: &str,
    start_byte: usize,
) -> Option<&'a DecodedSemanticToken> {
    let prefix = text.get(..start_byte)?;
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let character = prefix
        .rsplit_once('\n')
        .map(|(_, line)| line.len())
        .unwrap_or(prefix.len());

    tokens
        .iter()
        .find(|token| token.line == line && token.character == character)
}

fn completion_item_with_label<'a>(items: &'a [Value], label: &str) -> Option<&'a Value> {
    items
        .iter()
        .find(|item| item["label"].as_str() == Some(label))
}

fn lsp_position_for_ascii_byte_offset(text: &str, start_byte: usize) -> (usize, usize) {
    let prefix = text.get(..start_byte).expect("byte offset must be valid");
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let character = prefix
        .rsplit_once('\n')
        .map(|(_, line)| line.len())
        .unwrap_or(prefix.len());
    (line, character)
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(unique_name(name));
        fs::create_dir_all(&root).unwrap();

        let project = Self { root };
        project.write_nocter_home();
        project
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn nocter_home(&self) -> PathBuf {
        self.root.join(".nocter")
    }

    fn write_source(&self, name: &str, text: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, text).unwrap();
        path
    }

    fn write_nocter_home_file(&self, relative: &str, text: &str) {
        let path = self.nocter_home().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    fn write_nocter_home(&self) {
        let home = self.nocter_home();
        fs::create_dir_all(home.join("std")).unwrap();
        fs::write(home.join("std/prelude.nct"), "").unwrap();
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unique_name(name: &str) -> String {
    format!(
        "nocter-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}
