use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::identity::ServiceIdentity;
use crate::{RetirementEpoch, RetirementId};

/// Fixed cleanup-worker and live-resource limits for one retirement service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementCapacity {
    workers: usize,
    maximum_resources: usize,
}

impl RetirementCapacity {
    /// Creates a bounded retirement domain.
    ///
    /// # Errors
    ///
    /// Rejects zero workers or a resource bound smaller than the worker count.
    pub const fn new(workers: usize, maximum_resources: usize) -> Result<Self, RetirementError> {
        if workers == 0 {
            return Err(RetirementError::ZeroWorkers);
        }
        if maximum_resources < workers {
            return Err(RetirementError::ResourceCapacityBelowWorkers {
                workers,
                maximum_resources,
            });
        }
        Ok(Self {
            workers,
            maximum_resources,
        })
    }

    #[must_use]
    pub const fn workers(self) -> usize {
        self.workers
    }

    #[must_use]
    pub const fn maximum_resources(self) -> usize {
        self.maximum_resources
    }
}

/// Read-only retirement counters at one instant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementSnapshot {
    accepting: bool,
    reserved: usize,
    queued: usize,
    running: usize,
    completed: usize,
    capacity_epoch: RetirementEpoch,
}

impl RetirementSnapshot {
    #[must_use]
    pub const fn accepting(self) -> bool {
        self.accepting
    }

    /// Number of permits, live owners, queued resources, and running resources combined.
    #[must_use]
    pub const fn reserved(self) -> usize {
        self.reserved
    }

    #[must_use]
    pub const fn queued(self) -> usize {
        self.queued
    }

    #[must_use]
    pub const fn running(self) -> usize {
        self.running
    }

    #[must_use]
    pub const fn completed(self) -> usize {
        self.completed
    }

    #[must_use]
    pub const fn capacity_epoch(self) -> RetirementEpoch {
        self.capacity_epoch
    }

    #[must_use]
    pub const fn drained(self) -> bool {
        self.reserved == 0
    }
}

/// Saturation evidence for one failed resource reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetirementBackpressure {
    observed_epoch: RetirementEpoch,
}

impl RetirementBackpressure {
    #[must_use]
    pub const fn observed_epoch(self) -> RetirementEpoch {
        self.observed_epoch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementReserveError {
    Closed,
    Saturated(RetirementBackpressure),
    IdentityExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementError {
    ZeroWorkers,
    ResourceCapacityBelowWorkers {
        workers: usize,
        maximum_resources: usize,
    },
    ForeignCapacityEpoch(RetirementEpoch),
    IdentityExhausted,
    UnknownRetirement(RetirementId),
    InvalidRetirementState(RetirementId),
}

impl fmt::Display for RetirementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "resource retirement lifecycle failed: {self:?}")
    }
}

impl std::error::Error for RetirementError {}

struct QueuedRetirement<R> {
    identity: Option<RetirementId>,
    resource: R,
}

enum ActiveRetirement {
    Running,
    DetachedRunning,
    Completed,
}

/// Waiter-visible state of one explicit retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementStatus {
    Queued,
    Running,
    Completed,
}

struct Inner<R> {
    identity: ServiceIdentity,
    capacity: RetirementCapacity,
    accepting: bool,
    reserved: usize,
    running: usize,
    next_retirement: u64,
    capacity_epoch: RetirementEpoch,
    queue: VecDeque<QueuedRetirement<R>>,
    active: BTreeMap<RetirementId, ActiveRetirement>,
    notify: Arc<dyn Fn() + Send + Sync>,
}

impl<R> Inner<R> {
    fn snapshot(&self) -> RetirementSnapshot {
        RetirementSnapshot {
            accepting: self.accepting,
            reserved: self.reserved,
            queued: self.queue.len(),
            running: self.running,
            completed: self
                .active
                .values()
                .filter(|state| matches!(state, ActiveRetirement::Completed))
                .count(),
            capacity_epoch: self.capacity_epoch,
        }
    }

    fn release_reservation(&mut self) {
        debug_assert!(self.reserved > 0);
        self.reserved -= 1;
        self.capacity_epoch.advance();
    }
}

/// Bounded owner of permits and queued resource retirement.
pub struct RetirementService<R> {
    inner: Arc<Mutex<Inner<R>>>,
}

impl<R> Clone for RetirementService<R> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<R> RetirementService<R> {
    #[must_use]
    pub fn new(capacity: RetirementCapacity) -> Self {
        Self::with_notifier(capacity, || {})
    }

