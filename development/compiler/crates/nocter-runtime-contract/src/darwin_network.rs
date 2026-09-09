/// The fixed callback-event ABI shared by the Darwin network adapter and executor.
///
/// A serial Network.framework callback queue sends each complete record through an owned
/// `AF_UNIX/SOCK_DGRAM` socketpair. The kernel datagram queue is both the lossless mailbox and the
/// reactor-visible wake source, so no shared queue, lock, allocation, or memory-ordering convention
/// crosses the callback boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinNetworkCallbackEventAbiSchema {
    kind_offset: u64,
    payload_offsets: [u64; 4],
    size: u64,
    alignment: u64,
}

impl DarwinNetworkCallbackEventAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        kind_offset: 0,
        payload_offsets: [8, 16, 24, 32],
        size: 40,
        alignment: 8,
    };

    #[must_use]
    pub const fn kind_offset(self) -> u64 {
        self.kind_offset
    }

    #[must_use]
    pub const fn payload_offset(self, lane: usize) -> Option<u64> {
        if lane < self.payload_offsets.len() {
            Some(self.payload_offsets[lane])
        } else {
            None
        }
    }

    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }
}

/// One provider event retained in callback order until the Nocter executor consumes it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkEventKind {
    ConnectionState,
    ReceiveCompletion,
    SendCompletion,
    ListenerState,
    AcceptedConnection,
}

/// The ownership and representation of one event payload lane.
///
/// A retained object crosses the callback channel at +1. A consumer must either transfer that
/// ownership into a higher-level value or release it with the matching runtime family.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkEventPayload {
    Empty,
    Word,
    OptionalRetainedNetworkObject,
    RetainedNetworkObject,
    OptionalRetainedDispatchData,
}

impl DarwinNetworkEventKind {
    #[must_use]
    pub const fn code(self) -> u64 {
        match self {
            Self::ConnectionState => 0,
            Self::ReceiveCompletion => 1,
            Self::SendCompletion => 2,
            Self::ListenerState => 3,
            Self::AcceptedConnection => 4,
        }
    }

    #[must_use]
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::ConnectionState),
            1 => Some(Self::ReceiveCompletion),
            2 => Some(Self::SendCompletion),
            3 => Some(Self::ListenerState),
            4 => Some(Self::AcceptedConnection),
            _ => None,
        }
    }

    #[must_use]
    pub const fn payload(self, lane: usize) -> Option<DarwinNetworkEventPayload> {
        let payloads = match self {
            Self::ConnectionState | Self::ListenerState => [
                DarwinNetworkEventPayload::Word,
                DarwinNetworkEventPayload::OptionalRetainedNetworkObject,
                DarwinNetworkEventPayload::Empty,
                DarwinNetworkEventPayload::Empty,
            ],
            Self::ReceiveCompletion => [
                DarwinNetworkEventPayload::OptionalRetainedDispatchData,
                DarwinNetworkEventPayload::OptionalRetainedNetworkObject,
                DarwinNetworkEventPayload::Word,
                DarwinNetworkEventPayload::OptionalRetainedNetworkObject,
            ],
            Self::SendCompletion => [
                DarwinNetworkEventPayload::OptionalRetainedNetworkObject,
                DarwinNetworkEventPayload::Empty,
                DarwinNetworkEventPayload::Empty,
                DarwinNetworkEventPayload::Empty,
            ],
            Self::AcceptedConnection => [
                DarwinNetworkEventPayload::RetainedNetworkObject,
                DarwinNetworkEventPayload::Empty,
                DarwinNetworkEventPayload::Empty,
                DarwinNetworkEventPayload::Empty,
            ],
        };
        if lane < payloads.len() {
            Some(payloads[lane])
        } else {
            None
        }
    }
}

/// Network.framework connection states normalized at the provider boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkConnectionState {
    Invalid,
    Waiting,
    Preparing,
    Ready,
    Failed,
    Cancelled,
}

impl DarwinNetworkConnectionState {
    #[must_use]
    pub const fn code(self) -> u64 {
        match self {
            Self::Invalid => 0,
            Self::Waiting => 1,
            Self::Preparing => 2,
            Self::Ready => 3,
            Self::Failed => 4,
            Self::Cancelled => 5,
        }
    }

    #[must_use]
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Invalid),
            1 => Some(Self::Waiting),
            2 => Some(Self::Preparing),
            3 => Some(Self::Ready),
            4 => Some(Self::Failed),
            5 => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// Whether this is the last provider callback state for a cancelled connection.
    ///
    /// Receipt alone does not permit release: the consumer must complete a barrier on the same
    /// serial dispatch queue so the callback that sent the event has returned.
    #[must_use]
    pub const fn is_final_callback_state(self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

/// Network.framework listener states normalized at the provider boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkListenerState {
    Invalid,
    Waiting,
    Ready,
    Failed,
    Cancelled,
}

impl DarwinNetworkListenerState {
    #[must_use]
    pub const fn code(self) -> u64 {
        match self {
            Self::Invalid => 0,
            Self::Waiting => 1,
            Self::Ready => 2,
            Self::Failed => 3,
            Self::Cancelled => 4,
        }
    }

