use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SERVICE: AtomicU64 = AtomicU64::new(1);

/// Process-local identity of one lifecycle service.
///
/// This value is deliberately private: consumers receive service-qualified observations but
/// cannot manufacture or reinterpret the qualification.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ServiceIdentity(u64);

impl ServiceIdentity {
    pub(crate) fn allocate() -> Self {
        let identity = NEXT_SERVICE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("blocking-job service identity space exhausted");
        Self(identity)
    }
}

/// Service-qualified monotonic identity of one accepted blocking job.
///
/// Identities are never reused within one service and cannot alias an identity issued by another
/// service, so a delayed or misrouted completion cannot name a different job.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct JobId {
    service: ServiceIdentity,
    sequence: u64,
}

impl JobId {
    pub(crate) const fn new(service: ServiceIdentity, sequence: u64) -> Self {
        Self { service, sequence }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.sequence
    }
}

/// Service-qualified monotonic observation of admission-capacity changes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CapacityEpoch {
    service: ServiceIdentity,
    sequence: u64,
}

/// Service-qualified monotonic observation of retirement-capacity changes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RetirementEpoch {
    service: ServiceIdentity,
    sequence: u64,
}

/// Service-qualified identity of one retirement observed by an explicit waiter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RetirementId {
    service: ServiceIdentity,
    sequence: u64,
}

impl RetirementId {
    pub(crate) const fn new(service: ServiceIdentity, sequence: u64) -> Self {
        Self { service, sequence }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.sequence
    }
}

impl RetirementEpoch {
    pub(crate) const fn initial(service: ServiceIdentity) -> Self {
        Self {
            service,
            sequence: 0,
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.sequence
    }

    pub(crate) const fn belongs_to(self, service: ServiceIdentity) -> bool {
        self.service.0 == service.0
    }

    pub(crate) fn can_cover_releases(self, releases: usize) -> bool {
        u64::try_from(releases)
            .ok()
            .and_then(|releases| self.sequence.checked_add(releases))
            .is_some()
    }

    pub(crate) fn advance(&mut self) {
        self.sequence = self
            .sequence
            .checked_add(1)
            .expect("retirement admission reserves epoch space before resource creation");
    }
}

impl CapacityEpoch {
    pub(crate) const fn initial(service: ServiceIdentity) -> Self {
        Self {
            service,
            sequence: 0,
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.sequence
    }

    pub(crate) const fn belongs_to(self, service: ServiceIdentity) -> bool {
        self.service.0 == service.0
    }

    pub(crate) fn advance(&mut self) {
        // One service can release no more jobs than it accepted. Job identity allocation stops
        // before this epoch could wrap, so saturation here also represents permanent identity
        // exhaustion rather than a missed reusable-capacity notification.
        self.sequence = self.sequence.saturating_add(1);
    }
}
