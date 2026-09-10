use crate::{DarwinNetworkConnectionState, DarwinNetworkEventKind, DarwinNetworkListenerState};

/// The two native owner families admitted by the closed Network.framework adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkOwnerKind {
    Connection,
    Listener,
}

/// Compiler-owned lifecycle state for one native network owner.
///
/// Provider states remain event payloads. This state instead records the facts needed to decide
/// whether another adapter operation is legal and whether native storage may be released.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkOwnerState {
    Initialized,
    Running,
    CancelRequested,
    FinalStateObserved,
    Quiesced,
    Released,
}

impl DarwinNetworkOwnerState {
    pub const ALL: &'static [Self] = &[
        Self::Initialized,
        Self::Running,
        Self::CancelRequested,
        Self::FinalStateObserved,
        Self::Quiesced,
        Self::Released,
    ];

    #[must_use]
    pub const fn code(self) -> u64 {
        match self {
            Self::Initialized => 0,
            Self::Running => 1,
            Self::CancelRequested => 2,
            Self::FinalStateObserved => 3,
            Self::Quiesced => 4,
            Self::Released => 5,
        }
    }

    #[must_use]
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Initialized),
            1 => Some(Self::Running),
            2 => Some(Self::CancelRequested),
            3 => Some(Self::FinalStateObserved),
            4 => Some(Self::Quiesced),
            5 => Some(Self::Released),
            _ => None,
        }
    }
}

/// Operations admitted by the compiler-owned Network.framework adapter.
///
/// This is deliberately smaller than the provider API. Source code cannot set handlers, select a
/// queue, manipulate Blocks, or independently release captured objects.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkAdapterOperation {
    CreateOutboundConnection,
    CreateSecureOutboundConnection,
    CreateListener,
    AdoptAcceptedConnection,
    Start,
    EventDescriptor,
    BeginReceive,
    BeginSend,
    CopyAddress,
    ListenerPort,
    ReceiveEvent,
    TryReceiveEvent,
    RequestCancel,
    ObserveFinalState,
    CompleteReleaseBarrier,
    Release,
    Dispose,
}

impl DarwinNetworkAdapterOperation {
    pub const ALL: &'static [Self] = &[
        Self::CreateOutboundConnection,
        Self::CreateSecureOutboundConnection,
        Self::CreateListener,
        Self::AdoptAcceptedConnection,
        Self::Start,
        Self::EventDescriptor,
        Self::BeginReceive,
        Self::BeginSend,
        Self::CopyAddress,
        Self::ListenerPort,
        Self::ReceiveEvent,
        Self::TryReceiveEvent,
        Self::RequestCancel,
        Self::ObserveFinalState,
        Self::CompleteReleaseBarrier,
        Self::Release,
        Self::Dispose,
    ];

    /// Returns the owner created by an operation with no prior owner.
    #[must_use]
    pub const fn created_owner(self) -> Option<DarwinNetworkOwnerKind> {
        match self {
            Self::CreateOutboundConnection
            | Self::CreateSecureOutboundConnection
            | Self::AdoptAcceptedConnection => Some(DarwinNetworkOwnerKind::Connection),
            Self::CreateListener => Some(DarwinNetworkOwnerKind::Listener),
            Self::Start
            | Self::EventDescriptor
            | Self::BeginReceive
            | Self::BeginSend
            | Self::CopyAddress
            | Self::ListenerPort
            | Self::ReceiveEvent
            | Self::TryReceiveEvent
            | Self::RequestCancel
            | Self::ObserveFinalState
            | Self::CompleteReleaseBarrier
            | Self::Release
            | Self::Dispose => None,
        }
    }

    /// Computes the deterministic lifecycle transition for an existing owner model.
    ///
    /// Event receipt remains separate: only the typed provider-state observers may select
    /// [`Self::ObserveFinalState`].
    ///
    /// # Errors
    ///
    /// Returns the same closed transition error as [`DarwinNetworkOwner::apply`].
    pub const fn transition(
        self,
        kind: DarwinNetworkOwnerKind,
        state: DarwinNetworkOwnerState,
    ) -> Result<DarwinNetworkOwnerState, DarwinNetworkOperationError> {
        match (DarwinNetworkOwner { kind, state }).apply(self) {
            Ok(owner) => Ok(owner.state()),
            Err(error) => Err(error),
        }
    }
}

