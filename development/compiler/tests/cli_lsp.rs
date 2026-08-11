use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const NOCTER: &str = env!("CARGO_BIN_EXE_nocter");

#[path = "support/builtin_std.rs"]
mod builtin_std;

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
        "index.nct",
        "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
    );
    let config = project.write_source(
        "config/index.nct",
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
    let source = project.write_source("index.nct", source_text);
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
    let source = project.write_source("index.nct", source_text);
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
    let source = project.write_source("index.nct", source_text);
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
    let source = project.write_source("index.nct", source_text);
    project.write_source(
        "factory/index.nct",
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
    let source = project.write_source("index.nct", source_text);
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
fn lsp_references_include_closed_declared_target_modules() {
    let project = TempProject::new("cli-lsp-package-references");
    project.write_source(
        "nocter.nct",
        r#"#executable: { name: "app", module: "." }
#executable: { name: "other", module: "./other" }
"#,
    );
    let app_text = "use ./lib.answer\n\nfunc main(): i32 {\n    return answer()\n}\n";
    let app = project.write_source("index.nct", app_text);
    let other = project.write_source(
        "other/index.nct",
        "use ../lib.answer\n\nfunc main(): i32 { return answer() }\n",
    );
    project.write_source("lib/index.nct", "pub func answer(): i32 { return 42 }\n");
    let app_uri = file_uri(&app);
    let other_uri = file_uri(&other.canonicalize().unwrap());
    let root_uri = file_uri(project.root());

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": { "rootUri": root_uri }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": app_uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": app_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/references",
                "params": {
                    "textDocument": { "uri": app_uri },
                    "position": { "line": 3, "character": 12 },
                    "context": { "includeDeclaration": true }
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
    let references = response_with_id(&messages, 2)["result"]
        .as_array()
        .expect("expected reference locations");
    assert!(
        references
            .iter()
            .any(|reference| reference["uri"] == other_uri),
        "closed target reference missing: {references:#?}"
    );
}

#[test]
fn lsp_rename_plans_versioned_package_wide_edits() {
    let project = TempProject::new("cli-lsp-package-rename");
    project.write_source(
        "nocter.nct",
        r#"#executable: { name: "app", module: "." }
#executable: { name: "other", module: "./other" }
"#,
    );
    let app_text = "use ./lib.answer\n\nfunc main(): i32 {\n    return answer()\n}\n";
    let app = project.write_source("index.nct", app_text);
    let other = project.write_source(
        "other/index.nct",
        "use ../lib.answer\n\nfunc main(): i32 { return answer() }\n",
    );
    project.write_source(
        "lib/index.nct",
        "pub func answer(): i32 { return 42 }\npub func replacement(): i32 { return 0 }\n",
    );
    let app_uri = file_uri(&app);
    let other_uri = file_uri(&other.canonicalize().unwrap());

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": { "rootUri": file_uri(project.root()) }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": app_uri.clone(),
                        "languageId": "nocter",
                        "version": 9,
                        "text": app_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/prepareRename",
                "params": {
                    "textDocument": { "uri": app_uri.clone() },
                    "position": { "line": 3, "character": 12 }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/rename",
                "params": {
                    "textDocument": { "uri": app_uri.clone() },
                    "position": { "line": 3, "character": 12 },
                    "newName": "renamed"
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "textDocument/rename",
                "params": {
                    "textDocument": { "uri": app_uri.clone() },
                    "position": { "line": 3, "character": 12 },
                    "newName": "replacement"
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "textDocument/rename",
                "params": {
                    "textDocument": { "uri": app_uri },
                    "position": { "line": 3, "character": 12 },
                    "newName": "func"
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 6,
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
    let prepared = &response_with_id(&messages, 2)["result"];
    assert_eq!(prepared["placeholder"], "answer");
    assert_eq!(prepared["range"]["start"]["line"], 3);
    assert_eq!(prepared["range"]["start"]["character"], 11);

    let changes = response_with_id(&messages, 3)["result"]["documentChanges"]
        .as_array()
        .expect("expected package-wide document changes");
    assert_eq!(changes.len(), 3, "changes: {changes:#?}");
    let open_change = changes
        .iter()
        .find(|change| change["textDocument"]["uri"] == file_uri(&app))
        .expect("expected open app edits");
    assert_eq!(open_change["textDocument"]["version"], 9);
    assert_eq!(open_change["edits"].as_array().unwrap().len(), 2);
    let closed_change = changes
        .iter()
        .find(|change| change["textDocument"]["uri"] == other_uri)
        .expect("expected closed target edits");
    assert_eq!(closed_change["textDocument"]["version"], Value::Null);
    assert_eq!(closed_change["edits"].as_array().unwrap().len(), 2);
    assert!(changes.iter().all(|change| {
        change["edits"]
            .as_array()
            .is_some_and(|edits| edits.iter().all(|edit| edit["newText"] == "renamed"))
    }));

    assert_eq!(response_with_id(&messages, 4)["result"], Value::Null);
    assert_eq!(response_with_id(&messages, 5)["result"], Value::Null);
}

#[test]
fn lsp_rename_never_edits_dependency_owned_declarations() {
    let project = TempProject::new("cli-lsp-dependency-rename");
    project.write_source(
        "nocter.nct",
        r#"#dependencies: { math: { path: "./packages/math" } }
#executable: { name: "app", module: "." }
"#,
    );
    fs::create_dir_all(project.root().join("packages/math")).unwrap();
    project.write_source("packages/math/nocter.nct", "#name: \"math\"\n");
    project.write_source(
        "packages/math/index.nct",
        "pub func answer(): i32 { return 42 }\n",
    );
    let app_text = "use math.answer\n\nfunc main(): i32 { return answer() }\n";
    let app = project.write_source("index.nct", app_text);
    let app_uri = file_uri(&app);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": { "rootUri": file_uri(project.root()) }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": app_uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": app_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/prepareRename",
                "params": {
                    "textDocument": { "uri": app_uri.clone() },
                    "position": { "line": 2, "character": 27 }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/rename",
                "params": {
                    "textDocument": { "uri": app_uri },
                    "position": { "line": 2, "character": 27 },
                    "newName": "renamed"
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

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );
    let messages = read_frames(&output.stdout);
    assert_eq!(response_with_id(&messages, 2)["result"], Value::Null);
    assert_eq!(response_with_id(&messages, 3)["result"], Value::Null);
}

#[test]
fn lsp_completion_adds_imports_from_reachable_visible_exports() {
    let project = TempProject::new("cli-lsp-auto-import");
    project.write_source(
        "nocter.nct",
        r#"#executable: { name: "app", module: "." }
#executable: { name: "catalog", module: "./catalog" }
"#,
    );
    let app_text = "/// Runs.\nfunc main(): i32 {\n    return ans\n}\n";
    let app = project.write_source("index.nct", app_text);
    project.write_source(
        "catalog/index.nct",
        "use ../lib.answer\nfunc catalog(): i32 { return answer() }\n",
    );
    project.write_source(
        "lib/index.nct",
        "pub func answer(): i32 { return 42 }\npub(/) func answer_package(): i32 { return 43 }\npub(./) func answer_subtree(): i32 { return 44 }\nfunc hidden(): i32 { return 0 }\n",
    );
    project.write_source(
        "unused/index.nct",
        "pub func answer_from_unreachable_file(): i32 { return 0 }\n",
    );
    project.write_source(
        "widened/index.nct",
        "pub use ../lib.answer_subtree as leaked_answer\n",
    );
    let app_uri = file_uri(&app);

    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": { "rootUri": file_uri(project.root()) }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": app_uri.clone(),
                        "languageId": "nocter",
                        "version": 1,
                        "text": app_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": { "uri": app_uri },
                    "position": { "line": 2, "character": 14 }
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
    let items = response_with_id(&messages, 2)["result"]["items"]
        .as_array()
        .expect("expected completion items");
    let answer = completion_item_with_label(items, "answer").expect("expected auto import");
    assert_eq!(answer["detail"], "auto import from /lib");
    assert_eq!(
        answer["additionalTextEdits"][0]["newText"],
        "use /lib.answer\n\n"
    );
    assert_eq!(
        answer["additionalTextEdits"][0]["range"]["start"]["line"],
        0
    );
    let package_answer = completion_item_with_label(items, "answer_package")
        .expect("expected package-visible auto import");
    assert_eq!(package_answer["detail"], "auto import from /lib");
    assert!(completion_item_with_label(items, "answer_subtree").is_none());
    assert!(
        completion_item_with_label(items, "leaked_answer").is_none(),
        "an invalid widening re-export must not enter the semantic package index"
    );
    assert!(completion_item_with_label(items, "hidden").is_none());
    assert!(completion_item_with_label(items, "answer_from_unreachable_file").is_none());
}

#[test]
fn lsp_code_actions_share_import_interface_and_outcome_edit_planners() {
    let project = TempProject::new("cli-lsp-code-actions");
    project.write_source(
        "nocter.nct",
        r#"#executable: { name: "app", module: "." }
#executable: { name: "catalog", module: "./catalog" }
"#,
    );
    let app_text = r#"interface Printable {
    pub method &self.print(): i32
}

struct User { id: i32 }

conform Printable for User {}

func run(): i32 {
    return fallible()?
}

func fallible(): i32! {
    return 1
}

func main(): i32 {
    return external()
}
"#;
    let app = project.write_source("index.nct", app_text);
    project.write_source(
        "catalog/index.nct",
        "use ../lib.external\nfunc catalog(): i32 { return external() }\n",
    );
    project.write_source("lib/index.nct", "pub func external(): i32 { return 42 }\n");
    let app_uri = file_uri(&app);
    let external = app_text.rfind("external").unwrap();
    let impl_target = app_text.find("for User").unwrap() + 4;
    let propagation = app_text.find("fallible()?").unwrap() + "fallible()".len();
    let external_position = lsp_position_for_ascii_byte_offset(app_text, external);
    let impl_position = lsp_position_for_ascii_byte_offset(app_text, impl_target);
    let propagation_position = lsp_position_for_ascii_byte_offset(app_text, propagation);

    let action_request = |id, position: (usize, usize)| {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/codeAction",
            "params": {
                "textDocument": { "uri": app_uri.clone() },
                "range": {
                    "start": { "line": position.0, "character": position.1 },
                    "end": { "line": position.0, "character": position.1 + 1 }
                },
                "context": { "diagnostics": [] }
            }
        })
    };
    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": { "rootUri": file_uri(project.root()) }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": app_uri.clone(),
                        "languageId": "nocter",
                        "version": 3,
                        "text": app_text
                    }
                }
            }),
            action_request(2, external_position),
            action_request(3, impl_position),
            action_request(4, propagation_position),
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
    let import_actions = response_with_id(&messages, 2)["result"]
        .as_array()
        .expect("expected import actions");
    assert_eq!(import_actions.len(), 1, "{import_actions:#?}");
    assert_eq!(
        import_actions[0]["edit"]["documentChanges"][0]["edits"][0]["newText"],
        "use /lib.external\n\n"
    );

    let interface_actions = response_with_id(&messages, 3)["result"]
        .as_array()
        .expect("expected interface actions");
    let member_text = interface_actions[0]["edit"]["documentChanges"][0]["edits"][0]["newText"]
        .as_str()
        .unwrap();
    assert!(member_text.contains("method &self.print(): i32"));
    assert!(member_text.contains("loop {}"));

    let outcome_actions = response_with_id(&messages, 4)["result"]
        .as_array()
        .expect("expected outcome actions");
    assert_eq!(
        outcome_actions[0]["edit"]["documentChanges"][0]["edits"][0]["newText"],
        "!"
    );
}

#[test]
fn lsp_inlay_hints_publish_inferred_types_from_snapshot_facts() {
    let project = TempProject::new("cli-lsp-inlay-hints");
    let source_text = "func main(): i32 {\n    let value = 42\n    return value\n}\n";
    let source = project.write_source("index.nct", source_text);
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
                "method": "textDocument/inlayHint",
                "params": {
                    "textDocument": { "uri": uri },
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 4, "character": 0 }
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
    let hints = response_with_id(&messages, 2)["result"]
        .as_array()
        .expect("expected inlay hints");
    let type_hint = hints
        .iter()
        .find(|hint| hint["label"] == ": i32")
        .expect("expected inferred binding type");
    assert_eq!(type_hint["kind"], 1);
    assert_eq!(type_hint["position"]["line"], 1);
    assert_eq!(type_hint["position"]["character"], 13);
}

#[test]
fn lsp_inlay_hints_do_not_invent_result_contracts() {
    let project = TempProject::new("cli-lsp-provenance-inlay-anchor");
    let source_text = "func label(): &str {\n    return \"static\"\n}\n";
    let source = project.write_source("index.nct", source_text);
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
                "method": "textDocument/inlayHint",
                "params": {
                    "textDocument": { "uri": uri },
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 3, "character": 0 }
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
    let hints = response_with_id(&messages, 2)["result"]
        .as_array()
        .expect("inlay hints");
    assert!(
        hints.iter().all(|hint| {
            hint["label"] != " from inferred storage" && hint["label"] != " allocates"
        }),
        "compiler-only result facts leaked into source hints: {hints:?}"
    );
}

#[test]
fn lsp_does_not_offer_removed_result_allocation_contract_edits() {
    let project = TempProject::new("cli-lsp-result-allocation-quick-fix");
    project.write_source(
        "nocter.nct",
        r#"#executable: { name: "app", module: "." }
"#,
    );
    let source_text = r#"struct Buffer { ptr: *u8 }
interface Factory { pub method &self.make(): Buffer }

func make<F>(factory: &F): Buffer where F: Factory { return factory.make() }
"#;
    let source = project.write_source("index.nct", source_text);
    let uri = file_uri(&source);
    let output = nocter_lsp(
        &project,
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": { "rootUri": file_uri(project.root()) }
            }),
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "nocter",
                        "version": 7,
                        "text": source_text
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "textDocument/codeAction",
                "params": {
                    "textDocument": { "uri": uri },
                    "range": {
                        "start": { "line": 3, "character": 5 },
                        "end": { "line": 3, "character": 9 }
                    },
                    "context": { "diagnostics": [] }
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
    let actions = response_with_id(&messages, 2)["result"]
        .as_array()
        .expect("code actions");
    assert!(
        actions.is_empty(),
        "obsolete allocation actions: {actions:?}"
    );
}

#[test]
fn lsp_command_presents_native_tests_without_making_them_callable() {
    let project = TempProject::new("cli-lsp-native-tests");
    let source_text = "/// Verifies push behavior.\ntest pushes {\n    return\n}\n\n";
    let source = project.write_source("index.nct", source_text);
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
                "params": { "textDocument": { "uri": uri.clone() } }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": uri.clone() },
                    "position": { "line": 1, "character": 7 }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": uri.clone() },
                    "position": { "line": 1, "character": 7 }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "textDocument/semanticTokens/full",
                "params": { "textDocument": { "uri": uri.clone() } }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 6,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": { "uri": uri.clone() },
                    "position": { "line": 4, "character": 0 }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 7,
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
    let symbol = symbols
        .iter()
        .find(|symbol| symbol["name"] == "pushes")
        .expect("expected native test symbol");
    assert_eq!(symbol["kind"], 12);
    assert_eq!(symbol["selectionRange"]["start"]["line"], 1);
    assert_eq!(symbol["selectionRange"]["start"]["character"], 5);
    assert_eq!(symbol["selectionRange"]["end"]["character"], 11);

    let hover_response = &response_with_id(&messages, 3)["result"];
    let hover = hover_response["contents"]["value"]
        .as_str()
        .expect("expected native test hover");
    assert!(hover.contains("test pushes: void!"), "hover:\n{hover}");
    assert!(hover.contains("Verifies push behavior."), "hover:\n{hover}");
    assert_eq!(hover_response["range"]["start"]["character"], 5);
    assert_eq!(hover_response["range"]["end"]["character"], 11);

    let definitions = response_with_id(&messages, 4)["result"]
        .as_array()
        .expect("expected definition links");
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0]["targetUri"], uri);
    assert_eq!(
        definitions[0]["targetSelectionRange"]["start"]["character"],
        5
    );
    assert_eq!(
        definitions[0]["targetSelectionRange"]["end"]["character"],
        11
    );

    let semantic_data = response_with_id(&messages, 5)["result"]["data"]
        .as_array()
        .expect("expected semantic token data");
    let tokens = decode_semantic_tokens(semantic_data);
    let test_name = tokens
        .iter()
        .find(|token| token.lexeme(source_text) == Some("pushes"))
        .expect("expected test-name semantic token");
    assert_eq!(test_name.kind, 0);
    assert!(
        tokens
            .iter()
            .all(|token| token.lexeme(source_text) != Some("test"))
    );

    let completion_items = response_with_id(&messages, 6)["result"]["items"]
        .as_array()
        .expect("expected completion items");
    assert!(completion_item_with_label(completion_items, "test").is_some());
    assert!(completion_item_with_label(completion_items, "pushes").is_none());
}

