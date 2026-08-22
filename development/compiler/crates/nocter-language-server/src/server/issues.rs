use std::fmt;

use nocter_lsp::{LifecycleTransitionError, OutboundRequestError, ParameterError, ResponseError};

use crate::completion::CompletionQueryError;
use crate::hover::HoverQueryError;
use crate::inlay_hints::InlayHintQueryError;
use crate::navigation::NavigationQueryError;
use crate::rename::RenameQueryError;
use crate::semantic_tokens::SemanticTokensQueryError;
use crate::signature::SignatureQueryError;
use crate::{DiagnosticPublicationError, DocumentWorkspaceError, WorkspaceConfigurationError};

#[derive(Debug)]
pub enum ServerIssue {
    Parameters(ParameterError),
    Completion(CompletionQueryError),
    Documents(DocumentWorkspaceError),
    Diagnostics(DiagnosticPublicationError),
    Hover(HoverQueryError),
    InlayHints(InlayHintQueryError),
    SemanticTokens(SemanticTokensQueryError),
    Navigation(NavigationQueryError),
    Rename(RenameQueryError),
    Signature(SignatureQueryError),
    Outbound(OutboundRequestError),
    ClientResponse(ClientResponseError),
    Workspace(WorkspaceConfigurationError),
    Lifecycle(LifecycleTransitionError),
}

impl fmt::Display for ServerIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parameters(error) => error.fmt(formatter),
            Self::Completion(error) => error.fmt(formatter),
            Self::Documents(error) => error.fmt(formatter),
            Self::Diagnostics(error) => error.fmt(formatter),
            Self::Hover(error) => error.fmt(formatter),
            Self::InlayHints(error) => error.fmt(formatter),
            Self::SemanticTokens(error) => error.fmt(formatter),
            Self::Navigation(error) => error.fmt(formatter),
            Self::Rename(error) => error.fmt(formatter),
            Self::Signature(error) => error.fmt(formatter),
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
            Self::Completion(error) => Some(error),
            Self::Documents(error) => Some(error),
            Self::Diagnostics(error) => Some(error),
            Self::Hover(error) => Some(error),
            Self::InlayHints(error) => Some(error),
            Self::SemanticTokens(error) => Some(error),
            Self::Navigation(error) => Some(error),
            Self::Rename(error) => Some(error),
            Self::Signature(error) => Some(error),
            Self::Outbound(error) => Some(error),
            Self::ClientResponse(error) => Some(error),
            Self::Workspace(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
        }
    }
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
pub(super) enum InitializeFailure {
    Parameters(ParameterError),
    Workspace(WorkspaceConfigurationError),
}

impl InitializeFailure {
    pub(super) fn into_server_issue(self) -> Option<ServerIssue> {
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
