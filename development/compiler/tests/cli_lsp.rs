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

    let definition = &response_with_id(&messages, 4)["result"];
    assert_eq!(definition["uri"], json!(uri));
    assert_eq!(definition["range"]["start"]["line"], json!(1));
    assert_eq!(definition["range"]["start"]["character"], json!(5));

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