#[test]
fn lsp_hover_preserves_nested_process_result_and_static_provenance() {
    let project = TempProject::new("cli-lsp-process-env-hover");
    project.write_nocter_home_file(
        "std/process/index.nct",
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
        hover_text.contains("func lookup(name: &str): &str?! from static")
            && !hover_text.contains("Result provenance:"),
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

conform Read<T> for Box<T> {
    method &self.read(): &T from self {
        return &self.value
    }
}

conform Measure for Box<T> {
    method &self.measure(): usize {
        return 1
    }
}

func borrow<B, T>(value: &B): &T from value where B: Read<T> + Measure {
    return value.read()
}
"#;
    let source = project.write_source("index.nct", source_text);
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
    pub method &self.keep<T>(value: T): T from value {
        return value
    }
}

copy struct Unit { marker: i32 }
conform Identity for Unit {}

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
    let source = project.write_source("index.nct", source_text);
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
        json!("method &Unit.keep<i32>(value: i32): i32 from value")
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

#[test]
fn lsp_conversion_hover_and_definition_use_exact_as_ranges() {
    let project = TempProject::new("cli-lsp-conversion-plans");
    let source_text = r#"struct Text { value: &str }
coerce Text {
    pub &self as &str from self { return self.value }
}
func project(value: &Text): &str from value { return value as &str }
func widen(): i64 { return 1 as i64 }
"#;
    let source = project.write_source("index.nct", source_text);
    let uri = file_uri(&source);
    let coercion_operator = source_text.rfind("as &str").unwrap();
    let numeric_operator = source_text.rfind("as i64").unwrap();
    let (coercion_line, coercion_character) =
        lsp_position_for_ascii_byte_offset(source_text, coercion_operator);
    let (numeric_line, numeric_character) =
        lsp_position_for_ascii_byte_offset(source_text, numeric_operator);

    let output = nocter_lsp(
        &project,
        &[
            initialize_request(1),
            did_open_notification(&uri, source_text),
            text_document_position_request(
                2,
                "textDocument/hover",
                &uri,
                coercion_line,
                coercion_character,
            ),
            text_document_position_request(
                3,
                "textDocument/definition",
                &uri,
                coercion_line,
                coercion_character,
            ),
            text_document_position_request(
                4,
                "textDocument/hover",
                &uri,
                numeric_line,
                numeric_character,
            ),
            text_document_position_request(
                5,
                "textDocument/definition",
                &uri,
                numeric_line,
                numeric_character,
            ),
            shutdown_request(6),
            exit_notification(),
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
    assert_eq!(hover["range"]["start"]["line"], coercion_line);
    assert_eq!(hover["range"]["start"]["character"], coercion_character);
    assert_eq!(hover["range"]["end"]["character"], coercion_character + 2);
    let hover_text = hover["contents"]["value"].as_str().unwrap();
    assert!(hover_text.contains("&Text as &str"), "hover:\n{hover_text}");
    assert!(
        hover_text.contains("type-owned borrow coercion"),
        "hover:\n{hover_text}"
    );

    let definition = &response_with_id(&messages, 3)["result"][0];
    assert_eq!(definition["targetUri"], uri);
    assert_eq!(definition["originSelectionRange"], hover["range"]);
    assert_eq!(definition["targetSelectionRange"]["start"]["line"], 2);
    assert_eq!(definition["targetSelectionRange"]["start"]["character"], 14);
    assert_eq!(definition["targetSelectionRange"]["end"]["character"], 16);

    let numeric_hover = &response_with_id(&messages, 4)["result"];
    assert_eq!(numeric_hover["range"]["start"]["line"], numeric_line);
    assert_eq!(
        numeric_hover["range"]["start"]["character"],
        numeric_character
    );
    assert_eq!(
        numeric_hover["range"]["end"]["character"],
        numeric_character + 2
    );
    assert!(
        numeric_hover["contents"]["value"]
            .as_str()
            .is_some_and(|value| value.contains("lossless integer conversion"))
    );
    assert_eq!(response_with_id(&messages, 5)["result"], Value::Null);
}

#[test]
fn lsp_conversion_definition_crosses_a_public_reexport() {
    let project = TempProject::new("cli-lsp-reexported-coercion");
    let app_text = r#"use ./api.Text
func project(value: &Text): &str from value { return value as &str }
"#;
    let model_text = r#"pub struct Text { value: &str }
coerce Text { pub &self as &str from self { return self.value } }
"#;
    let app = project.write_source("index.nct", app_text);
    project.write_source("api/index.nct", "pub use ../model.Text\n");
    let model = project.write_source("model/index.nct", model_text);
    let app_uri = file_uri(&app);
    let model_uri = file_uri(&model.canonicalize().unwrap());
    let operator = app_text.rfind("as &str").unwrap();
    let (line, character) = lsp_position_for_ascii_byte_offset(app_text, operator);

    let output = nocter_lsp(
        &project,
        &[
            initialize_request(1),
            did_open_notification(&app_uri, app_text),
            text_document_position_request(2, "textDocument/hover", &app_uri, line, character),
            text_document_position_request(3, "textDocument/definition", &app_uri, line, character),
            shutdown_request(4),
            exit_notification(),
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
    assert_eq!(hover["range"]["start"]["line"], line);
    assert_eq!(hover["range"]["start"]["character"], character);
    assert!(
        hover["contents"]["value"]
            .as_str()
            .is_some_and(|value| value.contains("&Text as &str"))
    );
    let definition = &response_with_id(&messages, 3)["result"][0];
    assert_eq!(definition["targetUri"], model_uri);
    assert_eq!(definition["targetSelectionRange"]["start"]["line"], 1);
    assert_eq!(definition["targetSelectionRange"]["start"]["character"], 24);
    assert_eq!(definition["targetSelectionRange"]["end"]["character"], 26);
}

#[test]
fn lsp_definition_crosses_a_same_module_source_edge() {
    let project = TempProject::new("cli-lsp-same-module-source");
    let index_text = "use ./search\n\nfunc main(): i32 {\n    return answer()\n}\n";
    let search_text =
        "/// Returns the answer from a private module source.\nfunc answer(): i32 { return 42 }\n";
    let index = project.write_source("index.nct", index_text);
    let search = project.root().join("search.nct");
    let index_uri = file_uri(&index);
    let search_uri = file_uri(&search);
    let call = index_text.rfind("answer").unwrap();
    let (line, character) = lsp_position_for_ascii_byte_offset(index_text, call);

    let output = nocter_lsp(
        &project,
        &[
            initialize_request(1),
            did_open_notification(&index_uri, index_text),
            did_open_notification(&search_uri, search_text),
            text_document_position_request(2, "textDocument/hover", &index_uri, line, character),
            text_document_position_request(
                3,
                "textDocument/definition",
                &index_uri,
                line,
                character,
            ),
            shutdown_request(4),
            exit_notification(),
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
        response_with_id(&messages, 2)["result"]["contents"]["value"]
            .as_str()
            .is_some_and(|hover| hover.contains("func answer(): i32")),
        "messages: {messages:#?}"
    );
    let definition = &response_with_id(&messages, 3)["result"][0];
    assert_eq!(definition["targetUri"], search_uri);
    assert_eq!(definition["targetSelectionRange"]["start"]["line"], 1);
    assert_eq!(definition["targetSelectionRange"]["start"]["character"], 5);
}

#[test]
fn lsp_conversion_queries_remain_stable_for_private_and_incomplete_sources() {
    let private_project = TempProject::new("cli-lsp-private-coercion");
    let private_app_text = r#"use ./model.Text
func project(value: &Text): &str from value { return value as &str }
"#;
    let private_app = private_project.write_source("index.nct", private_app_text);
    private_project.write_source(
        "model/index.nct",
        "pub struct Text { value: &str }\ncoerce Text { &self as &str from self { return self.value } }\n",
    );
    let private_uri = file_uri(&private_app);
    let private_operator = private_app_text.rfind("as &str").unwrap();
    let (private_line, private_character) =
        lsp_position_for_ascii_byte_offset(private_app_text, private_operator);
    let private_output = nocter_lsp(
        &private_project,
        &[
            initialize_request(1),
            did_open_notification(&private_uri, private_app_text),
            text_document_position_request(
                2,
                "textDocument/hover",
                &private_uri,
                private_line,
                private_character,
            ),
            text_document_position_request(
                3,
                "textDocument/definition",
                &private_uri,
                private_line,
                private_character,
            ),
            shutdown_request(4),
            exit_notification(),
        ],
    );
    assert_eq!(
        private_output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&private_output.stderr)
    );
    let private_messages = read_frames(&private_output.stdout);
    assert_eq!(
        response_with_id(&private_messages, 2)["result"],
        Value::Null
    );
    assert_eq!(
        response_with_id(&private_messages, 3)["result"],
        Value::Null
    );
    assert!(private_messages.iter().any(|message| {
        message["method"] == "textDocument/publishDiagnostics"
            && message["params"]["diagnostics"]
                .as_array()
                .is_some_and(|diagnostics| {
                    diagnostics.iter().any(|diagnostic| {
                        diagnostic["message"]
                            .as_str()
                            .is_some_and(|text| text.contains("not accessible here"))
                    })
                })
    }));

    let incomplete_project = TempProject::new("cli-lsp-incomplete-coercion");
    let incomplete_text = r#"struct Text { value: &str }
coerce Text { pub &self as &str from self { return self.value } }
func project(value: &Text): &str from value { return value as &
"#;
    let incomplete_source = incomplete_project.write_source("index.nct", incomplete_text);
    let incomplete_uri = file_uri(&incomplete_source);
    let incomplete_operator = incomplete_text.rfind("as &").unwrap();
    let (incomplete_line, incomplete_character) =
        lsp_position_for_ascii_byte_offset(incomplete_text, incomplete_operator);
    let incomplete_output = nocter_lsp(
        &incomplete_project,
        &[
            initialize_request(1),
            did_open_notification(&incomplete_uri, incomplete_text),
            text_document_position_request(
                2,
                "textDocument/hover",
                &incomplete_uri,
                incomplete_line,
                incomplete_character,
            ),
            text_document_position_request(
                3,
                "textDocument/definition",
                &incomplete_uri,
                incomplete_line,
                incomplete_character,
            ),
            shutdown_request(4),
            exit_notification(),
        ],
    );
    assert_eq!(
        incomplete_output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&incomplete_output.stderr)
    );
    let incomplete_messages = read_frames(&incomplete_output.stdout);
    assert!(incomplete_messages.iter().any(|message| message["id"] == 2));
    assert!(incomplete_messages.iter().any(|message| message["id"] == 3));
    assert!(incomplete_messages.iter().any(|message| {
        message["method"] == "textDocument/publishDiagnostics"
            && message["params"]["diagnostics"]
                .as_array()
                .is_some_and(|diagnostics| !diagnostics.is_empty())
    }));
}

fn initialize_request(id: u64) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "initialize", "params": {} })
}

fn did_open_notification(uri: &str, source: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": uri,
                "languageId": "nocter",
                "version": 1,
                "text": source
            }
        }
    })
}

