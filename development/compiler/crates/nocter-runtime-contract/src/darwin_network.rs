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

/// Native-object-free words returned after consuming any connection callback event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinNetworkConnectionEventObservationAbiSchema {
    kind_offset: u64,
    value_offsets: [u64; 4],
    size: u64,
    alignment: u64,
}

/// Provider-object-opaque listener observation with an optional compiler-owned connection.
///
/// The first four words contain `(kind, state-or-adoption-status, error-domain, error-code)`.
/// The final 56 bytes are the fixed optional representation: a one-byte tag, alignment padding,
/// and one [`DarwinNetworkOwnerAbiSchema`](crate::DarwinNetworkOwnerAbiSchema) payload. A retained
/// native connection therefore never crosses this ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinNetworkListenerEventObservationAbiSchema {
    kind_offset: u64,
    value_offsets: [u64; 3],
    accepted_tag_offset: u64,
    accepted_owner_offset: u64,
    size: u64,
    alignment: u64,
}

impl DarwinNetworkListenerEventObservationAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        kind_offset: 0,
        value_offsets: [8, 16, 24],
        accepted_tag_offset: 32,
        accepted_owner_offset: 40,
        size: 88,
        alignment: 8,
    };

    #[must_use]
    pub const fn kind_offset(self) -> u64 {
        self.kind_offset
    }

    #[must_use]
    pub const fn value_offset(self, lane: usize) -> Option<u64> {
        if lane < self.value_offsets.len() {
            Some(self.value_offsets[lane])
        } else {
            None
        }
    }

    #[must_use]
    pub const fn accepted_tag_offset(self) -> u64 {
        self.accepted_tag_offset
    }

    #[must_use]
    pub const fn accepted_owner_offset(self) -> u64 {
        self.accepted_owner_offset
    }

    #[must_use]
    pub const fn accepted_optional_size(self) -> u64 {
        self.size - self.accepted_tag_offset
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

impl DarwinNetworkConnectionEventObservationAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        kind_offset: 0,
        value_offsets: [8, 16, 24, 32],
        size: 40,
        alignment: 8,
    };

    #[must_use]
    pub const fn kind_offset(self) -> u64 {
        self.kind_offset
    }

    #[must_use]
    pub const fn value_offset(self, lane: usize) -> Option<u64> {
        if lane < self.value_offsets.len() {
            Some(self.value_offsets[lane])
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

/// The fixed callback-channel transfer rule for the supported Darwin target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinNetworkChannelIoContract {
    complete_count: i64,
    interrupted_errno: i32,
}

impl DarwinNetworkChannelIoContract {
    pub const ARM64_DARWIN: Self = Self {
        complete_count: DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN
            .size
            .cast_signed(),
        interrupted_errno: 4,
    };

    #[must_use]
    pub const fn complete_count(self) -> i64 {
        self.complete_count
    }

    #[must_use]
    pub const fn interrupted_errno(self) -> i32 {
        self.interrupted_errno
    }

    /// Classifies one `send` or `recv` result without permitting short event records.
    #[must_use]
    pub const fn classify(self, result: i64, errno: i32) -> DarwinNetworkChannelIoOutcome {
        if result == self.complete_count {
            DarwinNetworkChannelIoOutcome::Complete
        } else if result == -1 && errno == self.interrupted_errno {
            DarwinNetworkChannelIoOutcome::Interrupted
        } else {
            DarwinNetworkChannelIoOutcome::Fatal
        }
    }
}

/// Closed callback-channel transfer outcomes understood by the native adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkChannelIoOutcome {
    Complete,
    Interrupted,
    Fatal,
}

/// Fixed Block roles admitted by the Darwin Network adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkCallbackRole {
    ConnectionState,
    ConnectionReceive,
    ConnectionSend,
    ListenerState,
    ListenerAccept,
}

