use crate::analysis::{CompileUnitAnalysis, FileAnalysis, analyze_compile_unit_with_entry};
use crate::ast::{
    AstFile, BindingStmt, Block, EnumDecl, Expr, FunctionDecl, ImplMember, Item, MethodDecl,
    Parameter, PrimitiveDecl, Stmt, StructDecl, StructField, TraitDecl,
};
use crate::comments::{DocumentationTarget, attach_documentation};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::lexer::{Keyword, Token, TokenKind, lex};
use crate::parser::parse;
use crate::resolve::{
    LocalSymbol, LocalSymbolKind, ResolveOutput, Symbol, SymbolKind, TypeSymbolKind, resolve,
};
use crate::source::{ByteSpan, JsonSpan, SourceMap};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
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
    published_diagnostic_uris: HashSet<String>,
    workspace_roots: Vec<WorkspaceRoot>,
    shutdown_requested: bool,
}

impl LspServer {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
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
            return self.handle_request(id, method, message.get("params"), writer);
        }

        self.handle_notification(method, message.get("params"), writer)
    }

    fn handle_request<W: Write>(
        &mut self,
        id: Value,
        method: &str,
        params: Option<&Value>,
        writer: &mut W,
    ) -> io::Result<bool> {
        match method {
            "initialize" => {
                self.workspace_roots = workspace_roots_from_initialize_params(params);
                write_message(writer, initialize_response(id))?;
                Ok(false)
            }
            "textDocument/semanticTokens/full" => {
                write_message(writer, self.semantic_tokens_response(id, params))?;
                Ok(false)
            }
            "textDocument/hover" => {
                write_message(writer, self.hover_response(id, params))?;
                Ok(false)
            }
            "textDocument/definition" => {
                write_message(writer, self.definition_response(id, params))?;
                Ok(false)
            }
            "textDocument/documentSymbol" => {
                write_message(writer, self.document_symbol_response(id, params))?;
                Ok(false)
            }
            "textDocument/completion" => {
                write_message(writer, self.completion_response(id, params))?;
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
                    let uri = document.uri.clone();
                    self.documents.insert(uri.clone(), document);
                    self.publish_workspace_diagnostics(&uri, writer)?;
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
                    self.documents.insert(uri.clone(), document);
                    self.publish_workspace_diagnostics(&uri, writer)?;
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = document_uri_from_params(params) {
                    self.documents.remove(&uri);
                    self.published_diagnostic_uris.remove(&uri);
                    write_message(writer, publish_diagnostics(&uri, Vec::new()))?;
                }
            }
            _ => {}
        }

        Ok(false)
    }

    fn publish_workspace_diagnostics<W: Write>(
        &mut self,
        root_uri: &str,
        writer: &mut W,
    ) -> io::Result<()> {
        let diagnostics_by_uri = analyze_workspace(root_uri, &self.documents);
        let current_uris = diagnostics_by_uri
            .iter()
            .map(|(uri, _)| uri.clone())
            .collect::<HashSet<_>>();

        for uri in self
            .published_diagnostic_uris
            .difference(&current_uris)
            .cloned()
            .collect::<Vec<_>>()
        {
            write_message(writer, publish_diagnostics(&uri, Vec::new()))?;
        }

        for (uri, diagnostics) in diagnostics_by_uri {
            write_message(writer, publish_diagnostics(&uri, diagnostics))?;
        }

        self.published_diagnostic_uris = current_uris;
        Ok(())
    }

    fn semantic_tokens_response(&self, id: Value, params: Option<&Value>) -> Value {
        let data = document_uri_from_params(params)
            .and_then(|uri| self.documents.get(&uri))
            .map(semantic_tokens_for_document)
            .unwrap_or_default();
        response(id, json!({ "data": data }))
    }

    fn hover_response(&self, id: Value, params: Option<&Value>) -> Value {
        let hover = document_uri_from_params(params).and_then(|uri| {
            self.workspace_hover_for_uri(&uri, params).or_else(|| {
                self.documents
                    .get(&uri)
                    .and_then(|document| hover_for_document(document, params))
            })
        });
        response(id, hover.unwrap_or(Value::Null))
    }

    fn definition_response(&self, id: Value, params: Option<&Value>) -> Value {
        let definition = document_uri_from_params(params).and_then(|uri| {
            self.workspace_definition_for_uri(&uri, params).or_else(|| {
                self.documents
                    .get(&uri)
                    .and_then(|document| definition_for_document(document, params))
            })
        });
        response(id, definition.unwrap_or(Value::Null))
    }

    fn document_symbol_response(&self, id: Value, params: Option<&Value>) -> Value {
        let symbols = document_uri_from_params(params)
            .and_then(|uri| self.documents.get(&uri))
            .and_then(document_symbols_for_document)
            .unwrap_or_default();
        response(id, Value::Array(symbols))
    }

    fn completion_response(&self, id: Value, params: Option<&Value>) -> Value {
        let items = document_uri_from_params(params)
            .and_then(|uri| {
                self.workspace_completion_for_uri(&uri).or_else(|| {
                    self.documents
                        .get(&uri)
                        .and_then(completion_items_for_document)
                })
            })
            .unwrap_or_else(keyword_completion_items);
        response(
            id,
            json!({
                "isIncomplete": false,
                "items": items
            }),
        )
    }

    fn workspace_hover_for_uri(&self, uri: &str, params: Option<&Value>) -> Option<Value> {
        let position = position_from_params(params)?;
        let document = self.documents.get(uri)?;
        let root_offset =
            lsp_position_to_byte_offset(&document.text, position.line, position.character);
        let mut open_documents = self.documents.values().collect::<Vec<_>>();
        open_documents.sort_by(|left, right| left.uri.cmp(&right.uri));

        let mut sources = SourceMap::new();
        let mut source_by_uri = HashMap::new();

        for document in open_documents {
            let source = sources.add_source(
                document.display_path.clone(),
                document.absolute_path.clone(),
                document.text.clone(),
            );
            source_by_uri.insert(document.uri.clone(), source);
        }

        let root = source_by_uri.get(uri).copied()?;
        let options = frontend_options_for_document(document);
        let unit = load_compile_unit(&mut sources, root, &options).ok()?;
        let analysis = analyze_compile_unit_with_entry(&sources, &unit, DEFAULT_ENTRY_NAME);
        let file = analysis
            .files
            .iter()
            .find(|file| file.ast.span.source == root)?;

        hover_for_file_analysis(&sources, &analysis, file, root_offset)
    }

    fn workspace_definition_for_uri(&self, uri: &str, params: Option<&Value>) -> Option<Value> {
        let position = position_from_params(params)?;
        let document = self.documents.get(uri)?;
        let root_offset =
            lsp_position_to_byte_offset(&document.text, position.line, position.character);
        let mut open_documents = self.documents.values().collect::<Vec<_>>();
        open_documents.sort_by(|left, right| left.uri.cmp(&right.uri));

        let mut sources = SourceMap::new();
        let mut source_by_uri = HashMap::new();

        for document in open_documents {
            let source = sources.add_source(
                document.display_path.clone(),
                document.absolute_path.clone(),
                document.text.clone(),
            );
            source_by_uri.insert(document.uri.clone(), source);
        }

        let root = source_by_uri.get(uri).copied()?;
        let options = frontend_options_for_document(document);
        let unit = load_compile_unit(&mut sources, root, &options).ok()?;
        let analysis = analyze_compile_unit_with_entry(&sources, &unit, DEFAULT_ENTRY_NAME);
        let file = analysis
            .files
            .iter()
            .find(|file| file.ast.span.source == root)?;

        definition_for_file_analysis(&sources, file, root_offset)
    }

    fn workspace_completion_for_uri(&self, uri: &str) -> Option<Vec<Value>> {
        let document = self.documents.get(uri)?;
        let mut open_documents = self.documents.values().collect::<Vec<_>>();
        open_documents.sort_by(|left, right| left.uri.cmp(&right.uri));

        let mut sources = SourceMap::new();
        let mut source_by_uri = HashMap::new();

        for document in open_documents {
            let source = sources.add_source(
                document.display_path.clone(),
                document.absolute_path.clone(),
                document.text.clone(),
            );
            source_by_uri.insert(document.uri.clone(), source);
        }

        let root = source_by_uri.get(uri).copied()?;
        let options = frontend_options_for_document(document);
        let unit = load_compile_unit(&mut sources, root, &options).ok()?;
        let analysis = analyze_compile_unit_with_entry(&sources, &unit, DEFAULT_ENTRY_NAME);
        let file = analysis
            .files
            .iter()
            .find(|file| file.ast.span.source == root)?;

        Some(completion_items_for_file_analysis(file))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceRoot {
    uri: String,
    path: Option<PathBuf>,
}

fn workspace_roots_from_initialize_params(params: Option<&Value>) -> Vec<WorkspaceRoot> {
    let Some(params) = params else {
        return Vec::new();
    };

    if let Some(folders) = params.get("workspaceFolders").and_then(Value::as_array) {
        let roots = folders
            .iter()
            .filter_map(|folder| {
                folder
                    .get("uri")
                    .and_then(Value::as_str)
                    .map(workspace_root_from_uri)
            })
            .collect::<Vec<_>>();
        if !roots.is_empty() {
            return roots;
        }
    }

    params
        .get("rootUri")
        .and_then(Value::as_str)
        .map(workspace_root_from_uri)
        .into_iter()
        .collect()
}

fn workspace_root_from_uri(uri: &str) -> WorkspaceRoot {
    WorkspaceRoot {
        uri: uri.to_string(),
        path: file_uri_to_path(uri),
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

fn analyze_workspace(
    root_uri: &str,
    documents: &HashMap<String, OpenDocument>,
) -> Vec<(String, Vec<LspDiagnostic>)> {
    let mut open_documents = documents.values().collect::<Vec<_>>();
    open_documents.sort_by(|left, right| left.uri.cmp(&right.uri));

    let mut sources = SourceMap::new();
    let mut source_by_uri = HashMap::new();

    for document in &open_documents {
        let source = sources.add_source(
            document.display_path.clone(),
            document.absolute_path.clone(),
            document.text.clone(),
        );
        source_by_uri.insert(document.uri.clone(), source);
    }

    let diagnostics = match source_by_uri.get(root_uri).copied() {
        Some(root) => match documents
            .get(root_uri)
            .map(frontend_options_for_document)
            .map(|options| load_compile_unit(&mut sources, root, &options))
            .unwrap_or_else(|| load_compile_unit(&mut sources, root, &FrontendOptions::default()))
        {
            Ok(unit) => {
                analyze_compile_unit_with_entry(&sources, &unit, DEFAULT_ENTRY_NAME).diagnostics()
            }
            Err(diagnostics) => diagnostics,
        },
        None => Vec::new(),
    };

    open_documents
        .into_iter()
        .map(|document| {
            (
                document.uri.clone(),
                diagnostics_for_lsp(document, diagnostics.clone()),
            )
        })
        .collect()
}

fn frontend_options_for_document(document: &OpenDocument) -> FrontendOptions {
    FrontendOptions {
        nocter_home: document
            .absolute_path
            .as_deref()
            .and_then(find_nearest_nocter_home),
        ..FrontendOptions::default()
    }
}

fn find_nearest_nocter_home(path: &Path) -> Option<PathBuf> {
    let mut directory = path.parent();
    while let Some(current) = directory {
        let home = current.join(".nocter");
        if home.is_dir() {
            return Some(home);
        }
        directory = current.parent();
    }

    None
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
                },
                "semanticTokensProvider": {
                    "legend": {
                        "tokenTypes": SEMANTIC_TOKEN_TYPES,
                        "tokenModifiers": SEMANTIC_TOKEN_MODIFIERS
                    },
                    "full": true
                },
                "hoverProvider": true,
                "definitionProvider": true,
                "documentSymbolProvider": true,
                "completionProvider": {
                    "resolveProvider": false,
                    "triggerCharacters": [".", ":"]
                }
            },
            "serverInfo": {
                "name": "nocter",
                "version": crate::driver::VERSION
            }
        }),
    )
}