    /// Creates a retirement service whose queue and capacity changes publish wake-only notices.
    #[must_use]
    pub fn with_notifier<N>(capacity: RetirementCapacity, notify: N) -> Self
    where
        N: Fn() + Send + Sync + 'static,
    {
        let identity = ServiceIdentity::allocate();
        Self {
            inner: Arc::new(Mutex::new(Inner {
                identity,
                capacity,
                accepting: true,
                reserved: 0,
                running: 0,
                next_retirement: 0,
                capacity_epoch: RetirementEpoch::initial(identity),
                queue: VecDeque::new(),
                active: BTreeMap::new(),
                notify: Arc::new(notify),
            })),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> RetirementSnapshot {
        lock(&self.inner).snapshot()
    }

    /// Reserves one queue slot before the corresponding native resource can exist.
    ///
    /// # Errors
    ///
    /// Reports closed admission, bounded resource saturation, or permanent observation-identity
    /// exhaustion before accepting a resource whose eventual release could not be represented.
    pub fn reserve(&self) -> Result<ResourcePermit<R>, RetirementReserveError> {
        let mut inner = lock(&self.inner);
        if !inner.accepting {
            return Err(RetirementReserveError::Closed);
        }
        if inner.reserved == inner.capacity.maximum_resources {
            return Err(RetirementReserveError::Saturated(RetirementBackpressure {
                observed_epoch: inner.capacity_epoch,
            }));
        }
        if !inner.capacity_epoch.can_cover_releases(inner.reserved + 1) {
            return Err(RetirementReserveError::IdentityExhausted);
        }
        inner.reserved += 1;
        Ok(ResourcePermit {
            inner: Some(Arc::clone(&self.inner)),
        })
    }

    /// Reports whether this service released capacity after the supplied observation.
    ///
    /// # Errors
    ///
    /// Rejects an observation issued by another retirement service.
    pub fn capacity_changed_since(
        &self,
        observed: RetirementEpoch,
    ) -> Result<bool, RetirementError> {
        let inner = lock(&self.inner);
        if !observed.belongs_to(inner.identity) {
            return Err(RetirementError::ForeignCapacityEpoch(observed));
        }
        Ok(inner.capacity_epoch != observed)
    }

    /// Returns the exact waiter-visible state while this explicit retirement remains current.
    #[must_use]
    pub fn status(&self, retirement: RetirementId) -> Option<RetirementStatus> {
        let inner = lock(&self.inner);
        if inner
            .queue
            .iter()
            .any(|queued| queued.identity == Some(retirement))
        {
            return Some(RetirementStatus::Queued);
        }
        inner.active.get(&retirement).and_then(|state| match state {
            ActiveRetirement::Running => Some(RetirementStatus::Running),
            ActiveRetirement::Completed => Some(RetirementStatus::Completed),
            ActiveRetirement::DetachedRunning => None,
        })
    }

    /// Consumes exact observed completion and releases its resource reservation.
    ///
    /// # Errors
    ///
    /// Rejects an unknown, queued, running, or already detached retirement.
    pub fn consume(&self, retirement: RetirementId) -> Result<(), RetirementError> {
        let notify = {
            let mut inner = lock(&self.inner);
            if !matches!(
                inner.active.get(&retirement),
                Some(ActiveRetirement::Completed)
            ) {
                return Err(
                    if inner.active.contains_key(&retirement)
                        || inner
                            .queue
                            .iter()
                            .any(|queued| queued.identity == Some(retirement))
                    {
                        RetirementError::InvalidRetirementState(retirement)
                    } else {
                        RetirementError::UnknownRetirement(retirement)
                    },
                );
            }
            inner.active.remove(&retirement);
            inner.release_reservation();
            Arc::clone(&inner.notify)
        };
        notify();
        Ok(())
    }

    /// Detaches an explicit waiter without cancelling resource cleanup.
    ///
    /// # Errors
    ///
    /// Rejects an unknown or already detached retirement.
    pub fn detach(&self, retirement: RetirementId) -> Result<(), RetirementError> {
        let mut notify = None;
        {
            let mut inner = lock(&self.inner);
            if let Some(queued) = inner
                .queue
                .iter_mut()
                .find(|queued| queued.identity == Some(retirement))
            {
                queued.identity = None;
                return Ok(());
            }
            match inner.active.get_mut(&retirement) {
                Some(state @ ActiveRetirement::Running) => {
                    *state = ActiveRetirement::DetachedRunning;
                }
                Some(ActiveRetirement::Completed) => {
                    inner.active.remove(&retirement);
                    inner.release_reservation();
                    notify = Some(Arc::clone(&inner.notify));
                }
                Some(ActiveRetirement::DetachedRunning) | None => {
                    return Err(RetirementError::UnknownRetirement(retirement));
                }
            }
        }
        if let Some(notify) = notify {
            notify();
        }
        Ok(())
    }

    /// Detaches every explicit waiter while preserving all queued and running cleanup.
    ///
    /// This is the adapter-destruction boundary. It cannot cancel native retirement and does not
    /// alter resources that are still held by external `ResourceOwner` values.
    pub fn detach_all(&self) {
        let notify = {
            let mut inner = lock(&self.inner);
            let mut changed = false;
            for queued in &mut inner.queue {
                changed |= queued.identity.take().is_some();
            }
            let identities = inner.active.keys().copied().collect::<Vec<_>>();
            for identity in identities {
                match inner.active.get_mut(&identity) {
                    Some(state @ ActiveRetirement::Running) => {
                        *state = ActiveRetirement::DetachedRunning;
                        changed = true;
                    }
                    Some(ActiveRetirement::Completed) => {
                        inner.active.remove(&identity);
                        inner.release_reservation();
                        changed = true;
                    }
                    Some(ActiveRetirement::DetachedRunning) | None => {}
                }
            }
            changed.then(|| Arc::clone(&inner.notify))
        };
        if let Some(notify) = notify {
            notify();
        }
    }

    /// Transfers the next queued resource to one cleanup worker.
    #[must_use]
    pub fn claim(&self) -> Option<RetiringResource<R>> {
        let mut inner = lock(&self.inner);
        if inner.running == inner.capacity.workers {
            return None;
        }
        let queued = inner.queue.pop_front()?;
        inner.running += 1;
        if let Some(identity) = queued.identity {
            let previous = inner.active.insert(identity, ActiveRetirement::Running);
            debug_assert!(previous.is_none());
        }
        Some(RetiringResource {
            identity: queued.identity,
            resource: Some(queued.resource),
            inner: Arc::clone(&self.inner),
        })
    }

    /// Stops new permits without invalidating already reserved cleanup admission.
    pub fn close_admission(&self) {
        let notify = {
            let mut inner = lock(&self.inner);
            if !inner.accepting {
                return;
            }
            inner.accepting = false;
            Arc::clone(&inner.notify)
        };
        notify();
    }
}

/// One reserved retirement slot before resource creation succeeds.
pub struct ResourcePermit<R> {
    inner: Option<Arc<Mutex<Inner<R>>>>,
}

impl<R> ResourcePermit<R> {
    /// Attaches an initialized resource to this infallible retirement reservation.
    #[must_use]
    pub fn attach(mut self, resource: R) -> ResourceOwner<R> {
        ResourceOwner {
            resource: Some(resource),
            inner: self.inner.take(),
        }
    }
}

impl<R> Drop for ResourcePermit<R> {
    fn drop(&mut self) {
        let Some(inner) = self.inner.take() else {
            return;
        };
        let notify = {
            let mut state = lock(&inner);
            state.release_reservation();
            Arc::clone(&state.notify)
        };
        notify();
    }
}

/// One live resource whose destruction always enters its pre-reserved cleanup queue.
pub struct ResourceOwner<R> {
    resource: Option<R>,
    inner: Option<Arc<Mutex<Inner<R>>>>,
}

impl<R> ResourceOwner<R> {
    /// Borrows the live resource.
    ///
    /// # Panics
    ///
    /// Panics only after an internal ownership invariant is violated; no public transition can
    /// detach the resource while the owner remains observable.
    #[must_use]
    pub fn resource(&self) -> &R {
        self.resource
            .as_ref()
            .expect("live retirement owner retains its resource")
    }

    /// Mutably borrows the live resource.
    ///
    /// # Panics
    ///
    /// Panics only after an internal ownership invariant is violated; no public transition can
    /// detach the resource while the owner remains observable.
    #[must_use]
    pub fn resource_mut(&mut self) -> &mut R {
        self.resource
            .as_mut()
            .expect("live retirement owner retains its resource")
    }

    /// Enqueues cleanup with one exact identity that an explicit close operation can await.
    ///
    /// Cleanup admission itself cannot fail: identity exhaustion still enqueues the resource as an
    /// unobserved retirement before reporting the inability to construct a waiter identity.
    ///
    /// # Errors
    ///
    /// Reports permanent retirement-identity exhaustion after transferring the resource to its
    /// pre-reserved queue.
    ///
    /// # Panics
    ///
    /// Panics only after an internal ownership invariant is violated; no public transition can
    /// detach the resource or service while the owner remains observable.
    pub fn retire(mut self) -> Result<RetirementId, RetirementError> {
        let resource = self
            .resource
            .take()
            .expect("live retirement owner retains its resource");
        let inner = self
            .inner
            .take()
            .expect("live retirement owner retains its service");
        let (identity, notify) = {
            let mut state = lock(&inner);
            let identity = state.next_retirement.checked_add(1).map(|next| {
                let identity = RetirementId::new(state.identity, state.next_retirement);
                state.next_retirement = next;
                identity
            });
            debug_assert!(state.queue.len() + state.running < state.reserved);
            state
                .queue
                .push_back(QueuedRetirement { identity, resource });
            (identity, Arc::clone(&state.notify))
        };
        notify();
        identity.ok_or(RetirementError::IdentityExhausted)
    }
}

impl<R> Drop for ResourceOwner<R> {
    fn drop(&mut self) {
        let Some(resource) = self.resource.take() else {
            return;
        };
        let Some(inner) = self.inner.take() else {
            unreachable!("live retirement owner retains its service")
        };
        let notify = {
            let mut state = lock(&inner);
            debug_assert!(state.queue.len() + state.running < state.reserved);
            state.queue.push_back(QueuedRetirement {
                identity: None,
                resource,
            });
            Arc::clone(&state.notify)
        };
        notify();
    }
}

/// One cleanup-worker owner. Destruction releases its reservation even after cleanup failure.
pub struct RetiringResource<R> {
    identity: Option<RetirementId>,
    resource: Option<R>,
    inner: Arc<Mutex<Inner<R>>>,
}

impl<R> RetiringResource<R> {
    /// Borrows the resource on its cleanup worker.
    ///
    /// # Panics
    ///
    /// Panics only after an internal ownership invariant is violated; cleanup cannot detach the
    /// resource without consuming this guard.
    #[must_use]
    pub fn resource(&self) -> &R {
        self.resource
            .as_ref()
            .expect("running retirement retains its resource")
    }

    /// Mutably borrows the resource on its cleanup worker.
    ///
    /// # Panics
    ///
    /// Panics only after an internal ownership invariant is violated; cleanup cannot detach the
    /// resource without consuming this guard.
    #[must_use]
    pub fn resource_mut(&mut self) -> &mut R {
        self.resource
            .as_mut()
            .expect("running retirement retains its resource")
    }

    /// Destroys the retired resource on the cleanup worker and releases its capacity slot.
    pub fn finish(mut self) {
        drop(self.resource.take());
    }
}

impl<R> Drop for RetiringResource<R> {
    fn drop(&mut self) {
        drop(self.resource.take());
        let notify = {
            let mut inner = lock(&self.inner);
            debug_assert!(inner.running > 0);
            inner.running -= 1;
            match self.identity {
                Some(identity) => match inner.active.get_mut(&identity) {
                    Some(state @ ActiveRetirement::Running) => {
                        *state = ActiveRetirement::Completed;
                    }
                    Some(ActiveRetirement::DetachedRunning) => {
                        inner.active.remove(&identity);
                        inner.release_reservation();
                    }
                    Some(ActiveRetirement::Completed) | None => {
                        unreachable!("running retirement identity has one active state")
                    }
                },
                None => inner.release_reservation(),
            }
            Arc::clone(&inner.notify)
        };
        notify();
    }
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{
        RetirementCapacity, RetirementError, RetirementReserveError, RetirementService,
        RetirementStatus,
    };

    fn service(maximum: usize) -> RetirementService<String> {
        RetirementService::new(RetirementCapacity::new(1, maximum).unwrap())
    }

    #[test]
    fn permit_bounds_live_and_queued_resources_before_creation() {
        let service = service(1);
        let permit = service.reserve().unwrap();
        let Err(RetirementReserveError::Saturated(backpressure)) = service.reserve() else {
            panic!("expected bounded retirement saturation")
        };
        assert_eq!(service.snapshot().reserved(), 1);
        drop(permit);
        assert!(
            service
                .capacity_changed_since(backpressure.observed_epoch())
                .unwrap()
        );
        assert!(service.reserve().is_ok());
    }

    #[test]
    fn owner_drop_uses_its_reserved_queue_without_fallible_admission() {
        let service = service(1);
        let owner = service.reserve().unwrap().attach(String::from("file"));
        assert_eq!(owner.resource(), "file");
        drop(owner);
        assert_eq!(service.snapshot().queued(), 1);
        let retirement = service.claim().unwrap();
        assert_eq!(retirement.resource(), "file");
        retirement.finish();
        assert!(service.snapshot().drained());
    }

    #[test]
    fn explicit_retirement_keeps_exact_completion_until_consumed() {
        let service = service(1);
        let retirement = service
            .reserve()
            .unwrap()
            .attach(String::from("file"))
            .retire()
            .unwrap();
        assert_eq!(service.status(retirement), Some(RetirementStatus::Queued));
        service.claim().unwrap().finish();
        assert_eq!(
            service.status(retirement),
            Some(RetirementStatus::Completed)
        );
        assert_eq!(service.snapshot().reserved(), 1);
        service.consume(retirement).unwrap();
        assert!(service.snapshot().drained());
        assert_eq!(
            service.consume(retirement),
            Err(RetirementError::UnknownRetirement(retirement))
        );
    }

    #[test]
    fn detached_waiter_never_prevents_resource_cleanup_or_capacity_release() {
        let service = service(1);
        let retirement = service
            .reserve()
            .unwrap()
            .attach(String::from("file"))
            .retire()
            .unwrap();
        let worker = service.claim().unwrap();
        service.detach(retirement).unwrap();
        assert_eq!(service.status(retirement), None);
        worker.finish();
        assert!(service.snapshot().drained());
    }

    #[test]
    fn adapter_detach_removes_every_waiter_without_cancelling_cleanup() {
        let service = service(2);
        let queued = service
            .reserve()
            .unwrap()
            .attach(String::from("queued"))
            .retire()
            .unwrap();
        let completed = service
            .reserve()
            .unwrap()
            .attach(String::from("completed"))
            .retire()
            .unwrap();
        service.claim().unwrap().finish();
        assert_eq!(service.status(queued), Some(RetirementStatus::Completed));
        service.detach_all();
        assert_eq!(service.status(queued), None);
        assert_eq!(service.status(completed), None);
        assert_eq!(service.snapshot().reserved(), 1);
        service.claim().unwrap().finish();
        assert!(service.snapshot().drained());
    }

    #[test]
    fn worker_owner_drop_closes_capacity_after_cleanup_panic() {
        let service = service(1);
        drop(service.reserve().unwrap().attach(String::from("file")));
        let retirement = service.claim().unwrap();
        drop(retirement);
        assert!(service.snapshot().drained());
    }

    #[test]
    fn closed_admission_still_accepts_pre_reserved_retirement() {
        let service = service(1);
        let owner = service.reserve().unwrap().attach(String::from("file"));
        service.close_admission();
        assert!(matches!(
            service.reserve(),
            Err(RetirementReserveError::Closed)
        ));
        drop(owner);
        service.claim().unwrap().finish();
        assert!(service.snapshot().drained());
    }

    #[test]
    fn closing_admission_notifies_worker_control_once() {
        let notifications = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&notifications);
        let service = RetirementService::<String>::with_notifier(
            RetirementCapacity::new(1, 1).unwrap(),
            move || {
                observed.fetch_add(1, Ordering::SeqCst);
            },
        );
        service.close_admission();
        service.close_admission();
        assert_eq!(notifications.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn epochs_cannot_cross_retirement_services() {
        let first = service(1);
        let second = service(1);
        let _permit = first.reserve().unwrap();
        let Err(RetirementReserveError::Saturated(backpressure)) = first.reserve() else {
            panic!("expected bounded retirement saturation")
        };
        assert!(matches!(
            second.capacity_changed_since(backpressure.observed_epoch()),
            Err(RetirementError::ForeignCapacityEpoch(_))
        ));
    }

    #[test]
    fn notification_runs_after_unlock_and_can_reenter_snapshot() {
        let notifications = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&notifications);
        let holder = Arc::new(std::sync::Mutex::new(None::<RetirementService<String>>));
        let callback_holder = Arc::clone(&holder);
        let service =
            RetirementService::with_notifier(RetirementCapacity::new(1, 1).unwrap(), move || {
                observed.fetch_add(1, Ordering::SeqCst);
                if let Some(service) = &*callback_holder.lock().unwrap() {
                    let _: super::RetirementSnapshot = service.snapshot();
                }
            });
        *holder.lock().unwrap() = Some(service.clone());
        drop(service.reserve().unwrap());
        assert_eq!(notifications.load(Ordering::SeqCst), 1);
    }
}
