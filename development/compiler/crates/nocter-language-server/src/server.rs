use std::collections::BTreeSet;
use std::sync::Arc;

use nocter_json::Value;
use nocter_lsp::{
    DidChangeParams, DidChangeWatchedFilesParams, DidCloseParams, DidOpenParams, DidSaveParams,
    IncomingMessage, InitializeParams, LifecycleTransitionError, OutboundRequests, ProtocolEvent,
    ProtocolSession, RequestId, ResponseErrorCode, ResponseResult, initialize_result,
    render_error_response, render_success_response, watched_files_registration,
};

use crate::{
    AcceptedDocumentGeneration, DiagnosticPublisher, DocumentWorkspace, DocumentWorkspaceChange,
    LanguageServerEnvironment, WorkspaceAnalyses, WorkspaceAnalysisGeneration,
    WorkspaceConfiguration,
};

mod issues;
mod semantic_requests;

use issues::InitializeFailure;
pub use issues::{ClientResponseError, ServerIssue};

/// One fully validated protocol and document-state transition.
#[derive(Debug, Default)]
pub struct ServerStep {
    response: Option<String>,
    outbound: Box<[String]>,
    analyses: Box<[Arc<WorkspaceAnalysisGeneration>]>,
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

    #[must_use]
    pub fn generation(&self) -> Option<&WorkspaceAnalysisGeneration> {
        self.analyses.first().map(Arc::as_ref)
    }

    #[must_use]
    pub fn analysis(&self) -> Option<&WorkspaceAnalysisGeneration> {
        self.generation()
    }

    pub fn analyses(&self) -> impl Iterator<Item = &WorkspaceAnalysisGeneration> {
        self.analyses.iter().map(Arc::as_ref)
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
                WorkspaceConfiguration::resolve(&self.environment, &params)
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
                if method.as_ref() == "textDocument/definition" =>
            {
                self.definition(&id, params)
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
        let generation: Result<Option<AcceptedDocumentGeneration>, ServerIssue> = match method {
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
            Ok(Some(generation)) => {
                let analysis = self
                    .analyses
                    .as_mut()
                    .expect("initialized server owns workspace analyses")
                    .analyze(generation);
                match self.diagnostics.publish(&analysis) {
                    Ok(outbound) => ServerStep {
                        outbound,
                        analyses: vec![analysis].into_boxed_slice(),
                        ..ServerStep::default()
                    },
                    Err(error) => ServerStep {
                        analyses: vec![analysis].into_boxed_slice(),
                        issues: vec![ServerIssue::Diagnostics(error)].into_boxed_slice(),
                        ..ServerStep::default()
                    },
                }
            }
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
        let mut snapshots = Vec::new();
        let mut outbound = Vec::new();
        let mut issues = Vec::new();
        let mut seen = BTreeSet::new();
        for change in params.changes() {
            if !seen.insert(change.uri().clone()) {
                continue;
            }
            let generation = match self.documents.refresh(change.uri()) {
                Ok(generation) => generation,
                Err(error) => {
                    issues.push(ServerIssue::Documents(error));
                    continue;
                }
            };
            let analysis = self
                .analyses
                .as_mut()
                .expect("initialized server owns workspace analyses")
                .analyze(generation);
            match self.diagnostics.publish(&analysis) {
                Ok(messages) => outbound.extend(messages.into_vec()),
                Err(error) => issues.push(ServerIssue::Diagnostics(error)),
            }
            snapshots.push(analysis);
        }
        ServerStep {
            outbound: outbound.into_boxed_slice(),
            analyses: snapshots.into_boxed_slice(),
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

    use nocter_analysis::{GenerationId, SemanticHighlightKind};
    use nocter_lsp::{DocumentUri, OutboundRequestError};
    use nocter_model::{CompilationTarget, PackageIdentity};
    use nocter_package::StandardPackage;

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
    fn type_hover_uses_the_visible_compiler_owned_construction_surface() {
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
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"pub struct Widget<T> {{\\n    pub visible: T\\n    hidden: T\\n}}\\n\\nconstruct Widget<T> {{\\n    pub func alternate(visible: T, hidden: T): Self {{ return Self {{ visible: move visible, hidden: move hidden }} }}\\n    pub default func new(visible: T, hidden: T): Self {{ return Self {{ visible: move visible, hidden: move hidden }} }}\\n}}\\n\\nfunc main(): void {{ return }}\\n\"}}}}}}"
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
            response.contains(concat!(
                "pub struct Widget<T>\\n\\nconstruct Widget<T> {\\n",
                "    pub func alternate(visible: T, hidden: T): Self\\n",
                "    pub default func new(visible: T, hidden: T): Self\\n}"
            )),
            "{response}"
        );
        assert!(!response.contains("\\n    pub visible: T"), "{response}");
        assert!(!response.contains("\\n    hidden: T"), "{response}");
        assert!(hover.issue().is_none(), "{:?}", hover.issue());
    }

    #[test]
    fn type_hover_presents_structural_and_variant_construction_in_valid_syntax() {
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
            response.contains("```nocter\\npub struct Sealed\\n```"),
            "{response}"
        );
        assert!(!response.contains("visible: i32"), "{response}");

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
    fn type_hover_uses_the_shortest_visible_import_alias() {
        let temporary = TemporaryDirectory::new();
        let widgets = temporary.path().join("widgets");
        fs::create_dir(&widgets).unwrap();
        fs::write(
            temporary.path().join("nocter.nct"),
            "#name: \"hover-alias\"\n#version: \"0.1.0\"\n#executable: {\n    name: \"hover-alias\",\n}\n",
        )
        .unwrap();
        fs::write(
            widgets.join("index.nct"),
            "pub struct Widget {\n    pub value: i32\n}\n\nconstruct Widget {\n    pub default func new(): Self { return Widget { value: 1 } }\n}\n",
        )
        .unwrap();
        let source = temporary.path().join("index.nct");
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
        assert!(response.contains("construct LocalWidget {"), "{response}");
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
            Some(r#"{"jsonrpc":"2.0","id":2,"result":{"data":[0,6,4,10,1]}}"#)
        );
        assert!(tokens.issue().is_none());

        let memory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/mem/index.nct");
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
        let highlights = memory.snapshot().semantic_highlights(memory.source().id());
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
                .find("some Iterator<Item = &str>")
                .unwrap(),
        )
        .unwrap();
        let opaque = document
            .snapshot()
            .semantic_highlights(document.source().id())
            .iter()
            .find(|highlight| highlight.range().start().get() == some_start)
            .copied()
            .unwrap();
        assert_eq!(opaque.kind(), SemanticHighlightKind::Keyword);
        assert_eq!(document.source().text_at(opaque.range()), Some("some"));

        for source in document.snapshot().sources().iter() {
            let highlights = document.snapshot().semantic_highlights(source.id());
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
            response.contains("\"result\":{\"data\":["),
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
            "I: Iterator + ExactSizeIterator, I.Item = T"
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