const SEMANTIC_TOKEN_TYPES: [&str; 5] = ["function", "variable", "parameter", "type", "property"];
const SEMANTIC_TOKEN_MODIFIERS: [&str; 1] = ["declaration"];
const SEMANTIC_DECLARATION_MODIFIER: u32 = 1 << 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemanticTokenKind {
    Function,
    Variable,
    Parameter,
    Type,
    Property,
}

impl SemanticTokenKind {
    const fn index(self) -> u32 {
        match self {
            SemanticTokenKind::Function => 0,
            SemanticTokenKind::Variable => 1,
            SemanticTokenKind::Parameter => 2,
            SemanticTokenKind::Type => 3,
            SemanticTokenKind::Property => 4,
        }
    }

    const fn hover_label(self) -> &'static str {
        match self {
            SemanticTokenKind::Function => "function",
            SemanticTokenKind::Variable => "variable",
            SemanticTokenKind::Parameter => "parameter",
            SemanticTokenKind::Type => "type",
            SemanticTokenKind::Property => "property",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemanticToken {
    start: LspPosition,
    length: usize,
    kind: SemanticTokenKind,
    modifiers: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClassifiedIdentifier {
    start_byte: usize,
    end_byte: usize,
    kind: SemanticTokenKind,
    modifiers: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HoverSymbol {
    name_span: ByteSpan,
    attach_start: usize,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResolvedReference {
    TopLevel(Box<Symbol>),
    Local(LocalSymbol),
}

impl ResolvedReference {
    fn declaration_span(&self) -> ByteSpan {
        match self {
            ResolvedReference::TopLevel(symbol) => symbol.declaration_span,
            ResolvedReference::Local(symbol) => symbol.name_span,
        }
    }
}

fn semantic_tokens_for_document(document: &OpenDocument) -> Vec<usize> {
    let semantic_tokens = classified_identifiers(document)
        .into_iter()
        .filter_map(|identifier| {
            let length = utf16_len(&document.text, identifier.start_byte, identifier.end_byte);
            (length > 0).then(|| SemanticToken {
                start: byte_offset_to_lsp_position(&document.text, identifier.start_byte),
                length,
                kind: identifier.kind,
                modifiers: identifier.modifiers,
            })
        })
        .collect::<Vec<_>>();

    encode_semantic_tokens(semantic_tokens)
}

fn classified_identifiers(document: &OpenDocument) -> Vec<ClassifiedIdentifier> {
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let lex_output = lex(&sources, source);
    let tokens = lex_output
        .tokens
        .iter()
        .filter(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
        .collect::<Vec<_>>();

    let mut identifiers = Vec::new();
    let mut pending_declaration = None;

    for (index, token) in tokens.iter().enumerate() {
        let previous = index
            .checked_sub(1)
            .and_then(|index| tokens.get(index))
            .copied();
        let next = tokens.get(index + 1).copied();

        match token.kind {
            TokenKind::Keyword(keyword) => {
                pending_declaration = pending_declaration_for_keyword(keyword);
            }
            TokenKind::Identifier => {
                let modifiers = if pending_declaration.is_some() {
                    SEMANTIC_DECLARATION_MODIFIER
                } else {
                    0
                };
                let kind = pending_declaration
                    .take()
                    .unwrap_or_else(|| classify_identifier(&document.text, token, previous, next));
                if token.span.start < token.span.end {
                    identifiers.push(ClassifiedIdentifier {
                        start_byte: token.span.start,
                        end_byte: token.span.end,
                        kind,
                        modifiers,
                    });
                }
            }
            _ => {
                if !matches!(
                    token.kind,
                    TokenKind::Punctuation("<")
                        | TokenKind::Punctuation(">")
                        | TokenKind::Punctuation(",")
                ) {
                    pending_declaration = None;
                }
            }
        }
    }

    identifiers.sort_by(|left, right| {
        left.start_byte
            .cmp(&right.start_byte)
            .then(left.end_byte.cmp(&right.end_byte))
    });
    identifiers
}

fn pending_declaration_for_keyword(keyword: Keyword) -> Option<SemanticTokenKind> {
    match keyword {
        Keyword::Func | Keyword::Method => Some(SemanticTokenKind::Function),
        Keyword::Type | Keyword::Struct | Keyword::Enum | Keyword::Trait | Keyword::Primitive => {
            Some(SemanticTokenKind::Type)
        }
        Keyword::Let | Keyword::Var => Some(SemanticTokenKind::Variable),
        _ => None,
    }
}

fn classify_identifier(
    text: &str,
    token: &Token,
    previous: Option<&Token>,
    next: Option<&Token>,
) -> SemanticTokenKind {
    if matches!(
        previous.map(|token| token.kind),
        Some(TokenKind::Punctuation("."))
    ) {
        return SemanticTokenKind::Property;
    }

    if matches!(
        next.map(|token| token.kind),
        Some(TokenKind::Punctuation("("))
    ) {
        return SemanticTokenKind::Function;
    }

    if matches!(
        next.map(|token| token.kind),
        Some(TokenKind::Punctuation(":"))
    ) {
        return SemanticTokenKind::Parameter;
    }

    let lexeme = text
        .get(token.span.start..token.span.end)
        .unwrap_or_default();
    if lexeme
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
    {
        return SemanticTokenKind::Type;
    }

    SemanticTokenKind::Variable
}

fn encode_semantic_tokens(tokens: Vec<SemanticToken>) -> Vec<usize> {
    let mut tokens = tokens;
    tokens.sort_by(|left, right| {
        left.start
            .line
            .cmp(&right.start.line)
            .then(left.start.character.cmp(&right.start.character))
    });

    let mut data = Vec::with_capacity(tokens.len() * 5);
    let mut previous_line = 0;
    let mut previous_character = 0;

    for token in tokens {
        let delta_line = token.start.line - previous_line;
        let delta_character = if delta_line == 0 {
            token.start.character - previous_character
        } else {
            token.start.character
        };

        data.push(delta_line);
        data.push(delta_character);
        data.push(token.length);
        data.push(token.kind.index() as usize);
        data.push(token.modifiers as usize);

        previous_line = token.start.line;
        previous_character = token.start.character;
    }

    data
}

fn utf16_len(text: &str, start: usize, end: usize) -> usize {
    text.get(start.min(text.len())..end.min(text.len()))
        .map(|text| text.encode_utf16().count())
        .unwrap_or(0)
}

fn hover_for_document(document: &OpenDocument, params: Option<&Value>) -> Option<Value> {
    let position = position_from_params(params)?;
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
    if let Some(hover) = documented_hover_for_document(document, offset) {
        return Some(hover);
    }

    let identifier = classified_identifiers(document)
        .into_iter()
        .find(|identifier| identifier.start_byte <= offset && offset < identifier.end_byte)?;
    let lexeme = document
        .text
        .get(identifier.start_byte..identifier.end_byte)?;
    let range = LspRange {
        start: byte_offset_to_lsp_position(&document.text, identifier.start_byte),
        end: byte_offset_to_lsp_position(&document.text, identifier.end_byte),
    };
    let declaration = if identifier.modifiers & SEMANTIC_DECLARATION_MODIFIER != 0 {
        " declaration"
    } else {
        ""
    };

    Some(json!({
        "contents": {
            "kind": "markdown",
            "value": format!("```nocter\n{}{} {}\n```", identifier.kind.hover_label(), declaration, lexeme)
        },
        "range": range
    }))
}

fn definition_for_document(document: &OpenDocument, params: Option<&Value>) -> Option<Value> {
    let position = position_from_params(params)?;
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let lex_output = lex(&sources, source);
    if !lex_output.diagnostics.is_empty() {
        return None;
    }
    let ast = parse(&sources, source, &lex_output.tokens).ast?;
    let resolved = resolve_single_file_for_hover(&document.text, source, &ast);
    definition_span_for_ast(&document.text, &ast, &resolved, offset)
        .and_then(|span| location_for_byte_span(&sources, span))
}

const LSP_SYMBOL_KIND_CLASS: u8 = 5;
const LSP_SYMBOL_KIND_METHOD: u8 = 6;
const LSP_SYMBOL_KIND_FIELD: u8 = 8;
const LSP_SYMBOL_KIND_ENUM: u8 = 10;
const LSP_SYMBOL_KIND_INTERFACE: u8 = 11;
const LSP_SYMBOL_KIND_FUNCTION: u8 = 12;
const LSP_SYMBOL_KIND_ENUM_MEMBER: u8 = 22;
const LSP_SYMBOL_KIND_STRUCT: u8 = 23;

const LSP_COMPLETION_ITEM_KIND_FUNCTION: u8 = 3;
const LSP_COMPLETION_ITEM_KIND_CLASS: u8 = 7;
const LSP_COMPLETION_ITEM_KIND_INTERFACE: u8 = 8;
const LSP_COMPLETION_ITEM_KIND_MODULE: u8 = 9;
const LSP_COMPLETION_ITEM_KIND_ENUM: u8 = 13;
const LSP_COMPLETION_ITEM_KIND_KEYWORD: u8 = 14;
const LSP_COMPLETION_ITEM_KIND_STRUCT: u8 = 22;

const KEYWORD_COMPLETIONS: [&str; 22] = [
    "from", "import", "use", "func", "pub", "type", "copy", "struct", "enum", "trait", "impl",
    "method", "let", "var", "return", "if", "else", "for", "in", "while", "match", "catch",
];

fn completion_items_for_file_analysis(file: &FileAnalysis) -> Vec<Value> {
    completion_items_for_resolved_symbols(&file.resolved)
}

fn completion_items_for_document(document: &OpenDocument) -> Option<Vec<Value>> {
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let lex_output = lex(&sources, source);
    if !lex_output.diagnostics.is_empty() {
        return None;
    }
    let ast = parse(&sources, source, &lex_output.tokens).ast?;
    let resolved = resolve_single_file_for_hover(&document.text, source, &ast);
    Some(completion_items_for_resolved_symbols(&resolved))
}

fn completion_items_for_resolved_symbols(resolved: &ResolveOutput) -> Vec<Value> {
    let mut items = keyword_completion_items();
    let mut seen = KEYWORD_COMPLETIONS
        .iter()
        .map(|keyword| (*keyword).to_string())
        .collect::<HashSet<_>>();

    let mut symbols = resolved.symbols.symbols().collect::<Vec<_>>();
    symbols.sort_by(|left, right| left.name.cmp(&right.name));

    for symbol in symbols {
        if !seen.insert(symbol.name.clone()) {
            continue;
        }
        items.push(completion_item(
            &symbol.name,
            completion_kind_for_symbol(symbol),
            Some(symbol_detail(symbol)),
        ));
    }

    items
}

fn keyword_completion_items() -> Vec<Value> {
    KEYWORD_COMPLETIONS
        .iter()
        .map(|keyword| {
            completion_item(
                keyword,
                LSP_COMPLETION_ITEM_KIND_KEYWORD,
                Some("keyword".to_string()),
            )
        })
        .collect()
}

fn completion_kind_for_symbol(symbol: &Symbol) -> u8 {
    match &symbol.kind {
        SymbolKind::Function(_) => LSP_COMPLETION_ITEM_KIND_FUNCTION,
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => LSP_COMPLETION_ITEM_KIND_CLASS,
            TypeSymbolKind::Struct => LSP_COMPLETION_ITEM_KIND_STRUCT,
            TypeSymbolKind::Enum => LSP_COMPLETION_ITEM_KIND_ENUM,
            TypeSymbolKind::Trait => LSP_COMPLETION_ITEM_KIND_INTERFACE,
        },
        SymbolKind::Imported(_) => LSP_COMPLETION_ITEM_KIND_MODULE,
    }
}

fn symbol_detail(symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(_) => "function".to_string(),
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => "type".to_string(),
            TypeSymbolKind::Struct => "struct".to_string(),
            TypeSymbolKind::Enum => "enum".to_string(),
            TypeSymbolKind::Trait => "trait".to_string(),
        },
        SymbolKind::Imported(imported) => format!("imported from {}", imported.path),
    }
}

fn completion_item(label: &str, kind: u8, detail: Option<String>) -> Value {
    let mut item = json!({
        "label": label,
        "kind": kind,
    });

    if let Some(detail) = detail
        && let Some(object) = item.as_object_mut()
    {
        object.insert("detail".to_string(), Value::String(detail));
    }

    item
}

fn document_symbols_for_document(document: &OpenDocument) -> Option<Vec<Value>> {
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let lex_output = lex(&sources, source);
    if !lex_output.diagnostics.is_empty() {
        return None;
    }
    let ast = parse(&sources, source, &lex_output.tokens).ast?;

    Some(
        ast.items
            .iter()
            .filter_map(|item| item_document_symbol(&document.text, item))
            .collect(),
    )
}

fn item_document_symbol(text: &str, item: &Item) -> Option<Value> {
    match item {
        Item::Use(_) | Item::Import(_) | Item::FromImport(_) => None,
        Item::Function(function) => Some(document_symbol(
            text,
            &function.name,
            LSP_SYMBOL_KIND_FUNCTION,
            function.span,
            function.name_span,
            Vec::new(),
        )),
        Item::Primitive(primitive) => Some(document_symbol(
            text,
            &primitive.name,
            LSP_SYMBOL_KIND_FUNCTION,
            primitive.span,
            primitive.name_span,
            Vec::new(),
        )),
        Item::TypeAlias(alias) => Some(document_symbol(
            text,
            &alias.name,
            LSP_SYMBOL_KIND_CLASS,
            alias.span,
            alias.name_span,
            Vec::new(),
        )),
        Item::Struct(struct_) => Some(document_symbol(
            text,
            &struct_.name,
            LSP_SYMBOL_KIND_STRUCT,
            struct_.span,
            struct_.name_span,
            struct_
                .fields
                .iter()
                .map(|field| {
                    document_symbol(
                        text,
                        &field.name,
                        LSP_SYMBOL_KIND_FIELD,
                        field.span,
                        field.name_span,
                        Vec::new(),
                    )
                })
                .collect(),
        )),
        Item::Enum(enum_) => Some(document_symbol(
            text,
            &enum_.name,
            LSP_SYMBOL_KIND_ENUM,
            enum_.span,
            enum_.name_span,
            enum_
                .variants
                .iter()
                .map(|variant| {
                    document_symbol(
                        text,
                        &variant.name,
                        LSP_SYMBOL_KIND_ENUM_MEMBER,
                        variant.span,
                        variant.name_span,
                        Vec::new(),
                    )
                })
                .collect(),
        )),
        Item::Trait(trait_) => Some(document_symbol(
            text,
            &trait_.name,
            LSP_SYMBOL_KIND_INTERFACE,
            trait_.span,
            trait_.name_span,
            trait_
                .methods
                .iter()
                .map(|method| method_document_symbol(text, method))
                .collect(),
        )),
        Item::Impl(impl_) => Some(document_symbol(
            text,
            &format!("impl {}", source_fragment(text, impl_.target_ty.span())),
            LSP_SYMBOL_KIND_CLASS,
            impl_.span,
            impl_.target_ty.span(),
            impl_
                .members
                .iter()
                .map(|member| impl_member_document_symbol(text, member))
                .collect(),
        )),
    }
}

fn impl_member_document_symbol(text: &str, member: &ImplMember) -> Value {
    match member {
        ImplMember::Function(function) => document_symbol(
            text,
            &function.name,
            LSP_SYMBOL_KIND_FUNCTION,
            function.span,
            function.name_span,
            Vec::new(),
        ),
        ImplMember::Method(method) => method_document_symbol(text, method),
    }
}

fn method_document_symbol(text: &str, method: &MethodDecl) -> Value {
    document_symbol(
        text,
        &method.name,
        LSP_SYMBOL_KIND_METHOD,
        method.span,
        method.name_span,
        Vec::new(),
    )
}

fn document_symbol(
    text: &str,
    name: &str,
    kind: u8,
    range_span: ByteSpan,
    selection_span: ByteSpan,
    children: Vec<Value>,
) -> Value {
    let mut symbol = json!({
        "name": name,
        "kind": kind,
        "range": range_for_byte_span(text, range_span),
        "selectionRange": range_for_byte_span(text, selection_span)
    });

    if !children.is_empty()
        && let Some(object) = symbol.as_object_mut()
    {
        object.insert("children".to_string(), Value::Array(children));
    }

    symbol
}

fn hover_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Value> {
    let source_file = sources.get(file.ast.span.source)?;
    let text = source_file.text();
    let symbols = hover_symbols_for_ast(text, &file.ast);
    let documentation = documentation_for_hover_symbols(file.ast.span.source, text, &symbols);

    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| symbol.name_span.start <= offset && offset < symbol.name_span.end)
    {
        let docs = documentation.get(symbol.name_span.start);
        return Some(json!({
            "contents": {
                "kind": "markdown",
                "value": hover_markdown(&symbol.label, docs)
            },
            "range": range_for_byte_span(text, symbol.name_span)
        }));
    }

    find_resolved_hover_symbol(&file.ast, &file.resolved, offset).map(|(name_span, reference)| {
        let (label, docs) = resolved_reference_hover_contents(sources, analysis, &reference);

        json!({
            "contents": {
                "kind": "markdown",
                "value": hover_markdown(&label, docs.as_deref())
            },
            "range": range_for_byte_span(text, name_span)
        })
    })
}

fn definition_for_file_analysis(
    sources: &SourceMap,
    file: &FileAnalysis,
    offset: usize,
) -> Option<Value> {
    let text = sources.get(file.ast.span.source)?.text();
    definition_span_for_ast(text, &file.ast, &file.resolved, offset)
        .and_then(|span| location_for_byte_span(sources, span))
}

fn definition_span_for_ast(
    text: &str,
    ast: &AstFile,
    resolved: &ResolveOutput,
    offset: usize,
) -> Option<ByteSpan> {
    let symbols = hover_symbols_for_ast(text, ast);
    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| symbol.name_span.start <= offset && offset < symbol.name_span.end)
    {
        return Some(symbol.name_span);
    }

    find_resolved_hover_symbol(ast, resolved, offset)
        .map(|(_, reference)| reference.declaration_span())
}

fn location_for_byte_span(sources: &SourceMap, span: ByteSpan) -> Option<Value> {
    let source = sources.get(span.source)?;
    Some(json!({
        "uri": uri_for_source_file(source),
        "range": range_for_byte_span(source.text(), span)
    }))
}

fn uri_for_source_file(source: &crate::source::SourceFile) -> String {
    source
        .absolute_path()
        .map(|path| format!("file://{}", percent_encode_path(&path.to_string_lossy())))
        .unwrap_or_else(|| source.display_path().to_string())
}

fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn documented_hover_for_document(document: &OpenDocument, offset: usize) -> Option<Value> {
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let lex_output = lex(&sources, source);
    if !lex_output.diagnostics.is_empty() {
        return None;
    }
    let ast = parse(&sources, source, &lex_output.tokens).ast?;
    let symbols = hover_symbols_for_ast(&document.text, &ast);
    let documentation = documentation_for_hover_symbols(source, &document.text, &symbols);

    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| symbol.name_span.start <= offset && offset < symbol.name_span.end)
    {
        let docs = documentation.get(symbol.name_span.start);
        let value = hover_markdown(&symbol.label, docs);

        return Some(json!({
            "contents": {
                "kind": "markdown",
                "value": value
            },
            "range": range_for_byte_span(&document.text, symbol.name_span)
        }));
    }