fn text_document_position_request(
    id: u64,
    method: &str,
    uri: &str,
    line: usize,
    character: usize,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        }
    })
}

fn shutdown_request(id: u64) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "shutdown", "params": null })
}

fn exit_notification() -> Value {
    json!({ "jsonrpc": "2.0", "method": "exit", "params": null })
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

#[test]
fn lsp_exposes_type_owned_construction_surfaces() {
    let project = TempProject::new("cli-lsp-construction-surfaces");
    let source_text = r#"pub struct Bucket<T> { pub value: T }
primitive stop(): never

construct Bucket<T> {
    pub default func new(value: T): Self { return Bucket<T> { value: value } }
    pub literal [](...items: T): Self from items {
        for item in items { return Bucket.new(move item) }
        return stop()
    }
}

func main(): i32 {
    let value = Bucket.new(1)
    return 0
}
"#;
    let source = project.write_source("index.nct", source_text);
    let uri = file_uri(&source);
    let hover_offset = source_text.find("struct Bucket").unwrap() + "struct ".len();
    let completion_offset = source_text.rfind("Bucket.new").unwrap() + "Bucket.".len();
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
                    "textDocument": { "uri": uri.clone() },
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

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        text(&output.stderr)
    );
    let messages = read_frames(&output.stdout);
    let hover = response_with_id(&messages, 2)["result"]["contents"]["value"]
        .as_str()
        .expect("expected hover contents");
    assert!(hover.contains("struct Bucket<T>"), "hover:\n{hover}");
    assert!(hover.contains("**Construction**"), "hover:\n{hover}");
    assert!(
        hover.contains("default func Bucket<T>.new(value: T): Bucket<T>"),
        "hover:\n{hover}"
    );
    assert!(
        hover.contains("literal Bucket<T> [](...items: T): Bucket<T> from items"),
        "hover:\n{hover}"
    );

    let items = response_with_id(&messages, 3)["result"]["items"]
        .as_array()
        .expect("expected completion items");
    let constructor =
        completion_item_with_label(items, "new").expect("expected constructor completion");
    assert_eq!(constructor["kind"], json!(4));
    assert_eq!(
        constructor["detail"],
        json!("func Bucket<T>.new(value: T): Bucket<T>")
    );
}

