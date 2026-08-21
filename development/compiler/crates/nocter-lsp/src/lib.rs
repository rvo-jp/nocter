//! Language Server Protocol framing, JSON-RPC messages, and lifecycle validation.
//!
//! This crate contains no compiler semantics and performs no filesystem access. Validated
//! messages cross this boundary before they may mutate editor documents or invoke analysis.

mod lifecycle;
mod message;
mod transport;

pub use lifecycle::{Lifecycle, LifecycleAction, LifecycleErrorCode, LifecycleState};
pub use message::{IncomingMessage, MessageDecodeError, MessageDecodeErrorKind, RequestId};
pub use transport::{FrameError, FrameReader, write_frame};
