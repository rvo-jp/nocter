/// Monotonic identity of one accepted blocking job.
///
/// Identities are never reused within one service, so a delayed completion cannot name a later
/// occupant of the same bounded capacity slot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct JobId(u64);

impl JobId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Monotonic observation of admission-capacity changes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CapacityEpoch(u64);

impl CapacityEpoch {
    pub(crate) const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) fn advance(&mut self) {
        // One service can release no more jobs than it accepted. Job identity allocation stops
        // before this epoch could wrap, so saturation here also represents permanent identity
        // exhaustion rather than a missed reusable-capacity notification.
        self.0 = self.0.saturating_add(1);
    }
}
