use crate::analysis::analyze_compile_unit_with_entry;
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::source::{JsonSpan, SourceMap};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub(super) fn run_lsp() -> ExitCode {
    let stdin = io::stdin();
    let stdout = io::stdout();
    match run_lsp_stream(stdin.lock(), stdout.lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lsp error: {error}");
            ExitCode::from(3)
        }
    }
}

fn run_lsp_stream<R: BufRead, W: Write>(mut reader: R, mut writer: W) -> io::Result<()> {
    let mut server = LspServer::new();

    while let Some(message) = read_message(&mut reader)? {
        if server.handle_message(message, &mut writer)? {
            break;
        }
    }

    Ok(())
}

struct LspServer {
    documents: HashMap<String, OpenDocument>,
    shutdown_requested: bool,
}

impl LspServer {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
            shutdown_requested: false,
        }
    }

    fn handle_message<W: Write>(&mut self, message: Value, writer: &mut W) -> io::Result<bool> {
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let id = message.get("id").cloned();

        if let Some(id) = id {
            return self.handle_request(id, method, writer);
        }

        self.handle_notification(method, message.get("params"), writer)
    }

    fn handle_request<W: Write>(
        &mut self,
        id: Value,
        method: &str,
        writer: &mut W,
    ) -> io::Result<bool> {
        match method {
            "initialize" => {
                write_message(writer, initialize_response(id))?;
                Ok(false)
            }
            "shutdown" => {
                self.shutdown_requested = true;
                write_message(writer, response(id, Value::Null))?;
                Ok(false)
            }
            _ => {
                write_message(
                    writer,
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32601,
                            "message": format!("method `{method}` is not supported by nocter lsp v0")
                        }
                    }),
                )?;
                Ok(false)
            }
        }
    }

    fn handle_notification<W: Write>(
        &mut self,
        method: &str,
        params: Option<&Value>,
        writer: &mut W,
    ) -> io::Result<bool> {
        match method {
            "initialized" => {}
            "exit" => return Ok(true),
            "textDocument/didOpen" => {
                if let Some(document) = open_document_from_params(params) {
                    self.publish_document_diagnostics(&document, writer)?;
                    self.documents.insert(document.uri.clone(), document);
                }
            }
            "textDocument/didChange" => {
                if let Some((uri, version, text)) = changed_document_from_params(params)
                    && let Some(mut document) = self.documents.remove(&uri)
                {
                    if document.change_is_stale(version) {
                        self.documents.insert(uri, document);
                        return Ok(false);
                    }

                    document.version = version;
                    document.text = text;
                    self.publish_document_diagnostics(&document, writer)?;
                    self.documents.insert(uri, document);
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = document_uri_from_params(params) {
                    self.documents.remove(&uri);
                    write_message(writer, publish_diagnostics(&uri, Vec::new()))?;
                }
            }
            _ => {}
        }

        Ok(false)
    }

    fn publish_document_diagnostics<W: Write>(
        &self,
        document: &OpenDocument,
        writer: &mut W,
    ) -> io::Result<()> {
        let diagnostics = analyze_document(document);
        write_message(
            writer,
            publish_diagnostics(&document.uri, diagnostics_for_lsp(document, diagnostics)),
        )
    }
}

#[derive(Debug, Clone)]
struct OpenDocument {
    uri: String,
    version: Option<i64>,
    display_path: String,
    absolute_path: Option<PathBuf>,
    text: String,
}

impl OpenDocument {
    fn change_is_stale(&self, version: Option<i64>) -> bool {
        matches!((self.version, version), (Some(current), Some(next)) if next < current)
    }
}

fn open_document_from_params(params: Option<&Value>) -> Option<OpenDocument> {
    let text_document = params?.get("textDocument")?;
    let uri = text_document.get("uri")?.as_str()?.to_string();
    let version = text_document.get("version").and_then(Value::as_i64);
    let text = text_document.get("text")?.as_str()?.to_string();
    Some(open_document(uri, version, text))
}

fn changed_document_from_params(params: Option<&Value>) -> Option<(String, Option<i64>, String)> {
    let params = params?;
    let uri = params
        .get("textDocument")?
        .get("uri")?
        .as_str()?
        .to_string();
    let version = params
        .get("textDocument")?
        .get("version")
        .and_then(Value::as_i64);
    let text = params
        .get("contentChanges")?
        .as_array()?
        .last()?
        .get("text")?
        .as_str()?
        .to_string();
    Some((uri, version, text))
}

fn document_uri_from_params(params: Option<&Value>) -> Option<String> {
    params?
        .get("textDocument")?
        .get("uri")?
        .as_str()
        .map(str::to_string)
}

fn open_document(uri: String, version: Option<i64>, text: String) -> OpenDocument {
    let path = file_uri_to_path(&uri);
    let display_path = path
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| uri.clone());
    let absolute_path = path
        .as_ref()
        .map(|path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));

    OpenDocument {
        uri,
        version,
        display_path,
        absolute_path,
        text,
    }
}

fn analyze_document(document: &OpenDocument) -> Vec<Diagnostic> {
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let unit = match load_compile_unit(&mut sources, source, &FrontendOptions::default()) {
        Ok(unit) => unit,
        Err(diagnostics) => return diagnostics,
    };
    let analysis = analyze_compile_unit_with_entry(&sources, &unit, DEFAULT_ENTRY_NAME);
    analysis.diagnostics()
}

