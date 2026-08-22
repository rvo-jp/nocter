//! Language Server Protocol framing, JSON-RPC messages, and lifecycle validation.
//!
//! This crate contains no compiler semantics and performs no filesystem access. Validated
//! messages cross this boundary before they may mutate editor documents or invoke analysis.

mod code_actions;
mod completion;
mod coordinates;
mod decode;
mod hover;
mod initialize;
mod inlay_hints;
mod lifecycle;
mod message;
mod navigation;
mod outbound;
mod parameters;
mod rename;
mod response;
mod semantic_tokens;
mod session;
mod signature;
mod text_edit;
mod transport;
mod uri;
mod watcher;

pub use code_actions::{CodeAction, CodeActionParams, code_actions_result};
pub use completion::{CompletionItem, CompletionItemKind, CompletionParams, completion_result};
pub use coordinates::{Position, Range, TextDocumentPositionParams};
pub use hover::{HoverParams, hover_result};
pub use initialize::{InitializeParams, WorkspaceFolder, initialize_result};
pub use inlay_hints::{InlayHint, InlayHintKind, InlayHintParams, inlay_hints_result};
pub use lifecycle::{Lifecycle, LifecycleAction, LifecycleState, LifecycleTransitionError};
pub use message::{
    IncomingMessage, MessageDecodeError, MessageDecodeErrorKind, RequestId, ResponseError,
    ResponseResult,
};
pub use navigation::{DefinitionParams, Location, ReferencesParams, locations_result};
pub use outbound::{CompletedRequest, OutboundRequest, OutboundRequestError, OutboundRequests};
pub use parameters::{
    DidChangeParams, DidCloseParams, DidOpenParams, DidSaveParams, ParameterError,
    ParameterErrorKind,
};
pub use rename::{DocumentEdit, RenameParams, workspace_edit_result};
pub use response::{
    ResponseErrorCode, render_error_response, render_notification, render_request,
    render_success_response,
};
pub use semantic_tokens::{
    SEMANTIC_TOKEN_MODIFIERS, SEMANTIC_TOKEN_TYPES, SemanticToken, SemanticTokenEncodingError,
    SemanticTokenType, SemanticTokensParams, semantic_tokens_result,
};
pub use session::{ProtocolEvent, ProtocolReception, ProtocolSession};
pub use signature::{SignatureHelpParams, SignatureParameter, signature_help_result};
pub use text_edit::TextEdit;
pub use transport::{FrameError, FrameReader, write_frame};
pub use uri::{DocumentUri, DocumentUriError, DocumentUriErrorKind};
pub use watcher::{
    DidChangeWatchedFilesParams, WATCHED_FILES_REGISTRATION_ID, WatchedFileChange,
    WatchedFileChangeKind, watched_files_registration,
};
