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
        fs::create_dir_all(home.join("targets/arm64-darwin/std")).unwrap();
        fs::write(home.join("std/prelude.nct"), "pub type Int = i32\n").unwrap();
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