fn initialize_response(id: Value) -> Value {
    response(
        id,
        json!({
            "capabilities": {
                "positionEncoding": "utf-16",
                "textDocumentSync": {
                    "openClose": true,
                    "change": 1
                }
            },
            "serverInfo": {
                "name": "nocter",
                "version": crate::driver::VERSION
            }
        }),
    )
}

fn response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn publish_diagnostics(uri: &str, diagnostics: Vec<LspDiagnostic>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {
            "uri": uri,
            "diagnostics": diagnostics
        }
    })
}

#[derive(Debug, Clone, Serialize)]
struct LspDiagnostic {
    range: LspRange,
    severity: u8,
    code: String,
    source: &'static str,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct LspRange {
    start: LspPosition,
    end: LspPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct LspPosition {
    line: usize,
    character: usize,
}

fn diagnostics_for_lsp(
    document: &OpenDocument,
    diagnostics: Vec<Diagnostic>,
) -> Vec<LspDiagnostic> {
    diagnostics
        .into_iter()
        .filter_map(|diagnostic| diagnostic_for_lsp(document, diagnostic))
        .collect()
}

fn diagnostic_for_lsp(document: &OpenDocument, diagnostic: Diagnostic) -> Option<LspDiagnostic> {
    let span = diagnostic.primary_span.as_deref();
    if let Some(span) = span
        && !span_belongs_to_document(document, span)
    {
        return None;
    }

    let range = span
        .map(|span| range_for_span(&document.text, span))
        .unwrap_or_else(|| LspRange {
            start: LspPosition {
                line: 0,
                character: 0,
            },
            end: LspPosition {
                line: 0,
                character: 0,
            },
        });

    Some(LspDiagnostic {
        range,
        severity: 1,
        code: diagnostic.code,
        source: "nocter",
        message: diagnostic.message,
    })
}

fn span_belongs_to_document(document: &OpenDocument, span: &JsonSpan) -> bool {
    if let (Some(document_path), Some(span_path)) = (&document.absolute_path, &span.absolute_path) {
        return Path::new(span_path) == document_path;
    }

    span.file == document.display_path || span.file == document.uri
}

fn range_for_span(text: &str, span: &JsonSpan) -> LspRange {
    LspRange {
        start: byte_offset_to_lsp_position(text, span.start_byte),
        end: byte_offset_to_lsp_position(text, span.end_byte),
    }
}

fn byte_offset_to_lsp_position(text: &str, offset: usize) -> LspPosition {
    let offset = offset.min(text.len());
    let mut line = 0;
    let mut line_start = 0;

    for (index, byte) in text.bytes().enumerate() {
        if index >= offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
            line_start = index + 1;
        }
    }

    let character = text
        .get(line_start..offset)
        .map(|line_text| line_text.encode_utf16().count())
        .unwrap_or(0);

    LspPosition { line, character }
}

fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    Some(PathBuf::from(percent_decode(rest)))
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3])
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            output.push(byte);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
    }

    let Some(content_length) = content_length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "LSP message missing Content-Length header",
        ));
    };

    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map(Some).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid LSP JSON message: {error}"),
        )
    })
}

fn write_message<W: Write>(writer: &mut W, message: Value) -> io::Result<()> {
    let body = serde_json::to_vec(&message).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to serialize LSP message: {error}"),
        )
    })?;

    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn decodes_file_uri_percent_encoding() {
        assert_eq!(
            file_uri_to_path("file:///tmp/nocter%20test/app.nct"),
            Some(PathBuf::from("/tmp/nocter test/app.nct"))
        );
    }

    #[test]
    fn converts_byte_offsets_to_utf16_positions() {
        let text = "a\néx\n";
        assert_eq!(
            byte_offset_to_lsp_position(text, 0),
            LspPosition {
                line: 0,
                character: 0
            }
        );
        assert_eq!(
            byte_offset_to_lsp_position(text, 4),
            LspPosition {
                line: 1,
                character: 1
            }
        );
    }

    #[test]
    fn handles_initialize_request() {
        let mut output = Vec::new();

        run_lsp_stream(
            Cursor::new(frame(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }))),
            &mut output,
        )
        .unwrap();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("\"id\":1"));
        assert!(text.contains("\"textDocumentSync\""));
    }

    #[test]
    fn publishes_diagnostics_for_open_document() {
        let mut output = Vec::new();
        let input = frame(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///tmp/nocter-lsp-test.nct",
                    "languageId": "nocter",
                    "version": 1,
                    "text": "func main(: i32 {\n"
                }
            }
        }));

        run_lsp_stream(Cursor::new(input), &mut output).unwrap();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("textDocument/publishDiagnostics"));
        assert!(text.contains("E0200"));
    }

    #[test]
    fn ignores_stale_document_changes() {
        let mut output = Vec::new();
        let mut input = frame(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///tmp/nocter-lsp-stale.nct",
                    "languageId": "nocter",
                    "version": 2,
                    "text": "func main(): i32 {\n    return 0\n}\n"
                }
            }
        }));
        input.extend(frame(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {
                    "uri": "file:///tmp/nocter-lsp-stale.nct",
                    "version": 1
                },
                "contentChanges": [{
                    "text": "func main(: i32 {\n"
                }]
            }
        })));

        run_lsp_stream(Cursor::new(input), &mut output).unwrap();

        let text = String::from_utf8(output).unwrap();
        assert_eq!(text.matches("textDocument/publishDiagnostics").count(), 1);
        assert!(!text.contains("E0200"));
    }

    fn frame(message: &Value) -> Vec<u8> {
        let body = serde_json::to_vec(message).unwrap();
        let mut framed = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        framed.extend(body);
        framed
    }
}
