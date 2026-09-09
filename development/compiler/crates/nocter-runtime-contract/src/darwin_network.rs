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
}

#[cfg(test)]
mod tests {
    use super::{
        DarwinNetworkCallbackEventAbiSchema, DarwinNetworkConnectionState, DarwinNetworkEventKind,
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
    }
}
