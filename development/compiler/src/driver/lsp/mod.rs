use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

mod analysis;
mod completion;
mod definition;
mod diagnostics;
mod documents;
mod hover;
mod import_completion;
mod locations;
mod protocol;
mod recovery;
mod references;
mod semantic;
mod signature_help;
mod symbols;

use analysis::{
    LspWorkspaceAnalysis, diagnostics_for_workspace_with_source_root,
    workspace_analysis_for_uri_with_source_root,
};
#[cfg(test)]
use analysis::{diagnostics_for_workspace, workspace_analysis_for_uri};
#[cfg(test)]
use completion::{
    LSP_COMPLETION_ITEM_KIND_CONSTRUCTOR, LSP_COMPLETION_ITEM_KIND_ENUM_MEMBER,
    LSP_COMPLETION_ITEM_KIND_FIELD, LSP_COMPLETION_ITEM_KIND_FUNCTION,
    LSP_COMPLETION_ITEM_KIND_METHOD, LSP_COMPLETION_ITEM_KIND_MODULE,
    LSP_COMPLETION_ITEM_KIND_STRUCT,
};
use completion::{
    completion_items_for_document_at_offset, completion_items_for_file_analysis_at_offset,
    keyword_completion_items, literal_shape_completion_items_for_file_analysis_at_offset,
};
use definition::{definition_for_document, definition_for_file_analysis};
use diagnostics::publish_diagnostics;
use documents::{
    OpenDocument, WorkspaceRoot, changed_document_from_params, document_uri_from_params,
    open_document_from_params, workspace_roots_from_initialize_params,
};
#[cfg(test)]
use documents::{file_uri_to_path, open_document};
use hover::{hover_for_document, hover_for_file_analysis};
use import_completion::{module_completion_items, source_root_for_document};
#[cfg(test)]
use protocol::byte_offset_to_lsp_position;
use protocol::{
    LspPosition, lsp_position_to_byte_offset, position_from_params, read_message, response,
    write_message,
};
use recovery::workspace_analysis_with_recovered_document;
use references::{references_for_document, references_for_file_analysis};
#[cfg(test)]
use semantic::ClassifiedIdentifier;
#[cfg(test)]
use semantic::SEMANTIC_DECLARATION_MODIFIER;
#[cfg(test)]
use semantic::SEMANTIC_READONLY_MODIFIER;
#[cfg(test)]
use semantic::SemanticTokenKind;
#[cfg(test)]
use semantic::semantic_token_kind_index;
use semantic::{
    SEMANTIC_TOKEN_MODIFIERS, SEMANTIC_TOKEN_TYPES, semantic_tokens_for_document,
    semantic_tokens_for_file_analysis,
};
#[cfg(test)]
use semantic::{classified_identifiers, classified_identifiers_for_file_analysis};
use signature_help::signature_help_value;
use symbols::document_symbols_for_document;
#[cfg(test)]
use symbols::{
    LSP_SYMBOL_KIND_ENUM_MEMBER, LSP_SYMBOL_KIND_FIELD, LSP_SYMBOL_KIND_FUNCTION,
    LSP_SYMBOL_KIND_STRUCT,
};

use crate::analysis::FileAnalysis;

pub(super) fn run_lsp() -> ExitCode {
    let stdin = io::stdin();
    let stdout = io::stdout();
    match run_lsp_stream(stdin.lock(), stdout.lock()) {
        Ok(exit) => exit,
        Err(error) => {
            eprintln!("lsp error: {error}");
            ExitCode::from(3)
        }
    }
}