/// One checked adapter owner state.
///
/// Keeping kind and lifecycle together prevents a caller from applying a connection operation to
/// a listener while separately claiming that its lifecycle was valid.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DarwinNetworkOwner {
    kind: DarwinNetworkOwnerKind,
    state: DarwinNetworkOwnerState,
}

impl DarwinNetworkOwner {
    /// Creates the exact owner family assigned by one ownerless adapter operation.
    ///
    /// # Errors
    ///
    /// Rejects operations that require an existing owner.
    pub const fn create(
        operation: DarwinNetworkAdapterOperation,
    ) -> Result<Self, DarwinNetworkOperationError> {
        let Some(kind) = operation.created_owner() else {
            return Err(DarwinNetworkOperationError::RequiresExistingOwner(
                operation,
            ));
        };
        Ok(Self::new(kind))
    }

    #[must_use]
    const fn new(kind: DarwinNetworkOwnerKind) -> Self {
        Self {
            kind,
            state: DarwinNetworkOwnerState::Initialized,
        }
    }

    #[must_use]
    pub const fn kind(self) -> DarwinNetworkOwnerKind {
        self.kind
    }

    #[must_use]
    pub const fn state(self) -> DarwinNetworkOwnerState {
        self.state
    }

    /// Applies an operation whose lifecycle result is deterministic.
    ///
    /// Event observation is handled by [`Self::observe_event`] because only a final cancellation
    /// event advances the release fence.
    ///
    /// # Errors
    ///
    /// Rejects creation operations, event observation, owner-kind mismatches, and invalid state
    /// transitions.
    pub const fn apply(
        self,
        operation: DarwinNetworkAdapterOperation,
    ) -> Result<Self, DarwinNetworkOperationError> {
        use DarwinNetworkAdapterOperation as Operation;
        use DarwinNetworkOwnerKind as Kind;
        use DarwinNetworkOwnerState as State;

        let state = match (operation, self.kind, self.state) {
            (Operation::Start, _, State::Initialized) => State::Running,
            (
                Operation::EventDescriptor,
                _,
                State::Initialized
                | State::Running
                | State::CancelRequested
                | State::FinalStateObserved
                | State::Quiesced,
            )
            | (Operation::ReceiveEvent, _, State::Running | State::CancelRequested)
            | (
                Operation::TryReceiveEvent | Operation::Dispose,
                _,
                State::Initialized | State::Running | State::CancelRequested,
            )
            | (Operation::CopyAddress, Kind::Connection, State::Running)
            | (Operation::ListenerPort, Kind::Listener, State::Running) => self.state,
            (Operation::BeginReceive | Operation::BeginSend, Kind::Connection, State::Running) => {
                State::Running
            }
            (
                Operation::RequestCancel,
                _,
                State::Initialized | State::Running | State::CancelRequested,
            ) => State::CancelRequested,
            (Operation::ObserveFinalState, _, State::CancelRequested) => State::FinalStateObserved,
            (Operation::CompleteReleaseBarrier, _, State::FinalStateObserved) => State::Quiesced,
            (Operation::Release, _, State::Quiesced) => State::Released,
            (
                Operation::CreateOutboundConnection
                | Operation::CreateSecureOutboundConnection
                | Operation::CreateListener
                | Operation::AdoptAcceptedConnection,
                _,
                _,
            ) => return Err(DarwinNetworkOperationError::RequiresCreation(operation)),
            _ => {
                return Err(DarwinNetworkOperationError::InvalidState {
                    operation,
                    kind: self.kind,
                    state: self.state,
                });
            }
        };
        Ok(Self {
            kind: self.kind,
            state,
        })
    }

    /// Applies one non-state event already transferred through the fixed callback channel.
    ///
    /// Completion events leave the lifecycle unchanged. Provider state events must use the typed
    /// state methods so a caller cannot independently claim that an arbitrary event was final.
    ///
    /// # Errors
    ///
    /// Rejects state events, events belonging to the other owner family, and events before start
    /// or after the release fence.
    pub const fn observe_event(
        self,
        event: DarwinNetworkEventKind,
    ) -> Result<Self, DarwinNetworkOperationError> {
        use DarwinNetworkEventKind as Event;
        use DarwinNetworkOwnerKind as Kind;
        use DarwinNetworkOwnerState as State;

        let belongs_to_owner = matches!(
            (self.kind, event),
            (
                Kind::Connection,
                Event::ReceiveCompletion | Event::SendCompletion
            ) | (Kind::Listener, Event::AcceptedConnection)
        );
        if !belongs_to_owner {
            return Err(DarwinNetworkOperationError::WrongEvent {
                kind: self.kind,
                event,
            });
        }
        if !matches!(self.state, State::Running | State::CancelRequested) {
            return Err(DarwinNetworkOperationError::InvalidState {
                operation: DarwinNetworkAdapterOperation::ReceiveEvent,
                kind: self.kind,
                state: self.state,
            });
        }
        Ok(self)
    }

