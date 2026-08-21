use nocter_json::Value;

use crate::{
    IncomingMessage, Lifecycle, LifecycleAction, LifecycleState, MessageDecodeErrorKind, RequestId,
    ResponseErrorCode, render_error_response,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ProtocolEvent {
    Initialize {
        id: RequestId,
        params: Option<Value>,
    },
    Initialized,
    Message(IncomingMessage),
    Shutdown {
        id: RequestId,
    },
    Exit {
        clean: bool,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProtocolReception {
    response: Option<String>,
    event: Option<ProtocolEvent>,
}

impl ProtocolReception {
    #[must_use]
    pub fn response(&self) -> Option<&str> {
        self.response.as_deref()
    }

    #[must_use]
    pub const fn event(&self) -> Option<&ProtocolEvent> {
        self.event.as_ref()
    }

    #[must_use]
    pub fn into_parts(self) -> (Option<String>, Option<ProtocolEvent>) {
        (self.response, self.event)
    }
}

/// Composes JSON-RPC envelope decoding with the LSP lifecycle gate.
#[derive(Debug, Default)]
pub struct ProtocolSession {
    lifecycle: Lifecycle,
}

impl ProtocolSession {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lifecycle: Lifecycle::new(),
        }
    }

    #[must_use]
    pub const fn state(&self) -> LifecycleState {
        self.lifecycle.state()
    }

    /// Commits or rejects the parameter validation of the pending initialize event.
    ///
    /// # Errors
    ///
    /// Returns an error when the session has no pending initialize event.
    pub fn complete_initialize(
        &mut self,
        accepted: bool,
    ) -> Result<(), crate::LifecycleTransitionError> {
        self.lifecycle.complete_initialize(accepted)
    }

    /// Validates one JSON body and returns either one immediate error response or one typed event.
    ///
    /// Notifications that are invalid for the current lifecycle intentionally produce neither.
    pub fn receive(&mut self, body: &str) -> ProtocolReception {
        let message = match IncomingMessage::decode(body) {
            Ok(message) => message,
            Err(error) => {
                let code = if error.kind() == MessageDecodeErrorKind::Json {
                    ResponseErrorCode::ParseError
                } else {
                    ResponseErrorCode::InvalidRequest
                };
                let detail = Value::String(error.to_string().into_boxed_str());
                return ProtocolReception {
                    response: Some(render_error_response(None, code, Some(&detail))),
                    event: None,
                };
            }
        };

        match self.lifecycle.accept(message) {
            LifecycleAction::Initialize { id, params } => {
                event(ProtocolEvent::Initialize { id, params })
            }
            LifecycleAction::Initialized => event(ProtocolEvent::Initialized),
            LifecycleAction::Dispatch(message) => event(ProtocolEvent::Message(message)),
            LifecycleAction::Shutdown { id } => event(ProtocolEvent::Shutdown { id }),
            LifecycleAction::Exit { clean } => event(ProtocolEvent::Exit { clean }),
            LifecycleAction::Reject { id, code } => ProtocolReception {
                response: Some(render_error_response(Some(&id), code, None)),
                event: None,
            },
            LifecycleAction::IgnoreNotification | LifecycleAction::IgnoreAfterExit => {
                ProtocolReception::default()
            }
        }
    }
}

fn event(event: ProtocolEvent) -> ProtocolReception {
    ProtocolReception {
        response: None,
        event: Some(event),
    }
}

#[cfg(test)]
mod tests {
    use nocter_json::parse;

    use super::*;

    #[test]
    fn separates_protocol_errors_from_validated_lifecycle_events() {
        let mut session = ProtocolSession::new();
        let malformed = session.receive("{");
        assert!(malformed.event().is_none());
        assert!(malformed.response().unwrap().contains("\"code\":-32700"));

        let invalid = session.receive(r#"{"jsonrpc":"2.0","id":null,"method":"initialize"}"#);
        assert!(invalid.event().is_none());
        assert!(invalid.response().unwrap().contains("\"code\":-32600"));

        let initialize =
            session.receive(r#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{}}"#);
        assert!(matches!(
            initialize.event(),
            Some(ProtocolEvent::Initialize {
                id: RequestId::String(id),
                ..
            }) if id.as_ref() == "init"
        ));
        assert_eq!(session.state(), LifecycleState::Initializing);
        session.complete_initialize(true).unwrap();
        assert_eq!(session.state(), LifecycleState::AwaitingInitialized);
    }

    #[test]
    fn emits_immediate_lifecycle_rejections_with_the_request_id() {
        let mut session = ProtocolSession::new();
        let reception = session
            .receive(r#"{"jsonrpc":"2.0","id":9,"method":"textDocument/hover","params":{}}"#);
        assert_eq!(
            reception.response(),
            Some(
                r#"{"jsonrpc":"2.0","id":9,"error":{"code":-32002,"message":"Server not initialized"}}"#
            )
        );
        assert!(reception.event().is_none());
    }

    #[test]
    fn reaches_clean_exit_only_after_shutdown() {
        let mut session = ProtocolSession::new();
        session.receive(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
        session.complete_initialize(true).unwrap();
        session.receive(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#);
        let shutdown = session.receive(r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#);
        assert_eq!(
            shutdown.event(),
            Some(&ProtocolEvent::Shutdown {
                id: RequestId::Integer(2)
            })
        );
        let exit = session.receive(r#"{"jsonrpc":"2.0","method":"exit"}"#);
        assert_eq!(exit.event(), Some(&ProtocolEvent::Exit { clean: true }));
    }

    #[test]
    fn retained_error_data_is_valid_json() {
        let mut session = ProtocolSession::new();
        let reception = session.receive("not-json");
        assert!(parse(reception.response().unwrap()).is_ok());
    }
}