fn run_lsp_stream<R: BufRead, W: Write>(mut reader: R, mut writer: W) -> io::Result<ExitCode> {
    let mut server = LspServer::new();

    while let Some(message) = read_message(&mut reader)? {
        if let Some(exit) = server.handle_message(message, &mut writer)? {
            return Ok(exit);
        }
    }

    Ok(ExitCode::SUCCESS)
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

    fn handle_message<W: Write>(
        &mut self,
        message: Value,
        writer: &mut W,
    ) -> io::Result<Option<ExitCode>> {
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
    ) -> io::Result<Option<ExitCode>> {
        if self.shutdown_requested && method != "shutdown" {
            write_message(
                writer,
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32600,
                        "message": "server is shutting down"
                    }
                }),
            )?;
            return Ok(None);
        }

        match method {
            "initialize" => {
                self.workspace_roots = workspace_roots_from_initialize_params(params);
                write_message(writer, initialize_response(id))?;
                Ok(None)
            }
            "textDocument/semanticTokens/full" => {
                write_message(writer, self.semantic_tokens_response(id, params))?;
                Ok(None)
            }
            "textDocument/hover" => {
                write_message(writer, self.hover_response(id, params))?;
                Ok(None)
            }
            "textDocument/definition" => {
                write_message(writer, self.definition_response(id, params))?;
                Ok(None)
            }
            "textDocument/references" => {
                write_message(writer, self.references_response(id, params))?;
                Ok(None)
            }
            "textDocument/documentSymbol" => {
                write_message(writer, self.document_symbol_response(id, params))?;
                Ok(None)
            }
            "textDocument/completion" => {
                write_message(writer, self.completion_response(id, params))?;
                Ok(None)
            }
            "textDocument/signatureHelp" => {
                write_message(writer, self.signature_help_response(id, params))?;
                Ok(None)
            }
            "shutdown" => {
                self.shutdown_requested = true;
                write_message(writer, response(id, Value::Null))?;
                Ok(None)
            }
            _ => {
                write_message(
                    writer,
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32601,
                        "message": format!("method `{method}` is not supported by Nocter LSP v0.2.0")
                        }
                    }),
                )?;
                Ok(None)
            }
        }
    }

    fn handle_notification<W: Write>(
        &mut self,
        method: &str,
        params: Option<&Value>,
        writer: &mut W,
    ) -> io::Result<Option<ExitCode>> {
        if self.shutdown_requested && method != "exit" {
            return Ok(None);
        }

        match method {
            "initialized" => {}
            "exit" => {
                return Ok(Some(if self.shutdown_requested {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }));
            }
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
                        return Ok(None);
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

        Ok(None)
    }

    fn publish_workspace_diagnostics<W: Write>(
        &mut self,
        root_uri: &str,
        writer: &mut W,
    ) -> io::Result<()> {
        let source_root = self
            .documents
            .get(root_uri)
            .and_then(|document| source_root_for_document(document, &self.workspace_roots));
        let diagnostics_by_uri =
            diagnostics_for_workspace_with_source_root(root_uri, &self.documents, source_root);
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
            .and_then(|uri| {
                self.workspace_semantic_tokens_for_uri(&uri)
                    .or_else(|| self.documents.get(&uri).map(semantic_tokens_for_document))
            })
            .unwrap_or_default();
        response(id, json!({ "data": data }))
    }

    fn hover_response(&self, id: Value, params: Option<&Value>) -> Value {
        let hover = document_uri_from_params(params).and_then(|uri| {
            self.workspace_hover_for_uri(&uri, params)
                .or_else(|| self.workspace_hover_for_recovered_uri(&uri, params))
                .or_else(|| {
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

    fn references_response(&self, id: Value, params: Option<&Value>) -> Value {
        let references = document_uri_from_params(params)
            .and_then(|uri| {
                self.workspace_references_for_uri(&uri, params).or_else(|| {
                    self.documents
                        .get(&uri)
                        .map(|document| references_for_document(document, params))
                })
            })
            .unwrap_or_default();
        response(id, Value::Array(references))
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
                let position = position_from_params(params)?;
                let document = self.documents.get(&uri)?;
                let offset =
                    lsp_position_to_byte_offset(&document.text, position.line, position.character);
                module_completion_items(document, &self.workspace_roots, offset)
                    .or_else(|| {
                        self.workspace_literal_completion_for_recovered_uri(&uri, &position)
                    })
                    .or_else(|| self.workspace_completion_for_recovered_uri(&uri, &position))
                    .or_else(|| self.workspace_completion_for_uri(&uri, &position))
                    .or_else(|| completion_items_for_document_at_offset(document, offset))
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

    fn signature_help_response(&self, id: Value, params: Option<&Value>) -> Value {
        let signature = document_uri_from_params(params).and_then(|uri| {
            let position = position_from_params(params)?;
            self.with_workspace_file_for_uri(&uri, |document, workspace, file| {
                let offset =
                    lsp_position_to_byte_offset(&document.text, position.line, position.character);
                crate::analysis::signature_help::signature_help_for_file_analysis(
                    &workspace.sources,
                    &workspace.analysis,
                    file,
                    offset,
                )
            })
            .or_else(|| self.workspace_signature_help_for_recovered_uri(&uri, &position))
        });
        response(
            id,
            signature.map(signature_help_value).unwrap_or(Value::Null),
        )
    }

    fn workspace_hover_for_uri(&self, uri: &str, params: Option<&Value>) -> Option<Value> {
        let position = position_from_params(params)?;
        self.with_workspace_file_for_uri(uri, |document, workspace, file| {
            let root_offset =
                lsp_position_to_byte_offset(&document.text, position.line, position.character);
            hover_for_file_analysis(&workspace.sources, &workspace.analysis, file, root_offset)
        })
    }

    fn workspace_hover_for_recovered_uri(
        &self,
        uri: &str,
        params: Option<&Value>,
    ) -> Option<Value> {
        let position = position_from_params(params)?;
        let document = self.documents.get(uri)?;
        let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
        let source_root = source_root_for_document(document, &self.workspace_roots);
        for recovered in [
            crate::analysis::interpolation_recovery_text(&document.text, offset),
            crate::analysis::literal_recovery_text(&document.text, offset),
            crate::analysis::region_recovery_text(&document.text, offset),
            crate::analysis::collection_for_recovery_text(&document.text, offset),
        ]
        .into_iter()
        .flatten()
        {
            let Some(workspace) = workspace_analysis_with_recovered_document(
                uri,
                &self.documents,
                recovered,
                source_root,
            ) else {
                continue;
            };
            let Some(file) = workspace.root_file() else {
                continue;
            };
            if let Some(hover) =
                hover_for_file_analysis(&workspace.sources, &workspace.analysis, file, offset)
            {
                return Some(hover);
            }
        }
        None
    }

    fn workspace_semantic_tokens_for_uri(&self, uri: &str) -> Option<Vec<usize>> {
        self.with_workspace_file_for_uri(uri, |document, _workspace, file| {
            Some(semantic_tokens_for_file_analysis(document, file))
        })
    }

    fn workspace_definition_for_uri(&self, uri: &str, params: Option<&Value>) -> Option<Value> {
        let position = position_from_params(params)?;
        self.with_workspace_file_for_uri(uri, |document, workspace, file| {
            let root_offset =
                lsp_position_to_byte_offset(&document.text, position.line, position.character);
            definition_for_file_analysis(
                &workspace.sources,
                &workspace.analysis,
                file,
                document,
                &self.documents,
                root_offset,
            )
        })
    }

    fn workspace_references_for_uri(
        &self,
        uri: &str,
        params: Option<&Value>,
    ) -> Option<Vec<Value>> {
        let position = position_from_params(params)?;
        self.with_workspace_file_for_uri(uri, |document, workspace, file| {
            let root_offset =
                lsp_position_to_byte_offset(&document.text, position.line, position.character);
            Some(references_for_file_analysis(
                &workspace.sources,
                &workspace.analysis,
                file,
                &self.documents,
                params,
                root_offset,
            ))
        })
    }

    fn workspace_completion_for_uri(
        &self,
        uri: &str,
        position: &LspPosition,
    ) -> Option<Vec<Value>> {
        self.with_workspace_file_for_uri(uri, |document, workspace, file| {
            let root_offset =
                lsp_position_to_byte_offset(&document.text, position.line, position.character);
            Some(completion_items_for_file_analysis_at_offset(
                &workspace.sources,
                &workspace.analysis,
                file,
                root_offset,
            ))
        })
    }

    fn workspace_completion_for_recovered_uri(
        &self,
        uri: &str,
        position: &LspPosition,
    ) -> Option<Vec<Value>> {
        let document = self.documents.get(uri)?;
        let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
        let (recovered, recovered_offset) =
            crate::analysis::completion_recovery_overlay(&document.text, offset)?;
        let source_root = source_root_for_document(document, &self.workspace_roots);
        let workspace = workspace_analysis_with_recovered_document(
            uri,
            &self.documents,
            recovered,
            source_root,
        )?;
        let file = workspace.root_file()?;
        Some(completion_items_for_file_analysis_at_offset(
            &workspace.sources,
            &workspace.analysis,
            file,
            recovered_offset,
        ))
    }

    fn workspace_literal_completion_for_recovered_uri(
        &self,
        uri: &str,
        position: &LspPosition,
    ) -> Option<Vec<Value>> {
        let document = self.documents.get(uri)?;
        let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
        let (recovered, recovered_offset) =
            crate::analysis::literal_recovery_overlay(&document.text, offset)?;
        let source_root = source_root_for_document(document, &self.workspace_roots);
        let workspace = workspace_analysis_with_recovered_document(
            uri,
            &self.documents,
            recovered,
            source_root,
        )?;
        let file = workspace.root_file()?;
        literal_shape_completion_items_for_file_analysis_at_offset(
            &workspace.sources,
            &workspace.analysis,
            file,
            recovered_offset,
        )
        .or_else(|| {
            crate::analysis::literals::literal_editor_info_at_offset(
                &workspace.analysis,
                file,
                recovered_offset,
                crate::analysis::literals::LiteralCursorRegion::Arguments,
            )?;
            Some(completion_items_for_file_analysis_at_offset(
                &workspace.sources,
                &workspace.analysis,
                file,
                recovered_offset,
            ))
        })
    }

    fn workspace_signature_help_for_recovered_uri(
        &self,
        uri: &str,
        position: &LspPosition,
    ) -> Option<crate::analysis::signature_help::SignatureHelpInfo> {
        let document = self.documents.get(uri)?;
        let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
        let source_root = source_root_for_document(document, &self.workspace_roots);
        let recoveries = crate::analysis::literal_recovery_text(&document.text, offset)
            .into_iter()
            .chain(crate::analysis::interpolation_signature_recovery_texts(
                &document.text,
                offset,
            ))
            .chain(crate::analysis::signature_recovery_texts(
                &document.text,
                offset,
            ));
        for recovered in recoveries {
            let Some(workspace) = workspace_analysis_with_recovered_document(
                uri,
                &self.documents,
                recovered,
                source_root,
            ) else {
                continue;
            };
            let Some(file) = workspace.root_file() else {
                continue;
            };
            if let Some(signature) =
                crate::analysis::signature_help::signature_help_for_file_analysis(
                    &workspace.sources,
                    &workspace.analysis,
                    file,
                    offset,
                )
            {
                return Some(signature);
            }
        }
        None
    }

    fn with_workspace_file_for_uri<T>(
        &self,
        uri: &str,
        f: impl FnOnce(&OpenDocument, &LspWorkspaceAnalysis, &FileAnalysis) -> Option<T>,
    ) -> Option<T> {
        let document = self.documents.get(uri)?;
        let source_root = source_root_for_document(document, &self.workspace_roots);
        let workspace =
            workspace_analysis_for_uri_with_source_root(uri, &self.documents, source_root)?;
        let file = workspace.root_file()?;

        f(document, &workspace, file)
    }
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
                "referencesProvider": true,
                "documentSymbolProvider": true,
                "completionProvider": {
                    "resolveProvider": false,
                    "triggerCharacters": [".", ":"]
                },
                "signatureHelpProvider": {
                    "triggerCharacters": ["(", ","],
                    "retriggerCharacters": [","]
                }
            },
            "serverInfo": {
                "name": "nocter",
                "version": crate::driver::VERSION
            }
        }),
    )
}

#[cfg(test)]
mod tests;