    /// Applies a typed connection-state event and derives finality from the provider state.
    ///
    /// # Errors
    ///
    /// Rejects a listener owner, an event outside the running/cancelling interval, or a final
    /// cancelled state for which this owner did not request cancellation.
    pub const fn observe_connection_state(
        self,
        state: DarwinNetworkConnectionState,
    ) -> Result<Self, DarwinNetworkOperationError> {
        if !matches!(self.kind, DarwinNetworkOwnerKind::Connection) {
            return Err(DarwinNetworkOperationError::WrongEvent {
                kind: self.kind,
                event: DarwinNetworkEventKind::ConnectionState,
            });
        }
        self.observe_provider_state(
            DarwinNetworkEventKind::ConnectionState,
            state.is_final_callback_state(),
        )
    }

    /// Applies a typed listener-state event and derives finality from the provider state.
    ///
    /// # Errors
    ///
    /// Rejects a connection owner, an event outside the running/cancelling interval, or a final
    /// cancelled state for which this owner did not request cancellation.
    pub const fn observe_listener_state(
        self,
        state: DarwinNetworkListenerState,
    ) -> Result<Self, DarwinNetworkOperationError> {
        if !matches!(self.kind, DarwinNetworkOwnerKind::Listener) {
            return Err(DarwinNetworkOperationError::WrongEvent {
                kind: self.kind,
                event: DarwinNetworkEventKind::ListenerState,
            });
        }
        self.observe_provider_state(
            DarwinNetworkEventKind::ListenerState,
            state.is_final_callback_state(),
        )
    }

    const fn observe_provider_state(
        self,
        event: DarwinNetworkEventKind,
        is_final: bool,
    ) -> Result<Self, DarwinNetworkOperationError> {
        use DarwinNetworkOwnerState as State;

        if !matches!(self.state, State::Running | State::CancelRequested) {
            return Err(DarwinNetworkOperationError::InvalidState {
                operation: DarwinNetworkAdapterOperation::ReceiveEvent,
                kind: self.kind,
                state: self.state,
            });
        }
        if !is_final {
            return Ok(self);
        }
        if !matches!(self.state, State::CancelRequested) {
            return Err(DarwinNetworkOperationError::UnexpectedFinalState {
                kind: self.kind,
                state: self.state,
                event,
            });
        }
        self.apply(DarwinNetworkAdapterOperation::ObserveFinalState)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinNetworkOperationError {
    RequiresExistingOwner(DarwinNetworkAdapterOperation),
    RequiresCreation(DarwinNetworkAdapterOperation),
    InvalidState {
        operation: DarwinNetworkAdapterOperation,
        kind: DarwinNetworkOwnerKind,
        state: DarwinNetworkOwnerState,
    },
    WrongEvent {
        kind: DarwinNetworkOwnerKind,
        event: DarwinNetworkEventKind,
    },
    UnexpectedFinalState {
        kind: DarwinNetworkOwnerKind,
        state: DarwinNetworkOwnerState,
        event: DarwinNetworkEventKind,
    },
}

impl std::fmt::Display for DarwinNetworkOperationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Darwin network adapter operation: {self:?}"
        )
    }
}

impl std::error::Error for DarwinNetworkOperationError {}

#[cfg(test)]
mod tests {
    use super::{
        DarwinNetworkAdapterOperation as Operation, DarwinNetworkOperationError,
        DarwinNetworkOwner, DarwinNetworkOwnerKind as Kind, DarwinNetworkOwnerState as State,
    };
    use crate::{
        DarwinNetworkConnectionState as ConnectionState, DarwinNetworkEventKind as Event,
        DarwinNetworkListenerState as ListenerState,
    };

