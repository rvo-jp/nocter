use std::collections::BTreeSet;

use nocter_json::Value;
use nocter_lsp::{
    DidChangeParams, DidChangeWatchedFilesParams, DidCloseParams, DidOpenParams, DidSaveParams,
    IncomingMessage, InitializeParams, LifecycleTransitionError, OutboundRequests, ProtocolEvent,
    ProtocolSession, RequestId, ResponseErrorCode, ResponseResult, initialize_result,
    render_error_response, render_success_response, watched_files_registration,
};

use crate::{
    DiagnosticPublisher, DocumentWorkspace, DocumentWorkspaceChange, LanguageServerEnvironment,
    WorkspaceAnalyses, WorkspaceConfiguration, WorkspaceSourceRevision,
};

#[cfg(test)]
use crate::{WorkspaceAnalysisBatch, WorkspaceAnalysisGeneration};

mod issues;
#[cfg(test)]
mod reference_application_tests;
#[cfg(test)]
mod semantic_matrix_tests;
mod semantic_requests;

use crate::workspace::resolve_workspace_configuration;
use issues::InitializeFailure;
pub use issues::{ClientResponseError, ServerIssue};

/// One fully validated protocol and document-state transition.
#[derive(Debug, Default)]
pub struct ServerStep {
    response: Option<String>,
    outbound: Box<[String]>,
    #[cfg(test)]
    analysis: Option<WorkspaceAnalysisBatch>,
    issues: Box<[ServerIssue]>,
    exit_code: Option<i32>,
}

impl ServerStep {
    #[must_use]
    pub fn response(&self) -> Option<&str> {
        self.response.as_deref()
    }

    #[must_use]
    pub const fn outbound_messages(&self) -> &[String] {
        &self.outbound
    }

    #[cfg(test)]
    #[must_use]
    pub fn generation(&self) -> Option<&WorkspaceAnalysisGeneration> {
        self.analysis.as_ref().map(WorkspaceAnalysisBatch::primary)
    }

    #[cfg(test)]
    #[must_use]
    pub fn analysis(&self) -> Option<&WorkspaceAnalysisGeneration> {
        self.generation()
    }

    #[cfg(test)]
    pub fn analyses(&self) -> impl Iterator<Item = &WorkspaceAnalysisGeneration> {
        self.analysis
            .iter()
            .flat_map(WorkspaceAnalysisBatch::publication_order)
    }

    #[must_use]
    pub const fn issue(&self) -> Option<&ServerIssue> {
        self.issues.first()
    }

    #[must_use]
    pub const fn issues(&self) -> &[ServerIssue] {
        &self.issues
    }

    #[must_use]
    pub const fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }
}

/// Sequential protocol service. Analysis is triggered only from its accepted generations.
#[derive(Debug)]
pub struct LanguageServer {
    protocol: ProtocolSession,
    documents: DocumentWorkspace,
    server_version: Box<str>,
    environment: LanguageServerEnvironment,
    initialization: Option<InitializeParams>,
    workspace: Option<WorkspaceConfiguration>,
    analyses: Option<WorkspaceAnalyses>,
    diagnostics: DiagnosticPublisher,
    outbound_requests: OutboundRequests,
    watcher: WatcherState,
}

impl LanguageServer {
    #[must_use]
    pub fn new(
        server_version: impl Into<Box<str>>,
        environment: LanguageServerEnvironment,
    ) -> Self {
        Self {
            protocol: ProtocolSession::new(),
            documents: DocumentWorkspace::new(),
            server_version: server_version.into(),
            environment,
            initialization: None,
            workspace: None,
            analyses: None,
            diagnostics: DiagnosticPublisher::new(),
            outbound_requests: OutboundRequests::new(),
            watcher: WatcherState::Unavailable,
        }
    }

    #[must_use]
    pub const fn initialization(&self) -> Option<&InitializeParams> {
        self.initialization.as_ref()
    }

    #[must_use]
    pub const fn workspace(&self) -> Option<&WorkspaceConfiguration> {
        self.workspace.as_ref()
    }

    /// Processes one unframed JSON body into at most one response and one accepted generation.
    pub fn receive(&mut self, body: &str) -> ServerStep {
        let reception = self.protocol.receive(body);
        let (response, event) = reception.into_parts();
        if response.is_some() {
            return ServerStep {
                response,
                ..ServerStep::default()
            };
        }
        let Some(event) = event else {
            return ServerStep::default();
        };
        match event {
            ProtocolEvent::Initialize { id, params } => self.initialize(&id, params),
            ProtocolEvent::Initialized => self.initialized(),
            ProtocolEvent::Message(message) => self.message(message),
            ProtocolEvent::Shutdown { id } => ServerStep {
                response: Some(render_success_response(&id, &Value::Null)),
                ..ServerStep::default()
            },
            ProtocolEvent::Exit { clean } => ServerStep {
                exit_code: Some(i32::from(!clean)),
                ..ServerStep::default()
            },
        }
    }

