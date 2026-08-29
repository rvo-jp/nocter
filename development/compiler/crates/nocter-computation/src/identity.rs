use std::any::type_name;

/// Deterministic identity of one query value across input revisions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(nocter_hash::sha256(bytes))
    }

    #[must_use]
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    #[must_use]
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

/// Exact stable bytes for one input or query key.
///
/// Implementations must encode every field that changes semantic identity. Arena IDs whose owner
/// is rebuilt between revisions must not implement this trait.
pub trait ComputationKey: Clone + Send + Sync + 'static {
    fn stable_bytes(&self) -> Box<[u8]>;
}

impl ComputationKey for () {
    fn stable_bytes(&self) -> Box<[u8]> {
        Box::new([])
    }
}

impl ComputationKey for u64 {
    fn stable_bytes(&self) -> Box<[u8]> {
        Box::new(self.to_be_bytes())
    }
}

impl ComputationKey for String {
    fn stable_bytes(&self) -> Box<[u8]> {
        self.as_bytes().into()
    }
}

impl ComputationKey for Box<str> {
    fn stable_bytes(&self) -> Box<[u8]> {
        self.as_bytes().into()
    }
}

/// Opaque diagnostic identity of one input or derived query instance.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ComputationIdentity {
    category: ComputationCategory,
    name: &'static str,
    key: Box<[u8]>,
}

impl ComputationIdentity {
    pub(crate) fn input<I: crate::Input>(key: &I::Key) -> Self {
        Self::new(ComputationCategory::Input, type_name::<I>(), key)
    }

    pub(crate) fn query<Q: crate::Query>(key: &Q::Key) -> Self {
        Self::new(ComputationCategory::Query, type_name::<Q>(), key)
    }

    fn new<K: ComputationKey>(category: ComputationCategory, name: &'static str, key: &K) -> Self {
        Self {
            category,
            name,
            key: key.stable_bytes(),
        }
    }

    #[must_use]
    pub const fn category(&self) -> ComputationCategory {
        self.category
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn key(&self) -> &[u8] {
        &self.key
    }
}

/// Whether a computation identity names authored input or a derived query.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ComputationCategory {
    Input,
    Query,
}
