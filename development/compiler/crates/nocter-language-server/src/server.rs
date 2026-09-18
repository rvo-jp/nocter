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
            .flat_map(WorkspaceAnalysisBatch::updated_generations)
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
        match self.outbound_requests.begin(
            "client/registerCapability",
            &watched_files_registration(nocter_language::SOURCE_FILE_GLOB),
        ) {
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
        let (outbound, issues) = match self.diagnostics.publish(&batch) {
            Ok(messages) => (messages, Vec::new()),
            Err(error) => (
                Vec::new().into_boxed_slice(),
                vec![ServerIssue::Diagnostics(error)],
            ),
        };
        ServerStep {
            outbound,
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
mod tests;
