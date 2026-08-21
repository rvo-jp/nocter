use nocter_json::Value;

use crate::{IncomingMessage, RequestId};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LifecycleState {
    #[default]
    Uninitialized,
    AwaitingInitialized,
    Running,
    Shutdown,
    Exited,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleErrorCode {
    ServerNotInitialized,
    InvalidRequest,
}

impl LifecycleErrorCode {
    #[must_use]
    pub const fn json_rpc_code(self) -> i32 {
        match self {
            Self::ServerNotInitialized => -32002,
            Self::InvalidRequest => -32600,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LifecycleAction {
    Initialize {
        id: RequestId,
        params: Option<Value>,
    },
    Initialized,
    Dispatch(IncomingMessage),
    Shutdown {
        id: RequestId,
    },
    Exit {
        clean: bool,
    },
    Reject {
        id: RequestId,
        code: LifecycleErrorCode,
    },
    IgnoreNotification,
    IgnoreAfterExit,
}

/// Deterministic LSP lifecycle gate applied before document or analysis state.
#[derive(Debug, Default)]
pub struct Lifecycle {
    state: LifecycleState,
}

impl Lifecycle {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: LifecycleState::Uninitialized,
        }
    }

    #[must_use]
    pub const fn state(&self) -> LifecycleState {
        self.state
    }

    pub fn accept(&mut self, message: IncomingMessage) -> LifecycleAction {
        if self.state == LifecycleState::Exited {
            return LifecycleAction::IgnoreAfterExit;
        }
        if message.method() == "exit" && !message.is_request() {
            let clean = self.state == LifecycleState::Shutdown;
            self.state = LifecycleState::Exited;
            return LifecycleAction::Exit { clean };
        }

        match self.state {
            LifecycleState::Uninitialized => self.accept_uninitialized(message),
            LifecycleState::AwaitingInitialized => self.accept_awaiting_initialized(message),
            LifecycleState::Running => self.accept_running(message),
            LifecycleState::Shutdown => reject_or_ignore(message),
            LifecycleState::Exited => unreachable!("exited state returned above"),
        }
    }

    fn accept_uninitialized(&mut self, message: IncomingMessage) -> LifecycleAction {
        match message {
            IncomingMessage::Request { id, method, params } if method.as_ref() == "initialize" => {
                self.state = LifecycleState::AwaitingInitialized;
                LifecycleAction::Initialize { id, params }
            }
            IncomingMessage::Request { id, .. } => LifecycleAction::Reject {
                id,
                code: LifecycleErrorCode::ServerNotInitialized,
            },
            IncomingMessage::Notification { .. } => LifecycleAction::IgnoreNotification,
        }
    }

    fn accept_awaiting_initialized(&mut self, message: IncomingMessage) -> LifecycleAction {
        match message {
            IncomingMessage::Notification { method, .. } if method.as_ref() == "initialized" => {
                self.state = LifecycleState::Running;
                LifecycleAction::Initialized
            }
            IncomingMessage::Request { id, method, .. } if method.as_ref() == "initialize" => {
                LifecycleAction::Reject {
                    id,
                    code: LifecycleErrorCode::InvalidRequest,
                }
            }
            IncomingMessage::Request { id, .. } => LifecycleAction::Reject {
                id,
                code: LifecycleErrorCode::ServerNotInitialized,
            },
            IncomingMessage::Notification { .. } => LifecycleAction::IgnoreNotification,
        }
    }

    fn accept_running(&mut self, message: IncomingMessage) -> LifecycleAction {
        match message {
            IncomingMessage::Request { id, method, .. } if method.as_ref() == "initialize" => {
                LifecycleAction::Reject {
                    id,
                    code: LifecycleErrorCode::InvalidRequest,
                }
            }
            IncomingMessage::Request { id, method, .. } if method.as_ref() == "shutdown" => {
                self.state = LifecycleState::Shutdown;
                LifecycleAction::Shutdown { id }
            }
            IncomingMessage::Notification { method, .. }
                if method.as_ref() == "initialize"
                    || method.as_ref() == "initialized"
                    || method.as_ref() == "shutdown" =>
            {
                LifecycleAction::IgnoreNotification
            }
            message => LifecycleAction::Dispatch(message),
        }
    }
}

fn reject_or_ignore(message: IncomingMessage) -> LifecycleAction {
    match message {
        IncomingMessage::Request { id, .. } => LifecycleAction::Reject {
            id,
            code: LifecycleErrorCode::InvalidRequest,
        },
        IncomingMessage::Notification { .. } => LifecycleAction::IgnoreNotification,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: i32, method: &str) -> IncomingMessage {
        IncomingMessage::Request {
            id: RequestId::Integer(id),
            method: method.into(),
            params: None,
        }
    }

    fn notification(method: &str) -> IncomingMessage {
        IncomingMessage::Notification {
            method: method.into(),
            params: None,
        }
    }

    #[test]
    fn accepts_only_the_ordered_initialize_shutdown_exit_sequence() {
        let mut lifecycle = Lifecycle::new();
        assert!(matches!(
            lifecycle.accept(request(1, "initialize")),
            LifecycleAction::Initialize { .. }
        ));
        assert_eq!(lifecycle.state(), LifecycleState::AwaitingInitialized);
        assert_eq!(
            lifecycle.accept(notification("initialized")),
            LifecycleAction::Initialized
        );
        assert_eq!(lifecycle.state(), LifecycleState::Running);
        assert!(matches!(
            lifecycle.accept(request(2, "textDocument/hover")),
            LifecycleAction::Dispatch(_)
        ));
        assert_eq!(
            lifecycle.accept(request(3, "shutdown")),
            LifecycleAction::Shutdown {
                id: RequestId::Integer(3)
            }
        );
        assert_eq!(lifecycle.state(), LifecycleState::Shutdown);
        assert_eq!(
            lifecycle.accept(notification("exit")),
            LifecycleAction::Exit { clean: true }
        );
        assert_eq!(lifecycle.state(), LifecycleState::Exited);
        assert_eq!(
            lifecycle.accept(request(4, "unknown")),
            LifecycleAction::IgnoreAfterExit
        );
    }

    #[test]
    fn rejects_requests_that_cross_lifecycle_boundaries() {
        let mut lifecycle = Lifecycle::new();
        assert_eq!(
            lifecycle.accept(request(1, "hover")),
            LifecycleAction::Reject {
                id: RequestId::Integer(1),
                code: LifecycleErrorCode::ServerNotInitialized,
            }
        );
        lifecycle.accept(request(2, "initialize"));
        assert_eq!(
            lifecycle.accept(request(3, "initialize")),
            LifecycleAction::Reject {
                id: RequestId::Integer(3),
                code: LifecycleErrorCode::InvalidRequest,
            }
        );
        lifecycle.accept(notification("initialized"));
        assert_eq!(
            lifecycle.accept(request(4, "initialize")),
            LifecycleAction::Reject {
                id: RequestId::Integer(4),
                code: LifecycleErrorCode::InvalidRequest,
            }
        );
        lifecycle.accept(request(5, "shutdown"));
        assert_eq!(
            lifecycle.accept(request(6, "hover")),
            LifecycleAction::Reject {
                id: RequestId::Integer(6),
                code: LifecycleErrorCode::InvalidRequest,
            }
        );
    }

    #[test]
    fn exit_without_shutdown_is_unclean() {
        let mut lifecycle = Lifecycle::new();
        assert_eq!(
            lifecycle.accept(notification("exit")),
            LifecycleAction::Exit { clean: false }
        );
    }
}
