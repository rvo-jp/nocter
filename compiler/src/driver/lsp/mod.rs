use crate::analysis::{FileAnalysis, analyze_compile_unit_with_entry};
use crate::ast::{ImplMember, Item, MethodDecl};
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::{ResolveOutput, Symbol, SymbolKind, TypeSymbolKind};
use crate::source::{ByteSpan, SourceMap};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod definition;
mod diagnostics;
mod documents;
mod hover;
mod protocol;
mod semantic;

use definition::{definition_for_document, definition_for_file_analysis};
use diagnostics::{LspDiagnostic, diagnostics_for_lsp, publish_diagnostics};
use documents::{
    OpenDocument, WorkspaceRoot, changed_document_from_params, document_uri_from_params,
    open_document_from_params, workspace_roots_from_initialize_params,
};
#[cfg(test)]
use documents::{file_uri_to_path, open_document};
use hover::{
    hover_for_document, hover_for_file_analysis, resolve_single_file_for_hover, source_fragment,
};
#[cfg(test)]
use protocol::LspPosition;
#[cfg(test)]
use protocol::byte_offset_to_lsp_position;
use protocol::{
    lsp_position_to_byte_offset, position_from_params, range_for_byte_span, read_message, response,
    write_message,
};
#[cfg(test)]
use semantic::SEMANTIC_DECLARATION_MODIFIER;
#[cfg(test)]
use semantic::SemanticTokenKind;
use semantic::{SEMANTIC_TOKEN_MODIFIERS, SEMANTIC_TOKEN_TYPES, semantic_tokens_for_document};

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