    resolved_reference_hover_for_ast(&document.text, source, &ast, offset).map(
        |(name_span, reference)| {
            let (label, docs) = single_file_resolved_reference_hover_contents(
                &document.text,
                &symbols,
                &documentation,
                &reference,
            );

            json!({
                "contents": {
                    "kind": "markdown",
                    "value": hover_markdown(&label, docs.as_deref())
                },
                "range": range_for_byte_span(&document.text, name_span)
            })
        },
    )
}

fn documentation_for_hover_symbols(
    source: crate::source::SourceId,
    text: &str,
    symbols: &[HoverSymbol],
) -> crate::comments::AttachedDocumentation {
    let targets = symbols
        .iter()
        .map(|symbol| DocumentationTarget::new(symbol.attach_start, symbol.name_span.start))
        .collect::<Vec<_>>();
    attach_documentation(source, text, &targets)
}

fn resolved_reference_hover_for_ast(
    text: &str,
    source: crate::source::SourceId,
    ast: &AstFile,
    offset: usize,
) -> Option<(ByteSpan, ResolvedReference)> {
    let resolved = resolve_single_file_for_hover(text, source, ast);
    find_resolved_hover_symbol(ast, &resolved, offset)
}

fn resolve_single_file_for_hover(
    text: &str,
    source: crate::source::SourceId,
    ast: &AstFile,
) -> ResolveOutput {
    let mut sources = SourceMap::new();
    let hover_source = sources.add_source("hover.nct", None, text.to_string());
    debug_assert_eq!(hover_source.raw(), source.raw());
    resolve(&sources, ast)
}

