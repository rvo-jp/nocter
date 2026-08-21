//! Language Server Protocol framing, JSON-RPC messages, and lifecycle validation.
//!
//! This crate contains no compiler semantics and performs no filesystem access. Validated
//! messages cross this boundary before they may mutate editor documents or invoke analysis.

mod decode;
mod hover;
mod initialize;
mod lifecycle;
mod message;
mod outbound;
mod parameters;
mod response;
mod session;
mod transport;
mod uri;
mod watcher;

pub use hover::{HoverParams, Position, Range, hover_result};
pub use initialize::{InitializeParams, WorkspaceFolder, initialize_result};
pub use lifecycle::{Lifecycle, LifecycleAction, LifecycleState, LifecycleTransitionError};
pub use message::{
    IncomingMessage, MessageDecodeError, MessageDecodeErrorKind, RequestId, ResponseError,
    ResponseResult,
};
pub use outbound::{CompletedRequest, OutboundRequest, OutboundRequestError, OutboundRequests};
pub use parameters::{
    DidChangeParams, DidCloseParams, DidOpenParams, DidSaveParams, ParameterError,
    ParameterErrorKind,
};
pub use response::{
    ResponseErrorCode, render_error_response, render_notification, render_request,
    render_success_response,
};
pub use session::{ProtocolEvent, ProtocolReception, ProtocolSession};
pub use transport::{FrameError, FrameReader, write_frame};
pub use uri::{DocumentUri, DocumentUriError, DocumentUriErrorKind};
pub use watcher::{
    DidChangeWatchedFilesParams, WATCHED_FILES_REGISTRATION_ID, WatchedFileChange,
    WatchedFileChangeKind, watched_files_registration,
};