impl DarwinNetworkCallbackRole {
    pub const ALL: &'static [Self] = &[
        Self::ConnectionState,
        Self::ConnectionReceive,
        Self::ConnectionSend,
        Self::ListenerState,
        Self::ListenerAccept,
    ];

    /// Returns the canonical NUL-terminated Objective-C Block signature.
    #[must_use]
    pub const fn block_signature(self) -> &'static [u8] {
        match self {
            Self::ConnectionState | Self::ListenerState => b"v20@?0i8^{nw_error=}12\0",
            Self::ConnectionReceive => {
                b"v36@?0^{dispatch_data_s=}8^{nw_content_context=}16B24^{nw_error=}28\0"
            }
            Self::ConnectionSend => b"v16@?0^{nw_error=}8\0",
            Self::ListenerAccept => b"v16@?0^{nw_connection=}8\0",
        }
    }

    #[must_use]
    pub const fn event_kind(self) -> Option<DarwinNetworkEventKind> {
        match self {
            Self::ConnectionState => Some(DarwinNetworkEventKind::ConnectionState),
            Self::ConnectionReceive => Some(DarwinNetworkEventKind::ReceiveCompletion),
            Self::ConnectionSend => Some(DarwinNetworkEventKind::SendCompletion),
            Self::ListenerState => Some(DarwinNetworkEventKind::ListenerState),
            Self::ListenerAccept => Some(DarwinNetworkEventKind::AcceptedConnection),
        }
    }
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
    pub const ALL: &'static [Self] = &[
        Self::Invalid,
        Self::Waiting,
        Self::Preparing,
        Self::Ready,
        Self::Failed,
        Self::Cancelled,
    ];

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
    pub const ALL: &'static [Self] = &[
        Self::Invalid,
        Self::Waiting,
        Self::Ready,
        Self::Failed,
        Self::Cancelled,
    ];

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

#[cfg(test)]
mod tests {
    use super::{
        DarwinNetworkCallbackEventAbiSchema, DarwinNetworkCallbackRole,
        DarwinNetworkChannelIoContract, DarwinNetworkChannelIoOutcome,
        DarwinNetworkConnectionState, DarwinNetworkEventKind, DarwinNetworkEventPayload,
        DarwinNetworkListenerEventObservationAbiSchema, DarwinNetworkListenerState,
    };

    #[test]
    fn callback_event_has_one_dense_fixed_record_layout() {
        let schema = DarwinNetworkCallbackEventAbiSchema::ARM64_DARWIN;
        assert_eq!(schema.kind_offset(), 0);
        assert_eq!(schema.payload_offset(0), Some(8));
        assert_eq!(schema.payload_offset(3), Some(32));
        assert_eq!(schema.payload_offset(4), None);

        let events = [
            Some(DarwinNetworkEventKind::ConnectionState),
            Some(DarwinNetworkEventKind::ReceiveCompletion),
            Some(DarwinNetworkEventKind::SendCompletion),
            Some(DarwinNetworkEventKind::ListenerState),
            Some(DarwinNetworkEventKind::AcceptedConnection),
        ];
        for (role, event) in DarwinNetworkCallbackRole::ALL.iter().copied().zip(events) {
            assert!(role.block_signature().ends_with(&[0]));
            assert_eq!(role.event_kind(), event);
        }
        assert_eq!(schema.size(), 40);
        assert_eq!(schema.alignment(), 8);

        let channel = DarwinNetworkChannelIoContract::ARM64_DARWIN;
        assert_eq!(channel.complete_count(), 40);
        assert_eq!(
            channel.classify(40, 0),
            DarwinNetworkChannelIoOutcome::Complete
        );
        assert_eq!(
            channel.classify(-1, channel.interrupted_errno()),
            DarwinNetworkChannelIoOutcome::Interrupted
        );
        assert_eq!(
            channel.classify(39, channel.interrupted_errno()),
            DarwinNetworkChannelIoOutcome::Fatal
        );
        assert_eq!(
            channel.classify(-1, 9),
            DarwinNetworkChannelIoOutcome::Fatal
        );
    }

    #[test]
    fn listener_observation_contains_only_values_and_an_optional_owned_connection() {
        let schema = DarwinNetworkListenerEventObservationAbiSchema::ARM64_DARWIN;
        assert_eq!(schema.kind_offset(), 0);
        assert_eq!(schema.value_offset(0), Some(8));
        assert_eq!(schema.value_offset(2), Some(24));
        assert_eq!(schema.value_offset(3), None);
        assert_eq!(schema.accepted_tag_offset(), 32);
        assert_eq!(schema.accepted_owner_offset(), 40);
        assert_eq!(schema.accepted_optional_size(), 56);
        assert_eq!(schema.size(), 88);
        assert_eq!(schema.alignment(), 8);
    }

    #[test]
    fn consumed_connection_event_has_one_native_free_result_layout() {
        let schema = super::DarwinNetworkConnectionEventObservationAbiSchema::ARM64_DARWIN;
        assert_eq!(schema.kind_offset(), 0);
        assert_eq!(schema.value_offset(0), Some(8));
        assert_eq!(schema.value_offset(3), Some(32));
        assert_eq!(schema.value_offset(4), None);
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
    }
}
