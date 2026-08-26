use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONSTRUCTION_IDENTITY: AtomicU64 = AtomicU64::new(1);

/// Process-local identity of one mutable semantic construction store.
///
/// Checkpoints use this identity only to reject application to another builder. It never enters
/// immutable semantic output and therefore cannot affect deterministic program identities.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct ConstructionIdentity(u64);

impl ConstructionIdentity {
    pub(crate) fn fresh() -> Self {
        let identity = NEXT_CONSTRUCTION_IDENTITY.fetch_add(1, Ordering::Relaxed);
        assert_ne!(identity, 0, "construction identity space exhausted");
        Self(identity)
    }
}

impl fmt::Debug for ConstructionIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ConstructionIdentity")
    }
}