    fn initialize(&mut self, id: &RequestId, params: Option<Value>) -> ServerStep {
        match InitializeParams::decode(params)
            .map_err(InitializeFailure::Parameters)
            .and_then(|params| {
                resolve_workspace_configuration(&self.environment, &params)
                    .map(|workspace| (params, workspace))
                    .map_err(InitializeFailure::Workspace)
            }) {
            Ok((params, workspace)) => {
                if let Err(error) = self.protocol.complete_initialize(true) {
                    return internal_transition_error(id, error);
                }
                self.initialization = Some(params);
                self.analyses = Some(WorkspaceAnalyses::new(workspace.clone()));
                self.workspace = Some(workspace);
                ServerStep {
                    response: Some(render_success_response(
                        id,
                        &initialize_result(&self.server_version),
                    )),
                    ..ServerStep::default()
                }
            }
            Err(error) => {
                if let Err(transition) = self.protocol.complete_initialize(false) {
                    return internal_transition_error(id, transition);
                }
                let detail = Value::String(error.to_string().into_boxed_str());
                ServerStep {
                    response: Some(render_error_response(
                        Some(id),
                        ResponseErrorCode::InvalidParams,
                        Some(&detail),
                    )),
                    issues: error
                        .into_server_issue()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
        }
    }

    fn initialized(&mut self) -> ServerStep {
        if !self
            .initialization
            .as_ref()
            .is_some_and(InitializeParams::supports_dynamic_watched_files)
        {
            return ServerStep::default();
        }
        match self
            .outbound_requests
            .begin("client/registerCapability", &watched_files_registration())
        {
            Ok(request) => {
                self.watcher = WatcherState::Registering;
                ServerStep {
                    outbound: vec![request.body().to_owned()].into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
            Err(error) => ServerStep {
                issues: vec![ServerIssue::Outbound(error)].into_boxed_slice(),
                ..ServerStep::default()
            },
        }
    }

    fn message(&mut self, message: IncomingMessage) -> ServerStep {
        match message {
            IncomingMessage::Request { id, method, params }
                if method.as_ref() == "textDocument/hover" =>
            {
                self.hover(&id, params)
            }
            IncomingMessage::Request { id, method, params }
                if method.as_ref() == "textDocument/completion" =>
            {
                self.completion(&id, params)
            }
            IncomingMessage::Request { id, method, params }
                if method.as_ref() == "textDocument/semanticTokens/full" =>
            {
                self.semantic_tokens(&id, params)
            }
            IncomingMessage::Request { id, method, params }
                if method.as_ref() == "textDocument/inlayHint" =>
            {
                self.inlay_hints(&id, params)
            }
            IncomingMessage::Request { id, method, params }
                if method.as_ref() == "textDocument/codeAction" =>
            {
                self.code_actions(&id, params)
            }
            IncomingMessage::Request { id, method, params }
                if method.as_ref() == "textDocument/definition" =>
            {
                self.definition(&id, params)
            }
            IncomingMessage::Request { id, method, params }
                if method.as_ref() == "textDocument/implementation" =>
            {
                self.implementation(&id, params)
            }
            IncomingMessage::Request { id, method, params }
                if method.as_ref() == "textDocument/references" =>
            {
                self.references(&id, params)
            }
            IncomingMessage::Request { id, method, params }
                if method.as_ref() == "textDocument/rename" =>
            {
                self.rename(&id, params)
            }
            IncomingMessage::Request { id, method, params }
                if method.as_ref() == "textDocument/signatureHelp" =>
            {
                self.signature_help(&id, params)
            }
            IncomingMessage::Request { id, .. } => ServerStep {
                response: Some(render_error_response(
                    Some(&id),
                    ResponseErrorCode::MethodNotFound,
                    None,
                )),
                ..ServerStep::default()
            },
            IncomingMessage::Notification { method, params } => self.notification(&method, params),
            IncomingMessage::Response { id, result } => self.client_response(id, result),
        }
    }

    fn client_response(&mut self, id: RequestId, result: ResponseResult) -> ServerStep {
        let completed = match self.outbound_requests.complete(id, result) {
            Ok(completed) => completed,
            Err(error) => {
                return ServerStep {
                    issues: vec![ServerIssue::Outbound(error)].into_boxed_slice(),
                    ..ServerStep::default()
                };
            }
        };
        if completed.method() != "client/registerCapability" {
            return ServerStep::default();
        }
        match completed.result() {
            ResponseResult::Success(Value::Null) => {
                self.watcher = WatcherState::Registered;
                ServerStep::default()
            }
            ResponseResult::Success(_) => {
                self.watcher = WatcherState::Failed;
                ServerStep {
                    issues: vec![ServerIssue::ClientResponse(
                        ClientResponseError::InvalidRegistrationResult,
                    )]
                    .into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
            ResponseResult::Error(error) => {
                self.watcher = WatcherState::Failed;
                ServerStep {
                    issues: vec![ServerIssue::ClientResponse(
                        ClientResponseError::RegistrationRejected(error.clone()),
                    )]
                    .into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
        }
    }

    fn notification(&mut self, method: &str, params: Option<Value>) -> ServerStep {
        if method == "workspace/didChangeWatchedFiles" {
            return self.watched_files(params);
        }
        let generation: Result<Option<WorkspaceSourceRevision>, ServerIssue> = match method {
            "textDocument/didOpen" => DidOpenParams::decode(params)
                .map_err(ServerIssue::Parameters)
                .and_then(|params| self.documents.open(&params).map_err(ServerIssue::Documents))
                .map(Some),
            "textDocument/didChange" => DidChangeParams::decode(params)
                .map_err(ServerIssue::Parameters)
                .and_then(|params| {
                    self.documents
                        .change(&params)
                        .map_err(ServerIssue::Documents)
                        .map(|change| match change {
                            DocumentWorkspaceChange::Accepted(generation) => Some(generation),
                            DocumentWorkspaceChange::IgnoredStale { .. } => None,
                        })
                }),
            "textDocument/didSave" => DidSaveParams::decode(params)
                .map_err(ServerIssue::Parameters)
                .and_then(|params| self.documents.save(&params).map_err(ServerIssue::Documents))
                .map(Some),
            "textDocument/didClose" => DidCloseParams::decode(params)
                .map_err(ServerIssue::Parameters)
                .and_then(|params| {
                    self.documents
                        .close(&params)
                        .map_err(ServerIssue::Documents)
                })
                .map(Some),
            _ => return ServerStep::default(),
        };
        match generation {
            Ok(Some(generation)) => self.analyze_revision(generation),
            Ok(None) => ServerStep::default(),
            Err(issue) => ServerStep {
                issues: vec![issue].into_boxed_slice(),
                ..ServerStep::default()
            },
        }
    }

    fn watched_files(&mut self, params: Option<Value>) -> ServerStep {
        if self.watcher != WatcherState::Registered {
            return ServerStep {
                issues: vec![ServerIssue::ClientResponse(
                    ClientResponseError::WatcherNotRegistered,
                )]
                .into_boxed_slice(),
                ..ServerStep::default()
            };
        }
        let params = match DidChangeWatchedFilesParams::decode(params) {
            Ok(params) => params,
            Err(error) => {
                return ServerStep {
                    issues: vec![ServerIssue::Parameters(error)].into_boxed_slice(),
                    ..ServerStep::default()
                };
            }
        };
        let mut seen = BTreeSet::new();
        let changed = params
            .changes()
            .iter()
            .filter(|change| seen.insert(change.uri().clone()))
            .map(|change| change.uri().clone())
            .collect::<Vec<_>>();
        if changed.is_empty() {
            return ServerStep::default();
        }
        let generation = match self.documents.refresh(&changed) {
            Ok(generation) => generation,
            Err(error) => {
                return ServerStep {
                    issues: vec![ServerIssue::Documents(error)].into_boxed_slice(),
                    ..ServerStep::default()
                };
            }
        };
        self.analyze_revision(generation)
    }

    fn analyze_revision(&mut self, generation: WorkspaceSourceRevision) -> ServerStep {
        let batch = match self
            .analyses
            .as_mut()
            .expect("initialized server owns workspace analyses")
            .analyze(generation)
        {
            Ok(batch) => batch,
            Err(error) => {
                return ServerStep {
                    issues: vec![ServerIssue::WorkspaceRevision(error)].into_boxed_slice(),
                    ..ServerStep::default()
                };
            }
        };
        let mut outbound = Vec::new();
        let mut issues = Vec::new();
        for analysis in batch.publication_order() {
            match self.diagnostics.publish(analysis) {
                Ok(messages) => outbound.extend(messages.into_vec()),
                Err(error) => issues.push(ServerIssue::Diagnostics(error)),
            }
        }
        ServerStep {
            outbound: outbound.into_boxed_slice(),
            #[cfg(test)]
            analysis: Some(batch),
            issues: issues.into_boxed_slice(),
            ..ServerStep::default()
        }
    }
}

fn internal_transition_error(id: &RequestId, error: LifecycleTransitionError) -> ServerStep {
    let detail = Value::String(error.to_string().into_boxed_str());
    ServerStep {
        response: Some(render_error_response(
            Some(id),
            ResponseErrorCode::InternalError,
            Some(&detail),
        )),
        issues: vec![ServerIssue::Lifecycle(error)].into_boxed_slice(),
        ..ServerStep::default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WatcherState {
    Unavailable,
    Registering,
    Registered,
    Failed,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_analysis::SemanticHighlightKind;
    use nocter_lsp::{DocumentUri, OutboundRequestError};
    use nocter_model::{CompilationTarget, PackageIdentity};
    use nocter_package::StandardPackage;
    use nocter_workspace_revision::GenerationId;

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn invalid_initialize_can_retry_before_document_generations_begin() {
        let mut server = server("0.14.0-dev");
        let invalid =
            server.receive(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        assert!(invalid.response().unwrap().contains("\"code\":-32602"));

        let initialized = server.receive(
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"capabilities":{}}}"#,
        );
        assert!(initialized.response().unwrap().contains("\"result\""));
        assert!(server.initialization().is_some());
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#);

        let uri = format!(
            "file:///tmp/nocter-language-server-virtual-{}.nct",
            std::process::id()
        );
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"func main(): void {{}}\"}}}}}}"
        ));
        assert_eq!(
            opened.generation().unwrap().generation(),
            GenerationId::new(1)
        );
        assert_eq!(opened.outbound_messages().len(), 1);
        assert!(opened.outbound_messages()[0].contains("window/showMessage"));

        let stale = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"version\":1}},\"contentChanges\":[{{\"text\":\"stale\"}}]}}}}"
        ));
        assert!(stale.generation().is_none());
        assert!(stale.issue().is_none());

        let unknown = server
            .receive(r#"{"jsonrpc":"2.0","id":"x","method":"textDocument/unknown","params":{}}"#);
        assert!(unknown.response().unwrap().contains("\"code\":-32601"));

        let shutdown = server.receive(r#"{"jsonrpc":"2.0","id":3,"method":"shutdown"}"#);
        assert_eq!(
            shutdown.response(),
            Some(r#"{"jsonrpc":"2.0","id":3,"result":null}"#)
        );
        let exit = server.receive(r#"{"jsonrpc":"2.0","method":"exit"}"#);
        assert_eq!(exit.exit_code(), Some(0));
    }

    #[test]
    fn malformed_document_notification_is_reported_without_a_protocol_response() {
        let mut server = server("dev");
        server.receive(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#,
        );
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let step =
            server.receive(r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{}}"#);
        assert!(step.response().is_none());
        assert!(matches!(step.issue(), Some(ServerIssue::Parameters(_))));
    }

    #[test]
    fn correlates_dynamic_watcher_registration_before_accepting_disk_changes() {
        let mut server = server("dev");
        server.receive(concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",",
            "\"params\":{\"capabilities\":{\"workspace\":{",
            "\"didChangeWatchedFiles\":{\"dynamicRegistration\":true}}}}}"
        ));
        let initialized = server.receive(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#);
        assert_eq!(initialized.outbound_messages().len(), 1);
        assert!(initialized.outbound_messages()[0].contains("client/registerCapability"));
        assert!(initialized.outbound_messages()[0].contains("**/*.nct"));

        let uri = format!("file:///tmp/nocter-watched-{}.nct", std::process::id());
        let watched = format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"workspace/didChangeWatchedFiles\",\"params\":{{\"changes\":[{{\"uri\":\"{uri}\",\"type\":2}}]}}}}"
        );
        let premature = server.receive(&watched);
        assert!(matches!(
            premature.issue(),
            Some(ServerIssue::ClientResponse(
                ClientResponseError::WatcherNotRegistered
            ))
        ));

        let registered = server.receive(r#"{"jsonrpc":"2.0","id":1,"result":null}"#);
        assert!(registered.issue().is_none());
        let refreshed = server.receive(&watched);
        assert_eq!(refreshed.analyses().count(), 1);
        assert_eq!(
            refreshed.generation().unwrap().generation(),
            GenerationId::new(1)
        );
        assert!(refreshed.issue().is_none());

        let unknown = server.receive(r#"{"jsonrpc":"2.0","id":99,"result":null}"#);
        assert!(matches!(
            unknown.issue(),
            Some(ServerIssue::Outbound(
                OutboundRequestError::UnknownResponse(RequestId::Integer(99))
            ))
        ));
    }

    #[test]
    fn hover_uses_normalized_checked_presentation_and_exact_name_range() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"/// Application entry.\\nfunc  main( ): void {{ return }}\\n\"}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.diagnostics()
        );

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":1,\"character\":7}}}}}}"
        ));
        let response = hover.response().unwrap();
        assert!(response.contains("```nocter\\nfunc main(): void\\n```"));
        assert!(response.contains("Application entry."));
        assert!(response.contains("\"start\":{\"line\":1,\"character\":6}"));
        assert!(response.contains("\"end\":{\"line\":1,\"character\":10}"));

        let keyword = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":1,\"character\":1}}}}}}"
        ));
        assert_eq!(
            keyword.response(),
            Some(r#"{"jsonrpc":"2.0","id":3,"result":null}"#)
        );
    }

    #[test]
    fn declaration_rule_diagnostic_preserves_unrelated_hover_authority() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let text = "primitive func forbidden(): void\nprimitive func also_forbidden(): void\n\nstruct Value {\n    number: i32\n}\n";
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{text_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        assert_eq!(
            snapshot
                .diagnostics()
                .iter()
                .filter(|diagnostic| diagnostic.code() == "E0208")
                .count(),
            2
        );

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":8}}}}}}"
        ));
        let response = hover.response().unwrap();
        assert!(response.contains("```nocter\\nstruct Value"), "{response}");
        assert!(response.contains("\"start\":{\"line\":3,\"character\":7}"));
        assert!(response.contains("\"end\":{\"line\":3,\"character\":12}"));

        let definition = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":8}}}}}}"
        ));
        let response = definition.response().unwrap();
        assert!(response.contains("/main.nct"), "{response}");
        assert!(response.contains("\"start\":{\"line\":3,\"character\":7}"));

        let tokens = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}}}}}}"
        ));
        let response = tokens.response().unwrap();
        assert!(response.contains("\"data\":["), "{response}");
        assert!(!response.contains("\"data\":[]"), "{response}");
    }

    #[test]
    fn mutable_binding_hover_uses_the_checked_var_introducer() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let text = "func main(): i32 {\n    var count = 1\n    count\n}\n";
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{text_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.diagnostics()
        );

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":2,\"character\":6}}}}}}"
        ));
        let response = hover.response().unwrap();
        assert!(
            response.contains("```nocter\\nvar count: i32\\n```"),
            "{response}"
        );
        assert!(
            response.contains("\"start\":{\"line\":2,\"character\":4}"),
            "{response}"
        );
        assert!(
            response.contains("\"end\":{\"line\":2,\"character\":9}"),
            "{response}"
        );
        assert!(hover.issue().is_none(), "{:?}", hover.issue());
    }

    #[test]
    fn catch_bindings_keep_one_exact_local_identity_across_hover_and_tokens() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let text = "func recover(input: i32!): i32 { move input catch failure { 0 } }\n";
        let failure_start = u32::try_from(text.find("failure").unwrap()).unwrap();
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{text_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.diagnostics()
        );
        let source = snapshot
            .sources()
            .iter()
            .find(|candidate| candidate.text() == text)
            .unwrap();
        let binding = snapshot
            .semantic_highlights(source.id())
            .unwrap()
            .iter()
            .find(|highlight| highlight.range().start().get() == failure_start)
            .copied()
            .unwrap();
        assert_eq!(binding.kind(), SemanticHighlightKind::Variable);
        assert!(binding.is_declaration());
        assert!(binding.is_readonly());
        assert_eq!(source.text_at(binding.range()), Some("failure"));

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":0,\"character\":{}}}}}}}",
            failure_start + 2
        ));
        let response = hover.response().unwrap();
        assert!(
            response.contains("```nocter\\nlet failure: error\\n```"),
            "{response}"
        );
        assert!(
            response.contains(&format!(
                "\"start\":{{\"line\":0,\"character\":{failure_start}}}"
            )),
            "{response}"
        );
        assert!(
            response.contains(&format!(
                "\"end\":{{\"line\":0,\"character\":{}}}",
                failure_start + 7
            )),
            "{response}"
        );
        assert!(hover.issue().is_none(), "{:?}", hover.issue());

        let tokens = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}}}}}}"
        ));
        assert!(
            tokens
                .response()
                .is_some_and(|response| response.contains("\"resultId\":\"1\""))
        );
        assert!(tokens.issue().is_none(), "{:?}", tokens.issue());
    }

    #[test]
    fn authority_diagnostics_do_not_hide_independently_typed_body_semantics() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("index.nct");
        let uri = format!("file://{}", source.display());
        let text = concat!(
            "#package: { name: \"ordinary\", version: \"0.1.0\" }\n",
            "primitive func unauthorized(): i32\n",
            "instance str {\n",
            "    method &self.inspect(): i32 {\n",
            "        let retained = identity(1)\n",
            "        return retained\n",
            "    }\n",
            "}\n",
            "func identity(value: i32): i32 { return value }\n",
        );
        fs::write(&source, text).unwrap();
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{text_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        assert_eq!(snapshot.diagnostics()[0].code(), "E0208");

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":4,\"character\":14}}}}}}"
        ));
        let response = hover.response().unwrap();
        assert!(
            response.contains("```nocter\\nlet retained: i32\\n```"),
            "{response}"
        );
        assert!(hover.issue().is_none(), "{:?}", hover.issue());

        let hints = server.receive(&format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":3,",
                "\"method\":\"textDocument/inlayHint\",",
                "\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},",
                "\"range\":{{\"start\":{{\"line\":4,\"character\":0}},",
                "\"end\":{{\"line\":4,\"character\":34}}}}}}}}"
            ),
            uri = uri,
        ));
        let response = hints.response().unwrap();
        assert!(
            response.contains(concat!(
                "\"position\":{\"line\":4,\"character\":20},",
                "\"label\":\": i32\",\"kind\":1"
            )),
            "{response}"
        );
        assert!(hints.issue().is_none(), "{:?}", hints.issue());

        let signature = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/signatureHelp\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":4,\"character\":33}}}}}}"
        ));
        let response = signature.response().unwrap();
        assert!(
            response.contains("func identity(value: i32): i32"),
            "{response}"
        );
        assert!(signature.issue().is_none(), "{:?}", signature.issue());

        let source = snapshot
            .sources()
            .iter()
            .find(|candidate| candidate.text() == text)
            .unwrap();
        assert!(
            snapshot
                .semantic_highlights(source.id())
                .unwrap()
                .iter()
                .any(|highlight| {
                    highlight.kind() == SemanticHighlightKind::Variable
                        && source.text_at(highlight.range()) == Some("retained")
                })
        );
    }

    #[test]
    fn type_hover_is_independent_of_the_construction_surface() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"pub struct Widget<T> {{\\n    pub visible: T\\n    pub hidden: T\\n}}\\n\\nconstruct Widget<T> {{\\n    pub func alternate(visible: T, hidden: T): Self {{ return Self {{ visible: move visible, hidden: move hidden }} }}\\n    pub func new(visible: T, hidden: T): Self {{ return Self {{ visible: move visible, hidden: move hidden }} }}\\n}}\\n\\nfunc main(): void {{ return }}\\n\"}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.diagnostics()
        );

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":0,\"character\":12}}}}}}"
        ));
        let response = hover.response().unwrap();
        assert!(
            response
                .contains("pub struct Widget<T> {\\n    pub visible: T\\n    pub hidden: T\\n}"),
            "{response}"
        );
        assert!(!response.contains("construct Widget"), "{response}");
        assert!(!response.contains("func alternate"), "{response}");
        assert!(!response.contains("func new"), "{response}");
        assert!(hover.issue().is_none(), "{:?}", hover.issue());
    }

    #[test]
    fn type_hover_presents_complete_visible_nominal_shapes() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"pub struct Pair {{\\n    pub left: i32\\n    pub right: i32\\n}}\\n\\npub struct Sealed {{\\n    pub visible: i32\\n    hidden: i32\\n}}\\n\\npub enum Choice {{\\n    first\\n    second(value: i32)\\n}}\\n\\nfunc main(): void {{ return }}\\n\"}}}}}}"
        ));
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );

        let pair = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":0,\"character\":12}}}}}}"
        ));
        let response = pair.response().unwrap();
        assert!(
            response.contains("pub struct Pair {\\n    pub left: i32\\n    pub right: i32\\n}"),
            "{response}"
        );

        let sealed = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":5,\"character\":12}}}}}}"
        ));
        let response = sealed.response().unwrap();
        assert!(
            response.contains("pub struct Sealed {\\n    pub visible: i32\\n    hidden: i32\\n}"),
            "{response}"
        );
        let choice = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":10,\"character\":11}}}}}}"
        ));
        let response = choice.response().unwrap();
        assert!(
            response.contains("pub enum Choice {\\n    first\\n    second(value: i32)\\n}"),
            "{response}"
        );
        assert!(choice.issue().is_none(), "{:?}", choice.issue());
    }

    #[test]
    fn type_hover_does_not_expose_an_opaque_representation() {
        let temporary = TemporaryDirectory::new();
        let widgets = temporary.path().join("widgets");
        fs::create_dir(&widgets).unwrap();
        fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"hover-opaque\", version: \"0.1.0\", }\n#executable: {\n    name: \"hover-opaque\",\n}\n",
        )
        .unwrap();
        fs::write(
            widgets.join("index.nct"),
            "see ./representation.nct\n\npub struct Widget\n",
        )
        .unwrap();
        fs::write(
            widgets.join("representation.nct"),
            "see ./index.nct\n\nstruct Widget {\n    visible: i32\n    hidden: i32\n}\n",
        )
        .unwrap();
        let source = temporary.path().join("app.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"use ./widgets.Widget\\n\\nfunc main(): void {{ return }}\\n\"}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.diagnostics()
        );

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":0,\"character\":16}}}}}}"
        ));
        let response = hover.response().unwrap();
        assert!(response.contains("pub struct Widget\\n"), "{response}");
        assert!(!response.contains("visible: i32"), "{response}");
        assert!(!response.contains("hidden: i32"), "{response}");
        assert!(hover.issue().is_none(), "{:?}", hover.issue());
    }

    #[test]
    fn type_hover_keeps_the_authored_nominal_identity() {
        let temporary = TemporaryDirectory::new();
        let widgets = temporary.path().join("widgets");
        fs::create_dir(&widgets).unwrap();
        fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"hover-alias\", version: \"0.1.0\", }\n#executable: {\n    name: \"hover-alias\",\n}\n",
        )
        .unwrap();
        fs::write(
            widgets.join("index.nct"),
            "pub struct Widget {\n    pub value: i32\n}\n\nconstruct Widget {\n    pub func new(): Self { return Widget { value: 1 } }\n}\n",
        )
        .unwrap();
        let source = temporary.path().join("app.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"use ./widgets.Widget as LocalWidget\\n\\nfunc main(): void {{\\n    let value: LocalWidget = LocalWidget.new()\\n    return\\n}}\\n\"}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.diagnostics()
        );

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":18}}}}}}"
        ));
        let response = hover.response().unwrap();
        assert!(response.contains("pub struct Widget"), "{response}");
        assert!(response.contains("pub value: i32"), "{response}");
        assert!(!response.contains("construct"), "{response}");
        assert!(!response.contains("LocalWidget"), "{response}");
        assert!(!response.contains("widgets.Widget"), "{response}");
        assert!(hover.issue().is_none(), "{:?}", hover.issue());
    }

    #[test]
    fn semantic_tokens_use_exact_compiler_bindings_instead_of_syntax_ranges() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"func  main( ): void {{ return }}\\n\"}}}}}}"
        ));

        let tokens = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}}}}}}"
        ));
        assert_eq!(
            tokens.response(),
            Some(
                r#"{"jsonrpc":"2.0","id":2,"result":{"resultId":"1","data":[0,6,4,10,1,0,9,4,1,0]}}"#
            )
        );
        assert!(tokens.issue().is_none());

        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"version\":2}},\"contentChanges\":[{{\"text\":\"func main(): void {{ return }}\\n\"}}]}}}}"
        ));
        let changed_tokens = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}}}}}}"
        ));
        assert!(
            changed_tokens
                .response()
                .is_some_and(|response| response.contains("\"resultId\":\"2\"")),
            "{:?}",
            changed_tokens.response()
        );
        assert!(changed_tokens.issue().is_none());

        let memory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/mem/raw_buffer.nct");
        let memory_uri = DocumentUri::new(format!("file://{}", memory.display())).unwrap();
        let memory = crate::semantic_document::semantic_document(
            &server.documents,
            server.analyses.as_ref().unwrap(),
            &memory_uri,
        )
        .unwrap()
        .unwrap();
        let field_starts = memory
            .source()
            .text()
            .match_indices("if len > buffer.len")
            .map(|(start, _)| u32::try_from(start + "if len > buffer.".len()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(field_starts.len(), 2);
        let highlights = memory
            .snapshot()
            .semantic_highlights(memory.source().id())
            .unwrap();
        let readonly = highlights
            .iter()
            .find(|highlight| highlight.range().start().get() == field_starts[0])
            .unwrap();
        let writable = highlights
            .iter()
            .find(|highlight| highlight.range().start().get() == field_starts[1])
            .unwrap();
        assert_eq!(readonly.kind(), SemanticHighlightKind::Property);
        assert!(readonly.is_readonly());
        assert_eq!(writable.kind(), SemanticHighlightKind::Property);
        assert!(!writable.is_readonly());
    }

    #[test]
    fn semantic_tokens_classify_readonly_receiver_as_a_parameter() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"func main(): void {{ return }}\\n\"}}}}}}"
        ));

        let standard = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/str/index.nct");
        let standard_uri = DocumentUri::new(format!("file://{}", standard.display())).unwrap();
        let document = crate::semantic_document::semantic_document(
            &server.documents,
            server.analyses.as_ref().unwrap(),
            &standard_uri,
        )
        .unwrap()
        .unwrap();
        let receiver_start = document
            .source()
            .text()
            .find("pub method &self.len(): usize")
            .unwrap()
            + "pub method &".len();
        let receiver_start = u32::try_from(receiver_start).unwrap();
        let receiver = document
            .snapshot()
            .semantic_highlights(document.source().id())
            .unwrap()
            .iter()
            .find(|highlight| highlight.range().start().get() == receiver_start)
            .copied()
            .unwrap();
        assert_eq!(receiver.kind(), SemanticHighlightKind::Parameter);
        assert!(receiver.is_declaration());
        assert!(receiver.is_readonly());
        assert_eq!(document.source().text_at(receiver.range()), Some("self"));
        let some_start = u32::try_from(
            document
                .source()
                .text()
                .find("some Iterator { .Item = &str }")
                .unwrap(),
        )
        .unwrap();
        let opaque = document
            .snapshot()
            .semantic_highlights(document.source().id())
            .unwrap()
            .iter()
            .find(|highlight| highlight.range().start().get() == some_start)
            .copied()
            .unwrap();
        assert_eq!(opaque.kind(), SemanticHighlightKind::Keyword);
        assert_eq!(document.source().text_at(opaque.range()), Some("some"));

        for source in document.snapshot().sources().iter() {
            let highlights = document
                .snapshot()
                .semantic_highlights(source.id())
                .unwrap();
            for highlight in &highlights {
                let range = source.utf16_range(highlight.range()).unwrap();
                assert_eq!(
                    range.start().line(),
                    range.end().line(),
                    "multiline semantic range in {}: {:?}",
                    source.name(),
                    source.text_at(highlight.range())
                );
            }
            for pair in highlights.windows(2) {
                assert!(
                    pair[0].range().end() <= pair[1].range().start(),
                    "overlapping semantic ranges in {}: {:?} and {:?}",
                    source.name(),
                    source.text_at(pair[0].range()),
                    source.text_at(pair[1].range())
                );
            }
        }

        let standard_uri = standard_uri.as_str();
        let tokens = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"{standard_uri}\"}}}}}}"
        ));
        let response = tokens.response().unwrap();
        assert!(
            response.contains("\"result\":{\"resultId\":\"1\",\"data\":["),
            "unexpected response {response:?} with issue {:?}",
            tokens.issue()
        );
        assert!(tokens.issue().is_none());
    }

    #[test]
    fn hover_rejects_positions_outside_the_current_source() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"func main(): void {{ return }}\\n\"}}}}}}"
        ));

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":9,\"character\":0}}}}}}"
        ));
        assert!(hover.response().unwrap().contains("\"code\":-32602"));
        assert!(matches!(hover.issue(), Some(ServerIssue::Hover(_))));
    }

    #[test]
    fn hover_normalizes_method_self_to_its_semantic_owner() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let source_uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{source_uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"func main(): void {{ return }}\\n\"}}}}}}"
        ));

        let standard = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/str/index.nct");
        let text = fs::read_to_string(&standard).unwrap();
        let (line, source_line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("pub method &self.len(): usize"))
            .unwrap();
        let character = source_line.find("len").unwrap();
        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
            standard.display()
        ));
        let response = hover.response().unwrap();
        assert!(response.contains("```nocter\\npub method &str.len(): usize\\n```"));
        assert!(response.contains(&format!(
            "\"start\":{{\"line\":{line},\"character\":{character}}}"
        )));

        let vec_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/vec/index.nct");
        let text = fs::read_to_string(&vec_source).unwrap();
        let (line, source_line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("pub func from_exact_iter<I>"))
            .unwrap();
        let character = source_line.find("from_exact_iter").unwrap();
        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
            vec_source.display()
        ));
        assert!(hover.response().unwrap().contains(concat!(
            "pub func Vec<T>.from_exact_iter<I>(iterator: I): Vec<T> where ",
            "I impl ExactSizeIterator { .Item = T }"
        )));

        let iter_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/iter/index.nct");
        let text = fs::read_to_string(&iter_source).unwrap();
        let (line, source_line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("pub method &+self.next(): Self.Item?"))
            .unwrap();
        let character = source_line.find("next").unwrap();
        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
            iter_source.display()
        ));
        let response = hover.response().unwrap();
        assert!(response.contains("pub method &+Iterator.next(): Iterator.Item?"));
        assert!(!response.contains(" from self"));

        let (line, source_line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("pub interface ExactSizeIterator"))
            .unwrap();
        let character = source_line.find("ExactSizeIterator").unwrap();
        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
            iter_source.display()
        ));
        assert!(
            hover
                .response()
                .unwrap()
                .contains("pub interface ExactSizeIterator where Self impl Iterator")
        );

        let vec_text = fs::read_to_string(&vec_source).unwrap();
        let (line, source_line) = vec_text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("I impl ExactSizeIterator { .Item = T }"))
            .unwrap();
        let character = source_line.find("Item").unwrap();
        let definition = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
            vec_source.display()
        ));
        let response = definition.response().unwrap();
        assert!(response.contains("/std/iter/index.nct"), "{response}");
    }

    #[test]
    fn directory_stream_contract_and_body_share_complete_editor_semantics() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let source_uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{source_uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"use std/fs.read_dir\\nfunc main(): void {{ return }}\\n\"}}}}}}"
        ));

        let completion = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{source_uri}\"}},\"position\":{{\"line\":1,\"character\":20}}}}}}"
        ));
        let response = completion.response().unwrap();
        assert!(response.contains("\"label\":\"read_dir\""), "{response}");
        assert!(completion.issue().is_none(), "{:?}", completion.issue());

        let definition = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{source_uri}\"}},\"position\":{{\"line\":0,\"character\":14}}}}}}"
        ));
        let response = definition.response().unwrap();
        assert!(response.contains("/std/fs/index.nct"), "{response}");
        assert!(definition.issue().is_none(), "{:?}", definition.issue());

        let contract = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/fs/index.nct");
        let text = fs::read_to_string(&contract).unwrap();
        let (line, source_line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("pub func read_dir(path: &str): ReadDir!"))
            .unwrap();
        let character = source_line.find("read_dir").unwrap();
        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
            contract.display()
        ));
        let response = hover.response().unwrap();
        assert!(
            response.contains("```nocter\\npub func read_dir(path: &str): ReadDir!\\n```"),
            "{response}"
        );
        assert!(hover.issue().is_none(), "{:?}", hover.issue());

        let body = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/fs/directory.nct");
        let text = fs::read_to_string(&body).unwrap();
        let (line, source_line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("if record_len <= dirent_name_offset"))
            .unwrap();
        let character = source_line.find("record_len").unwrap();
        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
            body.display()
        ));
        let response = hover.response().unwrap();
        assert!(
            response.contains("```nocter\\nlet record_len: usize\\n```"),
            "{response}"
        );
        assert!(hover.issue().is_none(), "{:?}", hover.issue());
    }

    #[test]
    fn streaming_line_contract_and_body_share_complete_editor_semantics() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let source_uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{source_uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"use std/io/buffer.BufReader\\nfunc inspect(reader: &+BufReader): void! {{\\n    let _line = reader.read_line()?\\n    return\\n}}\\n\"}}}}}}"
        ));

        let completion = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{source_uri}\"}},\"position\":{{\"line\":2,\"character\":23}}}}}}"
        ));
        let response = completion.response().unwrap();
        assert!(
            response.contains("\"label\":\"read_line\",\"kind\":2"),
            "{response}"
        );
        assert!(
            response.contains("\"label\":\"read_line_into\",\"kind\":2"),
            "{response}"
        );
        assert!(completion.issue().is_none(), "{:?}", completion.issue());

        let definition = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{source_uri}\"}},\"position\":{{\"line\":0,\"character\":20}}}}}}"
        ));
        let response = definition.response().unwrap();
        assert!(response.contains("/std/io/buffer/index.nct"), "{response}");
        assert!(definition.issue().is_none(), "{:?}", definition.issue());

        let contract =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/io/buffer/index.nct");
        let text = fs::read_to_string(&contract).unwrap();
        let (line, source_line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("pub method &+self.read_line(): String?!"))
            .unwrap();
        let character = source_line.find("read_line").unwrap();
        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
            contract.display()
        ));
        let response = hover.response().unwrap();
        assert!(
            response.contains("```nocter\\npub method &+BufReader.read_line(): String?!\\n```"),
            "{response}"
        );
        assert!(hover.issue().is_none(), "{:?}", hover.issue());

        let body =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/io/buffer/buffering.nct");
        let text = fs::read_to_string(&body).unwrap();
        let (line, source_line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("let ends_with_cr ="))
            .unwrap();
        let character = source_line.find("ends_with_cr").unwrap();
        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
            body.display()
        ));
        let response = hover.response().unwrap();
        assert!(
            response.contains("```nocter\\nlet ends_with_cr: bool\\n```"),
            "{response}"
        );
        assert!(hover.issue().is_none(), "{:?}", hover.issue());
    }

    #[test]
    fn collection_ordering_uses_slice_semantics_for_vec_editor_queries() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let source_uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{source_uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"use std/vec.Vec\\nfunc inspect(values: &+Vec<i32>): void {{\\n    values.sort()\\n    return\\n}}\\n\"}}}}}}"
        ));

        let completion = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{source_uri}\"}},\"position\":{{\"line\":2,\"character\":11}}}}}}"
        ));
        let response = completion.response().unwrap();
        assert!(
            response.contains("\"label\":\"sort\",\"kind\":2"),
            "{response}"
        );
        assert!(completion.issue().is_none(), "{:?}", completion.issue());

        let definition = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{source_uri}\"}},\"position\":{{\"line\":2,\"character\":13}}}}}}"
        ));
        let response = definition.response().unwrap();
        assert!(response.contains("/std/slice/index.nct"), "{response}");
        assert!(!response.contains("/std/vec/index.nct"), "{response}");
        assert!(definition.issue().is_none(), "{:?}", definition.issue());

        let contract = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/slice/index.nct");
        let text = fs::read_to_string(&contract).unwrap();
        let (line, source_line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("pub method &+self.sort(): void"))
            .unwrap();
        let character = source_line.find("sort").unwrap();
        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
            contract.display()
        ));
        let response = hover.response().unwrap();
        assert!(
            response.contains("sort(): void where (&T < &T): bool"),
            "{response}"
        );
        assert!(hover.issue().is_none(), "{:?}", hover.issue());

        let body = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/slice/ordering.nct");
        let text = fs::read_to_string(&body).unwrap();
        let (line, source_line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("let element_size = pointee_size(pointer)"))
            .unwrap();
        let character = source_line.find("element_size").unwrap();
        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
            body.display()
        ));
        let response = hover.response().unwrap();
        assert!(
            response.contains("```nocter\\nlet element_size: usize\\n```"),
            "{response}"
        );
        assert!(hover.issue().is_none(), "{:?}", hover.issue());
    }

    fn server(version: &str) -> LanguageServer {
        let root = std::env::temp_dir();
        LanguageServer::new(
            version,
            LanguageServerEnvironment::new(
                &root,
                crate::LanguageServerToolchain::new(
                    CompilationTarget::Arm64Darwin,
                    &root,
                    StandardPackage::new(PackageIdentity::new("toolchain:std"), &root),
                ),
            ),
        )
    }

    pub(super) fn semantic_server(root: &Path) -> LanguageServer {
        let standard_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std");
        LanguageServer::new(
            "dev",
            LanguageServerEnvironment::new(
                root,
                crate::LanguageServerToolchain::new(
                    CompilationTarget::Arm64Darwin,
                    root,
                    StandardPackage::new(PackageIdentity::new("toolchain:std"), standard_root),
                ),
            ),
        )
    }

    pub(super) struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        pub(super) fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-language-server-hover-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        pub(super) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
