use crate::{ReactorInterest, RegistrationId};

/// Target adapter for readiness registration and blocking event collection.
///
/// `deregister` is infallible at this boundary: a target adapter must normalize interruption,
/// already-removed registrations, and platform bookkeeping before returning. The scheduler first
/// invalidates the generation, so even a native event already in flight cannot wake a task.
pub trait Reactor {
    type Error;

    /// Installs one generation-qualified interest.
    ///
    /// # Errors
    ///
    /// Returns a target adapter error without retaining the rejected registration.
    fn register(
        &mut self,
        registration: RegistrationId,
        interest: ReactorInterest,
    ) -> Result<(), Self::Error>;

    fn deregister(&mut self, registration: RegistrationId, interest: ReactorInterest);

    /// Waits for the next native event batch.
    ///
    /// # Errors
    ///
    /// Returns a normalized target wait error. Interrupted waits should ordinarily be retried by
    /// the adapter rather than exposed here.
    fn wait(&mut self) -> Result<Box<[RegistrationId]>, Self::Error>;
}