fn find_resolved_hover_symbol<'a>(
    ast: &'a AstFile,
    resolved: &'a ResolveOutput,
    offset: usize,
) -> Option<(ByteSpan, ResolvedReference)> {
    let mut candidates = Vec::new();
    for item in &ast.items {
        collect_item_resolved_hover_symbols(item, resolved, offset, &mut candidates);
    }
    candidates.sort_by_key(|(span, _)| (span.end - span.start, span.start));
    candidates.into_iter().next()
}

fn collect_item_resolved_hover_symbols(
    item: &Item,
    resolved: &ResolveOutput,
    offset: usize,
    candidates: &mut Vec<(ByteSpan, ResolvedReference)>,
) {
    match item {
        Item::Function(function) => {
            collect_block_resolved_hover_symbols(&function.body, resolved, offset, candidates);
        }
        Item::Impl(impl_) => {
            for member in &impl_.members {
                match member {
                    ImplMember::Function(function) => {
                        collect_block_resolved_hover_symbols(
                            &function.body,
                            resolved,
                            offset,
                            candidates,
                        );
                    }
                    ImplMember::Method(method) => {
                        if let Some(body) = &method.body {
                            collect_block_resolved_hover_symbols(
                                body, resolved, offset, candidates,
                            );
                        }
                    }
                }
            }
        }
        Item::Use(_)
        | Item::Import(_)
        | Item::FromImport(_)
        | Item::Primitive(_)
        | Item::TypeAlias(_)
        | Item::Struct(_)
        | Item::Enum(_)
        | Item::Trait(_) => {}
    }
}

fn collect_block_resolved_hover_symbols(
    block: &Block,
    resolved: &ResolveOutput,
    offset: usize,
    candidates: &mut Vec<(ByteSpan, ResolvedReference)>,
) {
    for statement in &block.statements {
        collect_statement_resolved_hover_symbols(statement, resolved, offset, candidates);
    }
}

