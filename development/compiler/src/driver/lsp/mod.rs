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
mod package_navigation;
mod protocol;
mod recovery;
mod references;
mod request_validation;
mod semantic;
mod signature_help;
mod snapshot;
mod symbols;

use analysis::LspWorkspaceAnalysis;
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
    OpenDocument, WorkspaceRoot, changed_document_from_params, changed_file_paths_from_params,
    document_uri_from_params, open_document_from_params, workspace_roots_from_initialize_params,
};
#[cfg(test)]
use documents::{file_uri_to_path, open_document};
use hover::{hover_for_document, hover_for_file_analysis};
use import_completion::module_completion_items;
use package_navigation::package_entry_definition;
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
use snapshot::{LspSnapshot, SnapshotChange, SnapshotStore};
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
    snapshots: SnapshotStore,
    shutdown_requested: bool,
}

impl LspServer {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
            published_diagnostic_uris: HashSet::new(),
            workspace_roots: Vec::new(),
            snapshots: SnapshotStore::default(),
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

        if request_validation::supported_text_document_params_are_valid(method, params)
            == Some(false)
        {
            write_message(
                writer,
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32602,
                        "message": format!("invalid params for `{method}`")
                    }
                }),
            )?;
            return Ok(None);
        }

        match method {
            "initialize" => {
                self.workspace_roots = workspace_roots_from_initialize_params(params);
                self.snapshots.rebuild(
                    &self.documents,
                    &self.workspace_roots,
                    SnapshotChange::Full,
                );
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
                        "message": format!(
                            "method `{method}` is not supported by Nocter LSP v{}",
                            crate::driver::VERSION
                        )
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
                    let changed_path = document.absolute_path.clone();
                    self.documents.insert(uri.clone(), document);
                    self.snapshots.rebuild(
                        &self.documents,
                        &self.workspace_roots,
                        SnapshotChange::path(changed_path.as_deref()),
                    );
                    self.publish_snapshot_diagnostics(writer)?;
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
                    let changed_path = document.absolute_path.clone();
                    self.documents.insert(uri.clone(), document);
                    self.snapshots.rebuild(
                        &self.documents,
                        &self.workspace_roots,
                        SnapshotChange::path(changed_path.as_deref()),
                    );
                    self.publish_snapshot_diagnostics(writer)?;
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = document_uri_from_params(params) {
                    let changed_path = self
                        .documents
                        .remove(&uri)
                        .and_then(|document| document.absolute_path);
                    self.snapshots.rebuild(
                        &self.documents,
                        &self.workspace_roots,
                        SnapshotChange::path(changed_path.as_deref()),
                    );
                    self.published_diagnostic_uris.remove(&uri);
                    write_message(writer, publish_diagnostics(&uri, None, Vec::new()))?;
                    self.publish_snapshot_diagnostics(writer)?;
                }
            }
            "workspace/didChangeWatchedFiles" => {
                let paths = changed_file_paths_from_params(params);
                if !paths.is_empty() {
                    self.snapshots.rebuild(
                        &self.documents,
                        &self.workspace_roots,
                        SnapshotChange::paths(paths),
                    );
                    self.publish_snapshot_diagnostics(writer)?;
                }
            }
            _ => {}
        }

        Ok(None)
    }

    fn publish_snapshot_diagnostics<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        let snapshot = self.snapshot();
        let current_uris = snapshot
            .document_uris()
            .into_iter()
            .map(str::to_string)
            .collect::<HashSet<_>>();

        for uri in self
            .published_diagnostic_uris
            .difference(&current_uris)
            .cloned()
            .collect::<Vec<_>>()
        {
            write_message(writer, publish_diagnostics(&uri, None, Vec::new()))?;
        }

        for uri in snapshot.document_uris() {
            write_message(
                writer,
                publish_diagnostics(
                    uri,
                    snapshot.document(uri).and_then(|document| document.version),
                    snapshot.diagnostics_for_uri(uri).to_vec(),
                ),
            )?;
        }

        self.published_diagnostic_uris = current_uris;
        Ok(())
    }

    fn snapshot(&self) -> std::sync::Arc<LspSnapshot> {
        self.snapshots
            .current(&self.documents, &self.workspace_roots)
    }

    fn semantic_tokens_response(&self, id: Value, params: Option<&Value>) -> Value {
        let snapshot = self.snapshot();
        let data = document_uri_from_params(params)
            .and_then(|uri| {
                self.workspace_semantic_tokens_for_uri(&snapshot, &uri)
                    .or_else(|| snapshot.document(&uri).map(semantic_tokens_for_document))
            })
            .unwrap_or_default();
        response(
            id,
            json!({
                "data": data,
                "resultId": snapshot.generation().to_string()
            }),
        )
    }

    fn hover_response(&self, id: Value, params: Option<&Value>) -> Value {
        let snapshot = self.snapshot();
        let hover = document_uri_from_params(params).and_then(|uri| {
            self.workspace_hover_for_uri(&snapshot, &uri, params)
                .or_else(|| self.workspace_hover_for_recovered_uri(&snapshot, &uri, params))
                .or_else(|| {
                    snapshot
                        .documents()
                        .get(&uri)
                        .and_then(|document| hover_for_document(document, params))
                })
        });
        response(id, hover.unwrap_or(Value::Null))
    }

    fn definition_response(&self, id: Value, params: Option<&Value>) -> Value {
        let snapshot = self.snapshot();
        let definition = document_uri_from_params(params).and_then(|uri| {
            snapshot
                .documents()
                .get(&uri)
                .and_then(|document| {
                    package_entry_definition(document, snapshot.package_root(&uri), params)
                })
                .or_else(|| self.workspace_definition_for_uri(&snapshot, &uri, params))
                .or_else(|| {
                    snapshot
                        .documents()
                        .get(&uri)
                        .and_then(|document| definition_for_document(document, params))
                })
        });
        response(id, definition.unwrap_or(Value::Null))
    }

    fn references_response(&self, id: Value, params: Option<&Value>) -> Value {
        let snapshot = self.snapshot();
        let references = document_uri_from_params(params)
            .and_then(|uri| {
                self.workspace_references_for_uri(&snapshot, &uri, params)
                    .or_else(|| {
                        snapshot
                            .documents()
                            .get(&uri)
                            .map(|document| references_for_document(document, params))
                    })
            })
            .unwrap_or_default();
        response(id, Value::Array(references))
    }

    fn document_symbol_response(&self, id: Value, params: Option<&Value>) -> Value {
        let snapshot = self.snapshot();
        let symbols = document_uri_from_params(params)
            .and_then(|uri| snapshot.document(&uri))
            .and_then(document_symbols_for_document)
            .unwrap_or_default();
        response(id, Value::Array(symbols))
    }

    fn completion_response(&self, id: Value, params: Option<&Value>) -> Value {
        let snapshot = self.snapshot();
        let items = document_uri_from_params(params)
            .and_then(|uri| {
                let position = position_from_params(params)?;
                let document = snapshot.document(&uri)?;
                let offset =
                    lsp_position_to_byte_offset(&document.text, position.line, position.character);
                module_completion_items(document, snapshot.package_graph(&uri), offset)
                    .or_else(|| {
                        self.workspace_literal_completion_for_recovered_uri(
                            &snapshot, &uri, &position,
                        )
                    })
                    .or_else(|| {
                        self.workspace_completion_for_recovered_uri(&snapshot, &uri, &position)
                    })
                    .or_else(|| self.workspace_completion_for_uri(&snapshot, &uri, &position))
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
        let snapshot = self.snapshot();
        let signature = document_uri_from_params(params).and_then(|uri| {
            let position = position_from_params(params)?;
            self.with_workspace_file_for_uri(&snapshot, &uri, |document, workspace, file| {
                let offset =
                    lsp_position_to_byte_offset(&document.text, position.line, position.character);
                crate::analysis::signature_help::signature_help_for_file_analysis(
                    &workspace.sources,
                    workspace.semantic()?,
                    file,
                    offset,
                )
            })
            .or_else(|| self.workspace_signature_help_for_recovered_uri(&snapshot, &uri, &position))
        });
        response(
            id,
            signature.map(signature_help_value).unwrap_or(Value::Null),
        )
    }

    fn workspace_hover_for_uri(
        &self,
        snapshot: &LspSnapshot,
        uri: &str,
        params: Option<&Value>,
    ) -> Option<Value> {
        let position = position_from_params(params)?;
        self.with_workspace_file_for_uri(snapshot, uri, |document, workspace, file| {
            let root_offset =
                lsp_position_to_byte_offset(&document.text, position.line, position.character);
            hover_for_file_analysis(&workspace.sources, workspace.semantic()?, file, root_offset)
        })
    }

    fn workspace_hover_for_recovered_uri(
        &self,
        snapshot: &LspSnapshot,
        uri: &str,
        params: Option<&Value>,
    ) -> Option<Value> {
        let position = position_from_params(params)?;
        let document = snapshot.document(uri)?;
        let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
        for recovered in [
            crate::analysis::interpolation_recovery_text(&document.text, offset),
            crate::analysis::literal_recovery_text(&document.text, offset),
            crate::analysis::region_recovery_text(&document.text, offset),
            crate::analysis::collection_for_recovery_text(&document.text, offset),
            crate::analysis::block_recovery_text(&document.text, offset),
        ]
        .into_iter()
        .flatten()
        {
            let Some(workspace) = workspace_analysis_with_recovered_document(
                uri,
                snapshot.documents(),
                recovered,
                snapshot.package_graph(uri),
            ) else {
                continue;
            };
            let Some(file) = workspace.root_file() else {
                continue;
            };
            if let Some(hover) =
                hover_for_file_analysis(&workspace.sources, workspace.semantic()?, file, offset)
            {
                return Some(hover);
            }
        }
        None
    }

    fn workspace_semantic_tokens_for_uri(
        &self,
        snapshot: &LspSnapshot,
        uri: &str,
    ) -> Option<Vec<usize>> {
        self.with_workspace_file_for_uri(snapshot, uri, |document, _workspace, file| {
            Some(semantic_tokens_for_file_analysis(document, file))
        })
    }

    fn workspace_definition_for_uri(
        &self,
        snapshot: &LspSnapshot,
        uri: &str,
        params: Option<&Value>,
    ) -> Option<Value> {
        let position = position_from_params(params)?;
        self.with_workspace_file_for_uri(snapshot, uri, |document, workspace, file| {
            let root_offset =
                lsp_position_to_byte_offset(&document.text, position.line, position.character);
            definition_for_file_analysis(
                &workspace.sources,
                workspace.semantic()?,
                file,
                document,
                snapshot.documents(),
                root_offset,
            )
        })
    }

    fn workspace_references_for_uri(
        &self,
        snapshot: &LspSnapshot,
        uri: &str,
        params: Option<&Value>,
    ) -> Option<Vec<Value>> {
        let position = position_from_params(params)?;
        self.with_workspace_file_for_uri(snapshot, uri, |document, workspace, file| {
            let root_offset =
                lsp_position_to_byte_offset(&document.text, position.line, position.character);
            Some(references_for_file_analysis(
                &workspace.sources,
                workspace.semantic()?,
                file,
                snapshot.documents(),
                params,
                root_offset,
            ))
        })
    }

    fn workspace_completion_for_uri(
        &self,
        snapshot: &LspSnapshot,
        uri: &str,
        position: &LspPosition,
    ) -> Option<Vec<Value>> {
        self.with_workspace_file_for_uri(snapshot, uri, |document, workspace, file| {
            let root_offset =
                lsp_position_to_byte_offset(&document.text, position.line, position.character);
            Some(completion_items_for_file_analysis_at_offset(
                &workspace.sources,
                workspace.semantic()?,
                file,
                root_offset,
            ))
        })
    }

    fn workspace_completion_for_recovered_uri(
        &self,
        snapshot: &LspSnapshot,
        uri: &str,
        position: &LspPosition,
    ) -> Option<Vec<Value>> {
        let document = snapshot.document(uri)?;
        let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
        let (recovered, recovered_offset) =
            crate::analysis::completion_recovery_overlay(&document.text, offset)?;
        let workspace = workspace_analysis_with_recovered_document(
            uri,
            snapshot.documents(),
            recovered,
            snapshot.package_graph(uri),
        )?;
        let file = workspace.root_file()?;
        Some(completion_items_for_file_analysis_at_offset(
            &workspace.sources,
            workspace.semantic()?,
            file,
            recovered_offset,
        ))
    }

    fn workspace_literal_completion_for_recovered_uri(
        &self,
        snapshot: &LspSnapshot,
        uri: &str,
        position: &LspPosition,
    ) -> Option<Vec<Value>> {
        let document = snapshot.document(uri)?;
        let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
        let (recovered, recovered_offset) =
            crate::analysis::literal_recovery_overlay(&document.text, offset)?;
        let workspace = workspace_analysis_with_recovered_document(
            uri,
            snapshot.documents(),
            recovered,
            snapshot.package_graph(uri),
        )?;
        let file = workspace.root_file()?;
        literal_shape_completion_items_for_file_analysis_at_offset(
            &workspace.sources,
            workspace.semantic()?,
            file,
            recovered_offset,
        )
        .or_else(|| {
            let has_semantic_literal = crate::analysis::literals::literal_editor_info_at_offset(
                workspace.semantic()?,
                file,
                recovered_offset,
                crate::analysis::literals::LiteralCursorRegion::Arguments,
            )
            .is_some();
            if !has_semantic_literal
                && !crate::analysis::literals::literal_arguments_contain_offset(
                    file,
                    recovered_offset,
                )
            {
                return None;
            }
            Some(completion_items_for_file_analysis_at_offset(
                &workspace.sources,
                workspace.semantic()?,
                file,
                recovered_offset,
            ))
        })
    }

    fn workspace_signature_help_for_recovered_uri(
        &self,
        snapshot: &LspSnapshot,
        uri: &str,
        position: &LspPosition,
    ) -> Option<crate::analysis::signature_help::SignatureHelpInfo> {
        let document = snapshot.document(uri)?;
        let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
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
                snapshot.documents(),
                recovered,
                snapshot.package_graph(uri),
            ) else {
                continue;
            };
            let Some(file) = workspace.root_file() else {
                continue;
            };
            if let Some(signature) =
                crate::analysis::signature_help::signature_help_for_file_analysis(
                    &workspace.sources,
                    workspace.semantic()?,
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
        snapshot: &LspSnapshot,
        uri: &str,
        f: impl FnOnce(&OpenDocument, &LspWorkspaceAnalysis, &FileAnalysis) -> Option<T>,
    ) -> Option<T> {
        let document = snapshot.document(uri)?;
        let workspace = snapshot.analysis(uri)?;
        let file = workspace.root_file()?;

        f(document, workspace, file)
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
