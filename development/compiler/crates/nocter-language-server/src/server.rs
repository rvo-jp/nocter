use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use nocter_json::Value;
use nocter_lsp::{
    DidChangeParams, DidChangeWatchedFilesParams, DidCloseParams, DidOpenParams, DidSaveParams,
    HoverParams, IncomingMessage, InitializeParams, LifecycleTransitionError, OutboundRequestError,
    OutboundRequests, ParameterError, ProtocolEvent, ProtocolSession, RequestId, ResponseError,
    ResponseErrorCode, ResponseResult, initialize_result, render_error_response,
    render_success_response, watched_files_registration,
};

use crate::hover::{HoverQueryError, query_hover};
use crate::{
    AcceptedDocumentGeneration, DiagnosticPublicationError, DiagnosticPublisher, DocumentWorkspace,
    DocumentWorkspaceChange, DocumentWorkspaceError, LanguageServerEnvironment, WorkspaceAnalyses,
    WorkspaceAnalysisGeneration, WorkspaceConfiguration, WorkspaceConfigurationError,
};

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

    fn hover(&self, id: &RequestId, params: Option<Value>) -> ServerStep {
        let params = match HoverParams::decode(params) {
            Ok(params) => params,
            Err(error) => {
                let detail = Value::String(error.to_string().into_boxed_str());
                return ServerStep {
                    response: Some(render_error_response(
                        Some(id),
                        ResponseErrorCode::InvalidParams,
                        Some(&detail),
                    )),
                    ..ServerStep::default()
                };
            }
        };
        let result = query_hover(
            &self.documents,
            self.analyses
                .as_ref()
                .expect("initialized server owns workspace analyses"),
            &params,
        );
        match result {
            Ok(result) => ServerStep {
                response: Some(render_success_response(id, &result)),
                ..ServerStep::default()
            },
            Err(error) => {
                let detail = Value::String(error.to_string().into_boxed_str());
                ServerStep {
                    response: Some(render_error_response(
                        Some(id),
                        ResponseErrorCode::InvalidParams,
                        Some(&detail),
                    )),
                    issues: vec![ServerIssue::Hover(error)].into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
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

#[derive(Debug)]
pub enum ServerIssue {
    Parameters(ParameterError),
    Documents(DocumentWorkspaceError),
    Diagnostics(DiagnosticPublicationError),
    Hover(HoverQueryError),
    Outbound(OutboundRequestError),
    ClientResponse(ClientResponseError),
    Workspace(WorkspaceConfigurationError),
    Lifecycle(LifecycleTransitionError),
}

impl fmt::Display for ServerIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parameters(error) => error.fmt(formatter),
            Self::Documents(error) => error.fmt(formatter),
            Self::Diagnostics(error) => error.fmt(formatter),
            Self::Hover(error) => error.fmt(formatter),
            Self::Outbound(error) => error.fmt(formatter),
            Self::ClientResponse(error) => error.fmt(formatter),
            Self::Workspace(error) => error.fmt(formatter),
            Self::Lifecycle(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ServerIssue {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parameters(error) => Some(error),
            Self::Documents(error) => Some(error),
            Self::Diagnostics(error) => Some(error),
            Self::Hover(error) => Some(error),
            Self::Outbound(error) => Some(error),
            Self::ClientResponse(error) => Some(error),
            Self::Workspace(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WatcherState {
    Unavailable,
    Registering,
    Registered,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClientResponseError {
    InvalidRegistrationResult,
    RegistrationRejected(ResponseError),
    WatcherNotRegistered,
}

impl fmt::Display for ClientResponseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRegistrationResult => {
                formatter.write_str("watched-file registration response result is not null")
            }
            Self::RegistrationRejected(error) => error.fmt(formatter),
            Self::WatcherNotRegistered => {
                formatter.write_str("client sent watched-file changes before registration")
            }
        }
    }
}

impl std::error::Error for ClientResponseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RegistrationRejected(error) => Some(error),
            Self::InvalidRegistrationResult | Self::WatcherNotRegistered => None,
        }
    }
}

#[derive(Debug)]
enum InitializeFailure {
    Parameters(ParameterError),
    Workspace(WorkspaceConfigurationError),
}

impl InitializeFailure {
    fn into_server_issue(self) -> Option<ServerIssue> {
        match self {
            Self::Parameters(_) => None,
            Self::Workspace(error) => Some(ServerIssue::Workspace(error)),
        }
    }
}

impl From<ParameterError> for InitializeFailure {
    fn from(error: ParameterError) -> Self {
        Self::Parameters(error)
    }
}

impl fmt::Display for InitializeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parameters(error) => error.fmt(formatter),
            Self::Workspace(error) => error.fmt(formatter),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_analysis::GenerationId;
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
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"func  main( ): void {{ return }}\\n\"}}}}}}"
        ));
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":0,\"character\":7}}}}}}"
        ));
        let response = hover.response().unwrap();
        assert!(response.contains("```nocter\\nfunc main(): void\\n```"));
        assert!(response.contains("\"start\":{\"line\":0,\"character\":6}"));
        assert!(response.contains("\"end\":{\"line\":0,\"character\":10}"));

        let keyword = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":0,\"character\":1}}}}}}"
        ));
        assert_eq!(
            keyword.response(),
            Some(r#"{"jsonrpc":"2.0","id":3,"result":null}"#)
        );
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

    fn semantic_server(root: &Path) -> LanguageServer {
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

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-language-server-hover-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