fn collect_statement_resolved_hover_symbols(
    statement: &Stmt,
    resolved: &ResolveOutput,
    offset: usize,
    candidates: &mut Vec<(ByteSpan, ResolvedReference)>,
) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_resolved_hover_symbols(expression, resolved, offset, candidates);
            }
        }
        Stmt::Binding(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.initializer,
                resolved,
                offset,
                candidates,
            );
            if let Some(block) = &statement.else_block {
                collect_block_resolved_hover_symbols(block, resolved, offset, candidates);
            }
        }
        Stmt::Assignment(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.target,
                resolved,
                offset,
                candidates,
            );
            collect_expression_resolved_hover_symbols(
                &statement.value,
                resolved,
                offset,
                candidates,
            );
        }
        Stmt::If(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.condition,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(
                &statement.then_block,
                resolved,
                offset,
                candidates,
            );
            if let Some(block) = &statement.else_block {
                collect_block_resolved_hover_symbols(block, resolved, offset, candidates);
            }
        }
        Stmt::IfIs(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.expression,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(
                &statement.then_block,
                resolved,
                offset,
                candidates,
            );
            if let Some(block) = &statement.else_block {
                collect_block_resolved_hover_symbols(block, resolved, offset, candidates);
            }
        }
        Stmt::IfLet(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.initializer,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(
                &statement.then_block,
                resolved,
                offset,
                candidates,
            );
            if let Some(block) = &statement.else_block {
                collect_block_resolved_hover_symbols(block, resolved, offset, candidates);
            }
        }
        Stmt::Switch(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.expression,
                resolved,
                offset,
                candidates,
            );
            for arm in &statement.arms {
                collect_block_resolved_hover_symbols(&arm.body, resolved, offset, candidates);
            }
            if let Some(arm) = &statement.else_arm {
                collect_block_resolved_hover_symbols(&arm.body, resolved, offset, candidates);
            }
        }
        Stmt::ForRange(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.start,
                resolved,
                offset,
                candidates,
            );
            collect_expression_resolved_hover_symbols(&statement.end, resolved, offset, candidates);
            collect_block_resolved_hover_symbols(&statement.body, resolved, offset, candidates);
        }
        Stmt::While(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.condition,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(&statement.body, resolved, offset, candidates);
        }
        Stmt::WhileLet(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.initializer,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(&statement.body, resolved, offset, candidates);
        }
        Stmt::Loop(statement) => {
            collect_block_resolved_hover_symbols(&statement.body, resolved, offset, candidates);
        }
        Stmt::Expression(statement) => {
            collect_expression_resolved_hover_symbols(
                &statement.expression,
                resolved,
                offset,
                candidates,
            );
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn collect_expression_resolved_hover_symbols(
    expression: &Expr,
    resolved: &ResolveOutput,
    offset: usize,
    candidates: &mut Vec<(ByteSpan, ResolvedReference)>,
) {
    match expression {
        Expr::Identifier(expression) => {
            if span_contains(expression.span, offset) {
                if let Some(symbol) = resolved.local_symbol_for_identifier(expression) {
                    candidates.push((expression.span, ResolvedReference::Local(symbol.clone())));
                } else if let Some(symbol) = resolved.symbol_for_identifier(expression) {
                    candidates.push((
                        expression.span,
                        ResolvedReference::TopLevel(Box::new(symbol.clone())),
                    ));
                }
            }
        }
        Expr::Call(expression) => {
            if let Expr::Identifier(callee) = expression.callee.as_ref()
                && span_contains(callee.span, offset)
                && let Some(symbol) = resolved.symbol_for_call(expression)
            {
                candidates.push((
                    callee.span,
                    ResolvedReference::TopLevel(Box::new(symbol.clone())),
                ));
            }
            collect_expression_resolved_hover_symbols(
                &expression.callee,
                resolved,
                offset,
                candidates,
            );
            for argument in &expression.arguments {
                collect_expression_resolved_hover_symbols(argument, resolved, offset, candidates);
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_resolved_hover_symbols(element, resolved, offset, candidates);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression_resolved_hover_symbols(
                    &field.value,
                    resolved,
                    offset,
                    candidates,
                );
            }
        }
        Expr::Propagate(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.expression,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Force(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.expression,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Catch(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.expression,
                resolved,
                offset,
                candidates,
            );
            collect_block_resolved_hover_symbols(
                &expression.catch_block,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Unary(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.operand,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Binary(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.left,
                resolved,
                offset,
                candidates,
            );
            collect_expression_resolved_hover_symbols(
                &expression.right,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::TypeConversion(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.expression,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Member(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.object,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Index(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.object,
                resolved,
                offset,
                candidates,
            );
            collect_expression_resolved_hover_symbols(
                &expression.index,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::Group(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.expression,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::OptionalDefault(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.value,
                resolved,
                offset,
                candidates,
            );
            collect_expression_resolved_hover_symbols(
                &expression.default,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::PatternConditional(expression) => {
            collect_expression_resolved_hover_symbols(
                &expression.target,
                resolved,
                offset,
                candidates,
            );
            for arm in &expression.arms {
                collect_expression_resolved_hover_symbols(
                    &arm.expression,
                    resolved,
                    offset,
                    candidates,
                );
            }
            collect_expression_resolved_hover_symbols(
                &expression.fallback,
                resolved,
                offset,
                candidates,
            );
        }
        Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

fn hover_symbols_for_ast(text: &str, ast: &AstFile) -> Vec<HoverSymbol> {
    let mut symbols = Vec::new();
    for item in &ast.items {
        collect_item_hover_symbols(text, item, &mut symbols);
    }
    symbols
}

fn collect_item_hover_symbols(text: &str, item: &Item, symbols: &mut Vec<HoverSymbol>) {
    match item {
        Item::Use(_) | Item::Import(_) | Item::FromImport(_) => {}
        Item::Function(function) => {
            push_function_hover_symbol(text, function, symbols);
            collect_parameter_hover_symbols(text, &function.parameters.parameters, symbols);
            collect_block_hover_symbols(text, &function.body, symbols);
        }
        Item::Primitive(primitive) => {
            push_primitive_hover_symbol(text, primitive, symbols);
            collect_parameter_hover_symbols(text, &primitive.parameters.parameters, symbols);
        }
        Item::TypeAlias(alias) => push_hover_symbol(
            text,
            alias.name_span,
            alias.span.start,
            format!(
                "type {} = {}",
                alias.name,
                source_fragment(text, alias.target.span())
            ),
            symbols,
        ),
        Item::Struct(struct_) => collect_struct_hover_symbols(text, struct_, symbols),
        Item::Enum(enum_) => collect_enum_hover_symbols(text, enum_, symbols),
        Item::Trait(trait_) => collect_trait_hover_symbols(text, trait_, symbols),
        Item::Impl(impl_) => {
            for member in &impl_.members {
                match member {
                    ImplMember::Function(function) => {
                        push_function_hover_symbol(text, function, symbols);
                        collect_parameter_hover_symbols(
                            text,
                            &function.parameters.parameters,
                            symbols,
                        );
                        collect_block_hover_symbols(text, &function.body, symbols);
                    }
                    ImplMember::Method(method) => {
                        collect_method_hover_symbols(text, method, symbols)
                    }
                }
            }
        }
    }
}

fn collect_struct_hover_symbols(text: &str, struct_: &StructDecl, symbols: &mut Vec<HoverSymbol>) {
    let copy_prefix = if struct_.is_copy { "copy " } else { "" };
    push_hover_symbol(
        text,
        struct_.name_span,
        struct_.span.start,
        format!("{copy_prefix}struct {}", struct_.name),
        symbols,
    );
    for field in &struct_.fields {
        push_struct_field_hover_symbol(text, field, symbols);
    }
}

fn collect_enum_hover_symbols(text: &str, enum_: &EnumDecl, symbols: &mut Vec<HoverSymbol>) {
    push_hover_symbol(
        text,
        enum_.name_span,
        enum_.span.start,
        format!("enum {}", enum_.name),
        symbols,
    );
    for variant in &enum_.variants {
        let payload = if variant.payload.is_empty() {
            String::new()
        } else {
            format!("({})", parameters_label(text, &variant.payload))
        };
        push_hover_symbol(
            text,
            variant.name_span,
            variant.span.start,
            format!("variant {}{}", variant.name, payload),
            symbols,
        );
        collect_parameter_hover_symbols(text, &variant.payload, symbols);
    }
}

fn collect_trait_hover_symbols(text: &str, trait_: &TraitDecl, symbols: &mut Vec<HoverSymbol>) {
    push_hover_symbol(
        text,
        trait_.name_span,
        trait_.span.start,
        format!("trait {}", trait_.name),
        symbols,
    );
    for method in &trait_.methods {
        collect_method_hover_symbols(text, method, symbols);
    }
}

fn collect_method_hover_symbols(text: &str, method: &MethodDecl, symbols: &mut Vec<HoverSymbol>) {
    push_hover_symbol(
        text,
        method.name_span,
        method.span.start,
        function_like_header(
            text,
            method.span,
            method.body.as_ref().map(|body| body.span.start),
        ),
        symbols,
    );
    collect_parameter_hover_symbols(text, std::slice::from_ref(&method.receiver), symbols);
    collect_parameter_hover_symbols(text, &method.parameters.parameters, symbols);
    if let Some(body) = &method.body {
        collect_block_hover_symbols(text, body, symbols);
    }
}

fn push_function_hover_symbol(text: &str, function: &FunctionDecl, symbols: &mut Vec<HoverSymbol>) {
    push_hover_symbol(
        text,
        function.name_span,
        function.span.start,
        function_like_header(text, function.span, Some(function.body.span.start)),
        symbols,
    );
}

fn push_primitive_hover_symbol(
    text: &str,
    primitive: &PrimitiveDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol(
        text,
        primitive.name_span,
        primitive.span.start,
        function_like_header(text, primitive.span, None),
        symbols,
    );
}

fn push_struct_field_hover_symbol(text: &str, field: &StructField, symbols: &mut Vec<HoverSymbol>) {
    push_hover_symbol(
        text,
        field.name_span,
        field.span.start,
        format!(
            "field {}: {}",
            field.name,
            source_fragment(text, field.ty.span())
        ),
        symbols,
    );
}

fn collect_parameter_hover_symbols(
    text: &str,
    parameters: &[Parameter],
    symbols: &mut Vec<HoverSymbol>,
) {
    for parameter in parameters {
        push_hover_symbol(
            text,
            parameter.name_span,
            parameter.span.start,
            format!(
                "parameter {}: {}",
                parameter.name,
                source_fragment(text, parameter.ty.span())
            ),
            symbols,
        );
    }
}

fn collect_block_hover_symbols(text: &str, block: &Block, symbols: &mut Vec<HoverSymbol>) {
    for statement in &block.statements {
        collect_statement_hover_symbols(text, statement, symbols);
    }
}

fn collect_statement_hover_symbols(text: &str, statement: &Stmt, symbols: &mut Vec<HoverSymbol>) {
    match statement {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_hover_symbols(text, expression, symbols);
            }
        }
        Stmt::Binding(statement) => {
            push_binding_hover_symbol(text, statement, symbols);
            collect_expression_hover_symbols(text, &statement.initializer, symbols);
            if let Some(block) = &statement.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Stmt::Assignment(statement) => {
            collect_expression_hover_symbols(text, &statement.target, symbols);
            collect_expression_hover_symbols(text, &statement.value, symbols);
        }
        Stmt::If(statement) => {
            collect_expression_hover_symbols(text, &statement.condition, symbols);
            collect_block_hover_symbols(text, &statement.then_block, symbols);
            if let Some(block) = &statement.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Stmt::IfIs(statement) => {
            collect_block_hover_symbols(text, &statement.then_block, symbols);
            if let Some(block) = &statement.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Stmt::IfLet(statement) => {
            push_hover_symbol(
                text,
                statement.name_span,
                statement.span.start,
                format!("{} {}", binding_kind_label(statement.kind), statement.name),
                symbols,
            );
            collect_expression_hover_symbols(text, &statement.initializer, symbols);
            collect_block_hover_symbols(text, &statement.then_block, symbols);
            if let Some(block) = &statement.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Stmt::Switch(statement) => {
            collect_expression_hover_symbols(text, &statement.expression, symbols);
            for arm in &statement.arms {
                collect_block_hover_symbols(text, &arm.body, symbols);
            }
            if let Some(arm) = &statement.else_arm {
                collect_block_hover_symbols(text, &arm.body, symbols);
            }
        }
        Stmt::ForRange(statement) => {
            push_hover_symbol(
                text,
                statement.name_span,
                statement.span.start,
                format!("let {}", statement.name),
                symbols,
            );
            collect_expression_hover_symbols(text, &statement.start, symbols);
            collect_expression_hover_symbols(text, &statement.end, symbols);
            collect_block_hover_symbols(text, &statement.body, symbols);
        }
        Stmt::While(statement) => {
            collect_expression_hover_symbols(text, &statement.condition, symbols);
            collect_block_hover_symbols(text, &statement.body, symbols);
        }
        Stmt::WhileLet(statement) => {
            push_hover_symbol(
                text,
                statement.name_span,
                statement.span.start,
                format!("{} {}", binding_kind_label(statement.kind), statement.name),
                symbols,
            );
            collect_expression_hover_symbols(text, &statement.initializer, symbols);
            collect_block_hover_symbols(text, &statement.body, symbols);
        }
        Stmt::Loop(statement) => collect_block_hover_symbols(text, &statement.body, symbols),
        Stmt::Expression(statement) => {
            collect_expression_hover_symbols(text, &statement.expression, symbols);
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn push_binding_hover_symbol(text: &str, statement: &BindingStmt, symbols: &mut Vec<HoverSymbol>) {
    let ty = statement
        .ty
        .as_ref()
        .map(|ty| format!(": {}", source_fragment(text, ty.span())))
        .unwrap_or_default();
    push_hover_symbol(
        text,
        statement.name_span,
        statement.span.start,
        format!(
            "{} {}{}",
            binding_kind_label(statement.kind),
            statement.name,
            ty
        ),
        symbols,
    );
}

fn collect_expression_hover_symbols(text: &str, expression: &Expr, symbols: &mut Vec<HoverSymbol>) {
    match expression {
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_hover_symbols(text, element, symbols);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression_hover_symbols(text, &field.value, symbols);
            }
        }
        Expr::Propagate(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::Force(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::Catch(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
            collect_block_hover_symbols(text, &expression.catch_block, symbols);
        }
        Expr::Unary(expression) => {
            collect_expression_hover_symbols(text, &expression.operand, symbols)
        }
        Expr::Binary(expression) => {
            collect_expression_hover_symbols(text, &expression.left, symbols);
            collect_expression_hover_symbols(text, &expression.right, symbols);
        }
        Expr::TypeConversion(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::Call(expression) => {
            collect_expression_hover_symbols(text, &expression.callee, symbols);
            for argument in &expression.arguments {
                collect_expression_hover_symbols(text, argument, symbols);
            }
        }
        Expr::Member(expression) => {
            collect_expression_hover_symbols(text, &expression.object, symbols)
        }
        Expr::Index(expression) => {
            collect_expression_hover_symbols(text, &expression.object, symbols);
            collect_expression_hover_symbols(text, &expression.index, symbols);
        }
        Expr::Group(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::OptionalDefault(expression) => {
            collect_expression_hover_symbols(text, &expression.value, symbols);
            collect_expression_hover_symbols(text, &expression.default, symbols);
        }
        Expr::PatternConditional(expression) => {
            collect_expression_hover_symbols(text, &expression.target, symbols);
            for arm in &expression.arms {
                collect_expression_hover_symbols(text, &arm.expression, symbols);
            }
            collect_expression_hover_symbols(text, &expression.fallback, symbols);
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn push_hover_symbol(
    text: &str,
    name_span: ByteSpan,
    declaration_start: usize,
    label: String,
    symbols: &mut Vec<HoverSymbol>,
) {
    symbols.push(HoverSymbol {
        name_span,
        attach_start: declaration_line_start(text, declaration_start),
        label,
    });
}

fn function_like_header(text: &str, span: ByteSpan, body_start: Option<usize>) -> String {
    let end = body_start.unwrap_or(span.end).min(span.end);
    source_fragment(text, ByteSpan::new(span.source, span.start, end))
        .trim_end_matches('{')
        .trim()
        .to_string()
}

fn parameters_label(text: &str, parameters: &[Parameter]) -> String {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                source_fragment(text, parameter.ty.span())
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn binding_kind_label(kind: crate::ast::BindingKind) -> &'static str {
    match kind {
        crate::ast::BindingKind::Let => "let",
        crate::ast::BindingKind::Var => "var",
    }
}

fn source_fragment(text: &str, span: ByteSpan) -> &str {
    text.get(span.start.min(text.len())..span.end.min(text.len()))
        .unwrap_or_default()
        .trim()
}

fn hover_markdown(label: &str, documentation: Option<&str>) -> String {
    let mut value = format!("```nocter\n{label}\n```");
    if let Some(documentation) = documentation
        && !documentation.trim().is_empty()
    {
        value.push_str("\n\n");
        value.push_str(documentation.trim());
    }
    value
}

fn symbol_hover_label(text: &str, symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(signature) => format!(
            "func {}({}): {}",
            symbol.name,
            parameter_signatures_label(text, &signature.parameters),
            source_fragment(text, signature.return_type.span())
        ),
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => type_symbol
                .alias_target
                .as_ref()
                .map(|target| {
                    format!(
                        "type {} = {}",
                        symbol.name,
                        source_fragment(text, target.span())
                    )
                })
                .unwrap_or_else(|| format!("type {}", symbol.name)),
            TypeSymbolKind::Struct => format!("struct {}", symbol.name),
            TypeSymbolKind::Enum => format!("enum {}", symbol.name),
            TypeSymbolKind::Trait => format!("trait {}", symbol.name),
        },
        SymbolKind::Imported(imported) => format!("import {} from {}", symbol.name, imported.path),
    }
}

fn single_file_resolved_reference_hover_contents(
    text: &str,
    symbols: &[HoverSymbol],
    documentation: &crate::comments::AttachedDocumentation,
    reference: &ResolvedReference,
) -> (String, Option<String>) {
    match reference {
        ResolvedReference::TopLevel(symbol) => {
            let referenced = symbols
                .iter()
                .find(|candidate| candidate.name_span == symbol.name_span);
            let label = referenced
                .map(|symbol| symbol.label.clone())
                .unwrap_or_else(|| symbol_hover_label(text, symbol));
            let docs = referenced
                .and_then(|symbol| documentation.get(symbol.name_span.start))
                .map(str::to_string);
            (label, docs)
        }
        ResolvedReference::Local(symbol) => (local_symbol_hover_label(symbol), None),
    }
}

fn resolved_reference_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    reference: &ResolvedReference,
) -> (String, Option<String>) {
    match reference {
        ResolvedReference::TopLevel(symbol) => {
            resolved_symbol_hover_contents(sources, analysis, symbol).unwrap_or_else(|| {
                (
                    symbol_hover_label_for_sources(sources, symbol),
                    None::<String>,
                )
            })
        }
        ResolvedReference::Local(symbol) => (local_symbol_hover_label(symbol), None),
    }
}

fn local_symbol_hover_label(symbol: &LocalSymbol) -> String {
    match symbol.kind {
        LocalSymbolKind::Parameter => format!("parameter {}", symbol.name),
        LocalSymbolKind::Binding(kind) => {
            format!("{} {}", binding_kind_label(kind), symbol.name)
        }
        LocalSymbolKind::PatternPayload => format!("payload {}", symbol.name),
        LocalSymbolKind::CatchError => format!("catch {}", symbol.name),
        LocalSymbolKind::ForRange => format!("for {}", symbol.name),
    }
}

fn resolved_symbol_hover_contents(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    symbol: &Symbol,
) -> Option<(String, Option<String>)> {
    let file = analysis
        .files
        .iter()
        .find(|file| file.ast.span.source == symbol.declaration_span.source)?;
    let source_file = sources.get(file.ast.span.source)?;
    let text = source_file.text();
    let symbols = hover_symbols_for_ast(text, &file.ast);
    let hover_symbol = symbols
        .iter()
        .find(|candidate| candidate.name_span == symbol.declaration_span)
        .or_else(|| {
            symbols
                .iter()
                .find(|candidate| candidate.name_span == symbol.name_span)
        })?;
    let documentation = documentation_for_hover_symbols(file.ast.span.source, text, &symbols);
    let docs = documentation
        .get(hover_symbol.name_span.start)
        .map(str::to_string);

    Some((hover_symbol.label.clone(), docs))
}

fn symbol_hover_label_for_sources(sources: &SourceMap, symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(signature) => format!(
            "func {}({}): {}",
            symbol.name,
            parameter_signatures_label_for_sources(sources, &signature.parameters),
            source_fragment_from_sources(sources, signature.return_type.span())
        ),
        SymbolKind::Type(type_symbol) => match type_symbol.kind {
            TypeSymbolKind::Alias => type_symbol
                .alias_target
                .as_ref()
                .map(|target| {
                    format!(
                        "type {} = {}",
                        symbol.name,
                        source_fragment_from_sources(sources, target.span())
                    )
                })
                .unwrap_or_else(|| format!("type {}", symbol.name)),
            TypeSymbolKind::Struct => format!("struct {}", symbol.name),
            TypeSymbolKind::Enum => format!("enum {}", symbol.name),
            TypeSymbolKind::Trait => format!("trait {}", symbol.name),
        },
        SymbolKind::Imported(imported) => format!("import {} from {}", symbol.name, imported.path),
    }
}

fn parameter_signatures_label_for_sources(
    sources: &SourceMap,
    parameters: &[crate::resolve::ParameterSignature],
) -> String {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                source_fragment_from_sources(sources, parameter.ty.span())
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn source_fragment_from_sources(sources: &SourceMap, span: ByteSpan) -> String {
    sources
        .get(span.source)
        .map(|source| source_fragment(source.text(), span).to_string())
        .unwrap_or_default()
}

fn parameter_signatures_label(
    text: &str,
    parameters: &[crate::resolve::ParameterSignature],
) -> String {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                source_fragment(text, parameter.ty.span())
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn range_for_byte_span(text: &str, span: ByteSpan) -> LspRange {
    LspRange {
        start: byte_offset_to_lsp_position(text, span.start),
        end: byte_offset_to_lsp_position(text, span.end),
    }
}

fn declaration_line_start(text: &str, node_start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut line_start = node_start.min(bytes.len());
    while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
    }

    let mut start = line_start;
    while start < node_start && matches!(bytes[start], b' ' | b'\t') {
        start += 1;
    }

    start
}

fn position_from_params(params: Option<&Value>) -> Option<LspPosition> {
    let position = params?.get("position")?;
    Some(LspPosition {
        line: position.get("line")?.as_u64()? as usize,
        character: position.get("character")?.as_u64()? as usize,
    })
}

fn lsp_position_to_byte_offset(text: &str, line: usize, character: usize) -> usize {
    let mut current_line = 0;
    let mut line_start = 0;

    for (index, byte) in text.bytes().enumerate() {
        if current_line == line {
            break;
        }
        if byte == b'\n' {
            current_line += 1;
            line_start = index + 1;
        }
    }

    if current_line != line {
        return text.len();
    }

    let line_end = text[line_start..]
        .find('\n')
        .map(|offset| line_start + offset)
        .unwrap_or(text.len());
    let mut utf16_character = 0;

    for (offset, char) in text[line_start..line_end].char_indices() {
        if utf16_character >= character {
            return line_start + offset;
        }
        utf16_character += char.len_utf16();
        if utf16_character > character {
            return line_start + offset;
        }
    }

    line_end
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
        assert!(text.contains("\"semanticTokensProvider\""));
        assert!(text.contains("\"hoverProvider\""));
        assert!(text.contains("\"definitionProvider\""));
        assert!(text.contains("\"documentSymbolProvider\""));
        assert!(text.contains("\"completionProvider\""));
    }

    #[test]
    fn initializes_with_semantic_token_legend() {
        let response = initialize_response(json!(1));
        let legend = response["result"]["capabilities"]["semanticTokensProvider"]["legend"]
            .as_object()
            .expect("expected semantic token legend");

        assert_eq!(
            legend["tokenTypes"],
            json!(["function", "variable", "parameter", "type", "property"])
        );
        assert_eq!(legend["tokenModifiers"], json!(["declaration"]));
    }

    #[test]
    fn converts_utf16_positions_to_byte_offsets() {
        let text = "a\néx\n";
        assert_eq!(lsp_position_to_byte_offset(text, 0, 0), 0);
        assert_eq!(lsp_position_to_byte_offset(text, 1, 0), 2);
        assert_eq!(lsp_position_to_byte_offset(text, 1, 1), 4);
        assert_eq!(lsp_position_to_byte_offset(text, 1, 2), 5);
    }

    #[test]
    fn returns_semantic_tokens_for_open_document() {
        let uri = "file:///tmp/nocter-semantic.nct".to_string();
        let document = open_document(
            uri.clone(),
            Some(1),
            "func main(path: AppError): i32 {\n    let code = AppError.open_failed(path)\n    return code\n}\n"
                .to_string(),
        );
        let server = LspServer {
            documents: HashMap::from([(uri.clone(), document)]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.semantic_tokens_response(
            json!(2),
            Some(&json!({
                "textDocument": {
                    "uri": uri
                }
            })),
        );
        let data = response["result"]["data"]
            .as_array()
            .expect("expected semantic token data");

        assert!(!data.is_empty());
        assert_eq!(data.len() % 5, 0);
        assert_eq!(data[3], json!(SemanticTokenKind::Function.index()));
        assert_eq!(data[4], json!(SEMANTIC_DECLARATION_MODIFIER));
    }

    #[test]
    fn returns_hover_for_identifier() {
        let uri = "file:///tmp/nocter-hover.nct".to_string();
        let document = open_document(
            uri.clone(),
            Some(1),
            "func main(): i32 {\n    let answer = compute()\n    return answer\n}\n".to_string(),
        );
        let server = LspServer {
            documents: HashMap::from([(uri.clone(), document)]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.hover_response(
            json!(3),
            Some(&json!({
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": 1,
                    "character": 9
                }
            })),
        );

        assert_eq!(
            response["result"]["contents"]["value"],
            json!("```nocter\nlet answer\n```")
        );
        assert_eq!(response["result"]["range"]["start"]["line"], json!(1));
        assert_eq!(response["result"]["range"]["start"]["character"], json!(8));
    }

    #[test]
    fn returns_hover_for_local_reference() {
        let uri = "file:///tmp/nocter-hover-local-reference.nct".to_string();
        let document = open_document(
            uri.clone(),
            Some(1),
            "func main(path: str): i32 {\n    let code = 0\n    return code\n}\n".to_string(),
        );
        let server = LspServer {
            documents: HashMap::from([(uri.clone(), document)]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.hover_response(
            json!(4),
            Some(&json!({
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": 2,
                    "character": 12
                }
            })),
        );

        assert_eq!(
            response["result"]["contents"]["value"],
            json!("```nocter\nlet code\n```")
        );
        assert_eq!(response["result"]["range"]["start"]["line"], json!(2));
        assert_eq!(response["result"]["range"]["start"]["character"], json!(11));
    }

    #[test]
    fn returns_documented_hover_for_function_declaration() {
        let uri = "file:///tmp/nocter-hover-docs.nct".to_string();
        let document = open_document(
            uri.clone(),
            Some(1),
            "/// Computes the answer.\nfunc answer(path: str): i32 {\n    return 0\n}\n"
                .to_string(),
        );
        let server = LspServer {
            documents: HashMap::from([(uri.clone(), document)]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.hover_response(
            json!(4),
            Some(&json!({
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": 1,
                    "character": 6
                }
            })),
        );

        assert_eq!(
            response["result"]["contents"]["value"],
            json!("```nocter\nfunc answer(path: str): i32\n```\n\nComputes the answer.")
        );
        assert_eq!(response["result"]["range"]["start"]["line"], json!(1));
        assert_eq!(response["result"]["range"]["start"]["character"], json!(5));
    }

    #[test]
    fn returns_documented_hover_for_local_binding_declaration() {
        let uri = "file:///tmp/nocter-hover-local-docs.nct".to_string();
        let document = open_document(
            uri.clone(),
            Some(1),
            "func main(): i32 {\n    /// Exit code.\n    let code: i32 = 0\n    return code\n}\n"
                .to_string(),
        );
        let server = LspServer {
            documents: HashMap::from([(uri.clone(), document)]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.hover_response(
            json!(5),
            Some(&json!({
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": 2,
                    "character": 9
                }
            })),
        );

        assert_eq!(
            response["result"]["contents"]["value"],
            json!("```nocter\nlet code: i32\n```\n\nExit code.")
        );
    }

    #[test]
    fn returns_documented_hover_for_resolved_function_reference() {
        let uri = "file:///tmp/nocter-hover-reference-docs.nct".to_string();
        let document = open_document(
            uri.clone(),
            Some(1),
            "func main(): i32 {\n    return answer()\n}\n\n/// Computes the answer.\nfunc answer(): i32 {\n    return 42\n}\n"
                .to_string(),
        );
        let server = LspServer {
            documents: HashMap::from([(uri.clone(), document)]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.hover_response(
            json!(6),
            Some(&json!({
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": 1,
                    "character": 12
                }
            })),
        );

        assert_eq!(
            response["result"]["contents"]["value"],
            json!("```nocter\nfunc answer(): i32\n```\n\nComputes the answer.")
        );
        assert_eq!(response["result"]["range"]["start"]["line"], json!(1));
        assert_eq!(response["result"]["range"]["start"]["character"], json!(11));
    }

    #[test]
    fn returns_documented_hover_for_imported_function_reference() {
        let project = TempProject::new("lsp-hover-import");
        project.write_nocter_home();
        let app = project.write_source(
            "app.nct",
            "from ./config import answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
        );
        let config = project.write_source(
            "config.nct",
            "/// Returns the configured answer.\npub func answer(): i32 {\n    return 42\n}\n",
        );
        let app_uri = file_uri(&app);
        let config_uri = file_uri(&config);
        let server = LspServer {
            documents: HashMap::from([
                (
                    app_uri.clone(),
                    open_document(
                        app_uri.clone(),
                        Some(1),
                        "from ./config import answer\n\nfunc main(): i32 {\n    return answer()\n}\n"
                            .to_string(),
                    ),
                ),
                (
                    config_uri.clone(),
                    open_document(
                        config_uri,
                        Some(1),
                        "/// Returns the configured answer.\npub func answer(): i32 {\n    return 42\n}\n"
                            .to_string(),
                    ),
                ),
            ]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.hover_response(
            json!(7),
            Some(&json!({
                "textDocument": {
                    "uri": app_uri
                },
                "position": {
                    "line": 3,
                    "character": 12
                }
            })),
        );

        assert_eq!(
            response["result"]["contents"]["value"],
            json!("```nocter\nfunc answer(): i32\n```\n\nReturns the configured answer.")
        );
        assert_eq!(response["result"]["range"]["start"]["line"], json!(3));
        assert_eq!(response["result"]["range"]["start"]["character"], json!(11));
    }

    #[test]
    fn returns_definition_for_resolved_function_reference() {
        let uri = "file:///tmp/nocter-definition-reference.nct".to_string();
        let document = open_document(
            uri.clone(),
            Some(1),
            "func main(): i32 {\n    return answer()\n}\n\nfunc answer(): i32 {\n    return 42\n}\n"
                .to_string(),
        );
        let server = LspServer {
            documents: HashMap::from([(uri.clone(), document)]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.definition_response(
            json!(8),
            Some(&json!({
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": 1,
                    "character": 12
                }
            })),
        );

        assert_eq!(response["result"]["range"]["start"]["line"], json!(4));
        assert_eq!(response["result"]["range"]["start"]["character"], json!(5));
        assert_eq!(response["result"]["range"]["end"]["character"], json!(11));
    }

    #[test]
    fn returns_definition_for_local_reference() {
        let uri = "file:///tmp/nocter-definition-local.nct".to_string();
        let document = open_document(
            uri.clone(),
            Some(1),
            "func main(path: str): i32 {\n    let code = 0\n    return code\n}\n".to_string(),
        );
        let server = LspServer {
            documents: HashMap::from([(uri.clone(), document)]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.definition_response(
            json!(9),
            Some(&json!({
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": 2,
                    "character": 12
                }
            })),
        );

        assert_eq!(response["result"]["range"]["start"]["line"], json!(1));
        assert_eq!(response["result"]["range"]["start"]["character"], json!(8));
        assert_eq!(response["result"]["range"]["end"]["character"], json!(12));
    }

    #[test]
    fn returns_definition_for_imported_function_reference() {
        let project = TempProject::new("lsp-definition-import");
        project.write_nocter_home();
        let app = project.write_source(
            "app.nct",
            "from ./config import answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
        );
        let config =
            project.write_source("config.nct", "pub func answer(): i32 {\n    return 42\n}\n");
        let app_uri = file_uri(&app);
        let config_uri = file_uri(&config);
        let server = LspServer {
            documents: HashMap::from([
                (
                    app_uri.clone(),
                    open_document(
                        app_uri.clone(),
                        Some(1),
                        "from ./config import answer\n\nfunc main(): i32 {\n    return answer()\n}\n"
                            .to_string(),
                    ),
                ),
                (
                    config_uri.clone(),
                    open_document(
                        config_uri.clone(),
                        Some(1),
                        "pub func answer(): i32 {\n    return 42\n}\n".to_string(),
                    ),
                ),
            ]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.definition_response(
            json!(9),
            Some(&json!({
                "textDocument": {
                    "uri": app_uri
                },
                "position": {
                    "line": 3,
                    "character": 12
                }
            })),
        );

        assert_eq!(
            response["result"]["uri"],
            json!(file_uri(&config.canonicalize().unwrap()))
        );
        assert_eq!(response["result"]["range"]["start"]["line"], json!(0));
        assert_eq!(response["result"]["range"]["start"]["character"], json!(9));
        assert_eq!(response["result"]["range"]["end"]["character"], json!(15));
    }

    #[test]
    fn returns_document_symbols_for_top_level_declarations() {
        let uri = "file:///tmp/nocter-document-symbols.nct".to_string();
        let document = open_document(
            uri.clone(),
            Some(1),
            "struct Config {\n    path: str\n}\n\nenum Mode {\n    fast\n    slow\n}\n\nfunc main(): i32 {\n    return 0\n}\n"
                .to_string(),
        );
        let server = LspServer {
            documents: HashMap::from([(uri.clone(), document)]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.document_symbol_response(
            json!(10),
            Some(&json!({
                "textDocument": {
                    "uri": uri
                }
            })),
        );
        let symbols = response["result"]
            .as_array()
            .expect("expected document symbols");

        assert_eq!(symbols.len(), 3);
        assert_eq!(symbols[0]["name"], json!("Config"));
        assert_eq!(symbols[0]["kind"], json!(LSP_SYMBOL_KIND_STRUCT));
        assert_eq!(symbols[0]["children"][0]["name"], json!("path"));
        assert_eq!(
            symbols[0]["children"][0]["kind"],
            json!(LSP_SYMBOL_KIND_FIELD)
        );
        assert_eq!(symbols[1]["name"], json!("Mode"));
        assert_eq!(
            symbols[1]["children"][0]["kind"],
            json!(LSP_SYMBOL_KIND_ENUM_MEMBER)
        );
        assert_eq!(symbols[2]["name"], json!("main"));
        assert_eq!(symbols[2]["kind"], json!(LSP_SYMBOL_KIND_FUNCTION));
    }

    #[test]
    fn returns_completion_items_for_keywords_and_top_level_symbols() {
        let uri = "file:///tmp/nocter-completion.nct".to_string();
        let document = open_document(
            uri.clone(),
            Some(1),
            "struct Config {\n    path: str\n}\n\nfunc answer(): i32 {\n    return 42\n}\n"
                .to_string(),
        );
        let server = LspServer {
            documents: HashMap::from([(uri.clone(), document)]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.completion_response(
            json!(11),
            Some(&json!({
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": 5,
                    "character": 4
                }
            })),
        );
        let items = response["result"]["items"]
            .as_array()
            .expect("expected completion items");

        assert!(completion_item_with_label(items, "return").is_some());
        assert_eq!(
            completion_item_with_label(items, "answer").and_then(|item| item["kind"].as_u64()),
            Some(LSP_COMPLETION_ITEM_KIND_FUNCTION as u64)
        );
        assert_eq!(
            completion_item_with_label(items, "Config").and_then(|item| item["kind"].as_u64()),
            Some(LSP_COMPLETION_ITEM_KIND_STRUCT as u64)
        );
    }

    #[test]
    fn returns_completion_items_for_imported_symbols() {
        let project = TempProject::new("lsp-completion-import");
        project.write_nocter_home();
        let app = project.write_source(
            "app.nct",
            "from ./config import answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
        );
        let config =
            project.write_source("config.nct", "pub func answer(): i32 {\n    return 42\n}\n");
        let app_uri = file_uri(&app);
        let config_uri = file_uri(&config);
        let server = LspServer {
            documents: HashMap::from([
                (
                    app_uri.clone(),
                    open_document(
                        app_uri.clone(),
                        Some(1),
                        "from ./config import answer\n\nfunc main(): i32 {\n    return answer()\n}\n"
                            .to_string(),
                    ),
                ),
                (
                    config_uri.clone(),
                    open_document(
                        config_uri,
                        Some(1),
                        "pub func answer(): i32 {\n    return 42\n}\n".to_string(),
                    ),
                ),
            ]),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            shutdown_requested: false,
        };

        let response = server.completion_response(
            json!(12),
            Some(&json!({
                "textDocument": {
                    "uri": app_uri
                },
                "position": {
                    "line": 3,
                    "character": 4
                }
            })),
        );
        let items = response["result"]["items"]
            .as_array()
            .expect("expected completion items");

        assert_eq!(
            completion_item_with_label(items, "answer").and_then(|item| item["kind"].as_u64()),
            Some(LSP_COMPLETION_ITEM_KIND_FUNCTION as u64)
        );
    }

    #[test]
    fn initialize_stores_workspace_folders() {
        let mut server = LspServer::new();
        let mut output = Vec::new();

        server
            .handle_message(
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "rootUri": "file:///tmp/ignored-root",
                        "workspaceFolders": [
                            {
                                "uri": "file:///tmp/nocter-workspace-a",
                                "name": "workspace-a"
                            },
                            {
                                "uri": "file:///tmp/nocter-workspace-b",
                                "name": "workspace-b"
                            }
                        ]
                    }
                }),
                &mut output,
            )
            .unwrap();

        assert_eq!(
            server.workspace_roots,
            vec![
                WorkspaceRoot {
                    uri: "file:///tmp/nocter-workspace-a".to_string(),
                    path: Some(PathBuf::from("/tmp/nocter-workspace-a")),
                },
                WorkspaceRoot {
                    uri: "file:///tmp/nocter-workspace-b".to_string(),
                    path: Some(PathBuf::from("/tmp/nocter-workspace-b")),
                },
            ]
        );
    }

    #[test]
    fn initialize_falls_back_to_root_uri() {
        let mut server = LspServer::new();
        let mut output = Vec::new();

        server
            .handle_message(
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {
                        "rootUri": "file:///tmp/nocter-root"
                    }
                }),
                &mut output,
            )
            .unwrap();

        assert_eq!(
            server.workspace_roots,
            vec![WorkspaceRoot {
                uri: "file:///tmp/nocter-root".to_string(),
                path: Some(PathBuf::from("/tmp/nocter-root")),
            }]
        );
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

    #[test]
    fn publishes_diagnostics_for_open_imported_document_text() {
        let project = TempProject::new("lsp-open-import");
        let app = project.write_source(
            "app.nct",
            "from ./config import answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
        );
        let config =
            project.write_source("config.nct", "pub func answer(): i32 {\n    return 0\n}\n");
        let app_uri = file_uri(&app);
        let config_uri = file_uri(&config);
        let documents = HashMap::from([
            (
                app_uri.clone(),
                open_document(
                    app_uri.clone(),
                    Some(1),
                    "from ./config import answer\n\nfunc main(): i32 {\n    return answer()\n}\n"
                        .to_string(),
                ),
            ),
            (
                config_uri.clone(),
                open_document(
                    config_uri.clone(),
                    Some(1),
                    "pub func answer(: i32 {\n".to_string(),
                ),
            ),
        ]);

        let diagnostics = analyze_workspace(&app_uri, &documents);

        let config_diagnostics = diagnostics
            .iter()
            .find(|(uri, _)| uri == &config_uri)
            .map(|(_, diagnostics)| diagnostics)
            .expect("expected config diagnostics");
        assert!(
            config_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E0200")
        );
    }

    #[test]
    fn clears_diagnostics_for_uris_missing_from_next_publish() {
        let mut server = LspServer::new();
        let mut output = Vec::new();
        let uri = "file:///tmp/nocter-cleared.nct".to_string();
        server.published_diagnostic_uris.insert(uri.clone());

        server
            .publish_workspace_diagnostics("file:///tmp/missing-root.nct", &mut output)
            .unwrap();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains(&uri));
        assert!(text.contains("\"diagnostics\":[]"));
        assert!(server.published_diagnostic_uris.is_empty());
    }

    fn frame(message: &Value) -> Vec<u8> {
        let body = serde_json::to_vec(message).unwrap();
        let mut framed = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        framed.extend(body);
        framed
    }

    fn file_uri(path: &Path) -> String {
        format!("file://{}", path.to_string_lossy())
    }

    fn completion_item_with_label<'a>(items: &'a [Value], label: &str) -> Option<&'a Value> {
        items
            .iter()
            .find(|item| item.get("label").and_then(Value::as_str) == Some(label))
    }

    struct TempProject {
        root: PathBuf,
    }

    impl TempProject {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "nocter-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn write_source(&self, name: &str, text: &str) -> PathBuf {
            let path = self.root.join(name);
            std::fs::write(&path, text).unwrap();
            path
        }

        fn write_nocter_home(&self) -> PathBuf {
            let home = self.root.join(".nocter");
            std::fs::create_dir_all(home.join("std")).unwrap();
            std::fs::write(home.join("std/prelude.nct"), "pub type Int = i32\n").unwrap();
            home
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}
