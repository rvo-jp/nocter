use std::sync::atomic::{AtomicU64, Ordering};

/// Process-unique identity for one active mutation transaction.
///
/// A fresh identity is assigned on every begin, so a token cannot control another owner or a later
/// transaction on the same owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TransactionIdentity(u64);

impl TransactionIdentity {
    pub(crate) fn fresh() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let identity = NEXT.fetch_add(1, Ordering::Relaxed);
        assert_ne!(identity, 0, "transaction identity space exhausted");
        Self(identity)
    }
}
