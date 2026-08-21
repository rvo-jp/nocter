use std::fmt;
use std::sync::Arc;

use nocter_json::Value;
use nocter_lsp::{
    DidChangeParams, DidCloseParams, DidOpenParams, DidSaveParams, IncomingMessage,
    InitializeParams, LifecycleTransitionError, ParameterError, ProtocolEvent, ProtocolSession,
    RequestId, ResponseErrorCode, initialize_result, render_error_response,
    render_success_response,
};

use crate::{
    AcceptedDocumentGeneration, DiagnosticPublicationError, DiagnosticPublisher, DocumentWorkspace,
    DocumentWorkspaceChange, DocumentWorkspaceError, LanguageServerEnvironment, WorkspaceAnalyses,
    WorkspaceAnalysisGeneration, WorkspaceConfiguration, WorkspaceConfigurationError,
};

/// One fully validated protocol and document-state transition.
#[derive(Debug, Default)]
pub struct ServerStep {
    response: Option<String>,
    notifications: Box<[String]>,
    analysis: Option<Arc<WorkspaceAnalysisGeneration>>,
    issue: Option<ServerIssue>,
    exit_code: Option<i32>,
}

impl ServerStep {
    #[must_use]
    pub fn response(&self) -> Option<&str> {
        self.response.as_deref()
    }

    #[must_use]
    pub const fn notifications(&self) -> &[String] {
        &self.notifications
    }

    #[must_use]
    pub fn generation(&self) -> Option<&WorkspaceAnalysisGeneration> {
        self.analysis.as_deref()
    }

    #[must_use]
    pub fn analysis(&self) -> Option<&WorkspaceAnalysisGeneration> {
        self.analysis.as_deref()
    }

    #[must_use]
    pub const fn issue(&self) -> Option<&ServerIssue> {
        self.issue.as_ref()
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
            ProtocolEvent::Initialized => ServerStep::default(),
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
                    issue: error.into_server_issue(),
                    ..ServerStep::default()
                }
            }
        }
    }

    fn message(&mut self, message: IncomingMessage) -> ServerStep {
        match message {
            IncomingMessage::Request { id, .. } => ServerStep {
                response: Some(render_error_response(
                    Some(&id),
                    ResponseErrorCode::MethodNotFound,
                    None,
                )),
                ..ServerStep::default()
            },
            IncomingMessage::Notification { method, params } => self.notification(&method, params),
        }
    }

    fn notification(&mut self, method: &str, params: Option<Value>) -> ServerStep {
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
                    Ok(notifications) => ServerStep {
                        notifications,
                        analysis: Some(analysis),
                        ..ServerStep::default()
                    },
                    Err(error) => ServerStep {
                        analysis: Some(analysis),
                        issue: Some(ServerIssue::Diagnostics(error)),
                        ..ServerStep::default()
                    },
                }
            }
            Ok(None) => ServerStep::default(),
            Err(issue) => ServerStep {
                issue: Some(issue),
                ..ServerStep::default()
            },
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
        issue: Some(ServerIssue::Lifecycle(error)),
        ..ServerStep::default()
    }
}

#[derive(Debug)]
pub enum ServerIssue {
    Parameters(ParameterError),
    Documents(DocumentWorkspaceError),
    Diagnostics(DiagnosticPublicationError),
    Workspace(WorkspaceConfigurationError),
    Lifecycle(LifecycleTransitionError),
}

impl fmt::Display for ServerIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parameters(error) => error.fmt(formatter),
            Self::Documents(error) => error.fmt(formatter),
            Self::Diagnostics(error) => error.fmt(formatter),
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
            Self::Workspace(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
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
    use nocter_analysis::GenerationId;
    use nocter_model::{CompilationTarget, PackageIdentity};
    use nocter_package::StandardPackage;

    use super::*;

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
        assert_eq!(opened.notifications().len(), 1);
        assert!(opened.notifications()[0].contains("window/showMessage"));

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
}