    #[test]
    fn creation_is_the_only_ownerless_operation() {
        assert_eq!(
            Operation::CreateOutboundConnection.created_owner(),
            Some(Kind::Connection)
        );
        assert_eq!(
            Operation::AdoptAcceptedConnection.created_owner(),
            Some(Kind::Connection)
        );
        assert_eq!(
            Operation::CreateListener.created_owner(),
            Some(Kind::Listener)
        );
        assert!(
            Operation::ALL
                .iter()
                .copied()
                .filter(|operation| operation.created_owner().is_some())
                .eq([
                    Operation::CreateOutboundConnection,
                    Operation::CreateSecureOutboundConnection,
                    Operation::CreateListener,
                    Operation::AdoptAcceptedConnection,
                ])
        );
        assert_eq!(
            DarwinNetworkOwner::create(Operation::Start),
            Err(DarwinNetworkOperationError::RequiresExistingOwner(
                Operation::Start
            ))
        );
        let owner = DarwinNetworkOwner::create(Operation::CreateListener).unwrap();
        assert_eq!(
            owner.apply(Operation::CreateOutboundConnection),
            Err(DarwinNetworkOperationError::RequiresCreation(
                Operation::CreateOutboundConnection
            ))
        );
    }

    #[test]
    fn owner_state_tags_are_closed_and_round_trip() {
        for state in State::ALL.iter().copied() {
            assert_eq!(State::from_code(state.code()), Some(state));
        }
        assert_eq!(State::from_code(6), None);
    }

    #[test]
    fn connection_release_requires_cancel_event_then_barrier() {
        let owner = DarwinNetworkOwner::create(Operation::CreateOutboundConnection)
            .unwrap()
            .apply(Operation::Start)
            .unwrap();
        assert_eq!(owner.state(), State::Running);
        assert_eq!(owner.apply(Operation::BeginReceive).unwrap(), owner);
        assert_eq!(owner.apply(Operation::BeginSend).unwrap(), owner);
        assert_eq!(owner.apply(Operation::ReceiveEvent).unwrap(), owner);
        assert_eq!(
            owner.observe_event(Event::ReceiveCompletion).unwrap(),
            owner
        );
        assert!(matches!(
            owner.apply(Operation::Release),
            Err(DarwinNetworkOperationError::InvalidState { .. })
        ));

        let owner = owner.apply(Operation::RequestCancel).unwrap();
        let owner = owner
            .observe_connection_state(ConnectionState::Cancelled)
            .unwrap();
        assert_eq!(owner.state(), State::FinalStateObserved);
        let owner = owner.apply(Operation::CompleteReleaseBarrier).unwrap();
        assert_eq!(owner.state(), State::Quiesced);
        assert_eq!(
            owner.apply(Operation::Release).unwrap().state(),
            State::Released
        );
    }

    #[test]
    fn listener_and_connection_events_cannot_cross_owner_families() {
        let listener = DarwinNetworkOwner::create(Operation::CreateListener)
            .unwrap()
            .apply(Operation::Start)
            .unwrap();
        assert!(matches!(
            listener.observe_event(Event::ConnectionState),
            Err(DarwinNetworkOperationError::WrongEvent { .. })
        ));
        assert!(matches!(
            listener.apply(Operation::BeginReceive),
            Err(DarwinNetworkOperationError::InvalidState { .. })
        ));
        assert_eq!(
            listener.observe_event(Event::AcceptedConnection).unwrap(),
            listener
        );
        assert_eq!(listener.apply(Operation::ListenerPort).unwrap(), listener);
        let connection = DarwinNetworkOwner::create(Operation::CreateOutboundConnection)
            .unwrap()
            .apply(Operation::Start)
            .unwrap();
        assert!(matches!(
            connection.apply(Operation::ListenerPort),
            Err(DarwinNetworkOperationError::InvalidState { .. })
        ));
    }

    #[test]
    fn a_final_event_cannot_manufacture_release_authority() {
        let connection = DarwinNetworkOwner::create(Operation::AdoptAcceptedConnection)
            .unwrap()
            .apply(Operation::Start)
            .unwrap();
        assert!(matches!(
            connection.observe_connection_state(ConnectionState::Cancelled),
            Err(DarwinNetworkOperationError::UnexpectedFinalState { .. })
        ));
        let cancelled = connection.apply(Operation::RequestCancel).unwrap();
        assert!(matches!(
            cancelled.observe_listener_state(ListenerState::Cancelled),
            Err(DarwinNetworkOperationError::WrongEvent { .. })
        ));
    }
}
