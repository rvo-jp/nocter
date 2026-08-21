//! Language Server Protocol framing, JSON-RPC messages, and lifecycle validation.
//!
//! This crate contains no compiler semantics and performs no filesystem access. Validated
//! messages cross this boundary before they may mutate editor documents or invoke analysis.

mod decode;
mod initialize;
mod lifecycle;
mod message;
mod parameters;
mod response;
mod session;
mod transport;
mod uri;

pub use initialize::{InitializeParams, WorkspaceFolder, initialize_result};
pub use lifecycle::{Lifecycle, LifecycleAction, LifecycleState};
pub use message::{IncomingMessage, MessageDecodeError, MessageDecodeErrorKind, RequestId};
pub use parameters::{
    DidChangeParams, DidCloseParams, DidOpenParams, DidSaveParams, ParameterError,
    ParameterErrorKind,
};
pub use response::{ResponseErrorCode, render_error_response, render_success_response};
pub use session::{ProtocolEvent, ProtocolReception, ProtocolSession};
pub use transport::{FrameError, FrameReader, write_frame};
pub use uri::{DocumentUri, DocumentUriError, DocumentUriErrorKind};
