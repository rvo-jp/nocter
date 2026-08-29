//! Revisioned demand computation independent of compiler domain semantics.

mod database;
mod error;
mod identity;

pub use database::{ComputationRevision, Database, InputRevision};
pub use error::ComputationError;
pub use identity::{ComputationCategory, ComputationIdentity, ComputationKey, Fingerprint};

/// Immutable input family supplied at a computation revision.
pub trait Input: 'static {
    type Key: ComputationKey;
    type Value: QueryValue;
}

/// One pure derived computation family.
pub trait Query: 'static {
    type Key: ComputationKey;
    type Value: QueryValue;

    /// Derives this query value while recording any inputs and child queries read through the
    /// supplied database.
    ///
    /// # Errors
    ///
    /// Returns a computation-kernel failure. Recoverable compiler-domain outcomes belong in
    /// [`Self::Value`] so that they remain fingerprinted and reusable.
    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError>;
}

/// Immutable query or input value with deterministic cross-revision identity.
pub trait QueryValue: Send + Sync + 'static {
    fn fingerprint(&self) -> Fingerprint;
}

#[cfg(test)]
mod tests;