#[test]
fn lsp_equality_operator_uses_exact_source_identity_and_range() {
    let project = TempProject::new("cli-lsp-equality-operator");
    let source_text = r#"struct Text { value: i32 }
instance Text {
    pub operator (&self == other: &Self): bool {
        return self.value == other.value
    }
}
func equal(left: &Text, right: &Text): bool { return left == right }
func main(): i32 { return 0 }
"#;
    let source = project.write_source("index.nct", source_text);
    let uri = file_uri(&source);
    let declaration_offset = source_text.find("== other").unwrap();
    let use_offset = source_text.rfind("== right").unwrap();
    let (declaration_line, declaration_character) =
        lsp_position_for_ascii_byte_offset(source_text, declaration_offset);
    let (use_line, use_character) = lsp_position_for_ascii_byte_offset(source_text, use_offset);

    let output = nocter_lsp(
        &project,
        &[
            initialize_request(1),
            did_open_notification(&uri, source_text),
            text_document_position_request(2, "textDocument/hover", &uri, use_line, use_character),
            text_document_position_request(
                3,
                "textDocument/definition",
                &uri,
                use_line,
                use_character,
            ),
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "textDocument/references",
                "params": {
                    "textDocument": { "uri": uri.clone() },
                    "position": {
                        "line": declaration_line,
                        "character": declaration_character
                    },
                    "context": { "includeDeclaration": true }
                }
            }),
            shutdown_request(5),
            exit_notification(),
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
    assert_eq!(hover["range"]["start"]["line"], use_line);
    assert_eq!(hover["range"]["start"]["character"], use_character);
    assert_eq!(hover["range"]["end"]["character"], use_character + 2);
    assert!(
        hover["contents"]["value"]
            .as_str()
            .is_some_and(|value| value.contains("operator (&Text == other: &Text): bool"))
    );

    let definition = &response_with_id(&messages, 3)["result"][0];
    assert_eq!(definition["targetUri"], uri);
    assert_eq!(
        definition["targetSelectionRange"]["start"]["line"],
        declaration_line
    );
    assert_eq!(
        definition["targetSelectionRange"]["start"]["character"],
        declaration_character
    );
    assert_eq!(
        definition["targetSelectionRange"]["end"]["character"],
        declaration_character + 2
    );

    let references = response_with_id(&messages, 4)["result"]
        .as_array()
        .expect("expected equality references");
    assert_eq!(references.len(), 2, "{references:#?}");
    assert_eq!(references[0]["range"]["start"]["line"], declaration_line);
    assert_eq!(references[1]["range"]["start"]["line"], use_line);
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
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
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
        fs::create_dir_all(home.join("std/prelude")).unwrap();
        fs::write(home.join("std/prelude/index.nct"), "").unwrap();
        builtin_std::write_builtin_type_surfaces(&home);
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