    #[must_use]
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Invalid),
            1 => Some(Self::Waiting),
            2 => Some(Self::Ready),
            3 => Some(Self::Failed),
            4 => Some(Self::Cancelled),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_final_callback_state(self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

/// The two-step release fence for one cancelled Network.framework owner.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkReleaseFence {
    AwaitingFinalState,
    AwaitingDispatchBarrier,
    Releasable,
}

impl DarwinNetworkReleaseFence {
    #[must_use]
    pub const fn observe_connection_state(self, state: DarwinNetworkConnectionState) -> Self {
        if state.is_final_callback_state() {
            match self {
                Self::AwaitingFinalState => Self::AwaitingDispatchBarrier,
                Self::AwaitingDispatchBarrier | Self::Releasable => self,
            }
        } else {
            self
        }
    }

    #[must_use]
    pub const fn observe_listener_state(self, state: DarwinNetworkListenerState) -> Self {
        if state.is_final_callback_state() {
            match self {
                Self::AwaitingFinalState => Self::AwaitingDispatchBarrier,
                Self::AwaitingDispatchBarrier | Self::Releasable => self,
            }
        } else {
            self
        }
    }

    #[must_use]
    pub const fn complete_dispatch_barrier(self) -> Self {
        match self {
            Self::AwaitingDispatchBarrier => Self::Releasable,
            Self::AwaitingFinalState | Self::Releasable => self,
        }
    }

    #[must_use]
    pub const fn can_release(self) -> bool {
        matches!(self, Self::Releasable)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DarwinNetworkCallbackEventAbiSchema, DarwinNetworkConnectionState, DarwinNetworkEventKind,
        DarwinNetworkEventPayload, DarwinNetworkListenerState, DarwinNetworkReleaseFence,
    };

    #[test]
    fn callback_event_has_one_dense_fixed_record_layout() {
        let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
        assert_eq!(schema.kind_offset(), 0);
        assert_eq!(schema.payload_offset(0), Some(8));
        assert_eq!(schema.payload_offset(3), Some(32));
        assert_eq!(schema.payload_offset(4), None);
        assert_eq!(schema.size(), 40);
        assert_eq!(schema.alignment(), 8);
    }

    #[test]
    fn provider_codes_are_total_only_over_the_closed_domains() {
        for kind in [
            DarwinNetworkEventKind::ConnectionState,
            DarwinNetworkEventKind::ReceiveCompletion,
            DarwinNetworkEventKind::SendCompletion,
            DarwinNetworkEventKind::ListenerState,
            DarwinNetworkEventKind::AcceptedConnection,
        ] {
            assert_eq!(DarwinNetworkEventKind::from_code(kind.code()), Some(kind));
        }
        assert_eq!(DarwinNetworkEventKind::from_code(5), None);
        assert_eq!(
            DarwinNetworkEventKind::ReceiveCompletion.payload(0),
            Some(DarwinNetworkEventPayload::OptionalRetainedDispatchData)
        );
        assert_eq!(
            DarwinNetworkEventKind::AcceptedConnection.payload(0),
            Some(DarwinNetworkEventPayload::RetainedNetworkObject)
        );
        assert_eq!(DarwinNetworkEventKind::SendCompletion.payload(4), None);

        for state in [
            DarwinNetworkConnectionState::Invalid,
            DarwinNetworkConnectionState::Waiting,
            DarwinNetworkConnectionState::Preparing,
            DarwinNetworkConnectionState::Ready,
            DarwinNetworkConnectionState::Failed,
            DarwinNetworkConnectionState::Cancelled,
        ] {
            assert_eq!(
                DarwinNetworkConnectionState::from_code(state.code()),
                Some(state)
            );
        }
        assert_eq!(DarwinNetworkConnectionState::from_code(6), None);
        assert!(DarwinNetworkConnectionState::Cancelled.is_final_callback_state());

        for state in [
            DarwinNetworkListenerState::Invalid,
            DarwinNetworkListenerState::Waiting,
            DarwinNetworkListenerState::Ready,
            DarwinNetworkListenerState::Failed,
            DarwinNetworkListenerState::Cancelled,
        ] {
            assert_eq!(
                DarwinNetworkListenerState::from_code(state.code()),
                Some(state)
            );
        }
        assert_eq!(DarwinNetworkListenerState::from_code(5), None);
        assert!(DarwinNetworkListenerState::Cancelled.is_final_callback_state());

        let fence = DarwinNetworkReleaseFence::AwaitingFinalState;
        assert!(!fence.complete_dispatch_barrier().can_release());
        assert_eq!(
            fence.observe_connection_state(DarwinNetworkConnectionState::Ready),
            fence
        );
        let fence = fence.observe_connection_state(DarwinNetworkConnectionState::Cancelled);
        assert!(!fence.can_release());
        assert!(fence.complete_dispatch_barrier().can_release());

        let listener_fence = DarwinNetworkReleaseFence::AwaitingFinalState
            .observe_listener_state(DarwinNetworkListenerState::Cancelled);
        assert!(listener_fence.complete_dispatch_barrier().can_release());
    }
}
